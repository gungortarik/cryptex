//! Headless package experiment health gate using the real shared Desktop IPC client.
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use cryptexd::error::LifecycleError;
use cryptexd::ipc::HealthResponse;
use serde_json::json;

#[derive(Debug)]
struct Options {
    pid: u32,
    protocol: u32,
    root: PathBuf,
}

fn options(args: &[String]) -> Result<Options, LifecycleError> {
    let mut pid = None;
    let mut protocol = None;
    let mut root = None;
    for pair in args.chunks(2) {
        if pair.len() != 2 {
            return Err(LifecycleError::Usage("health option needs a value".into()));
        }
        match pair[0].as_str() {
            "--expected-pid" if pid.is_none() => pid = Some(positive(&pair[1])?),
            "--expected-protocol" if protocol.is_none() => protocol = Some(positive(&pair[1])?),
            "--runtime-root" if root.is_none() => root = Some(PathBuf::from(&pair[1])),
            _ => {
                return Err(LifecycleError::Usage(
                    "unknown or duplicate health option".into(),
                ));
            }
        }
    }
    let result = Options {
        pid: pid.ok_or_else(|| LifecycleError::Usage("expected PID required".into()))?,
        protocol: protocol
            .ok_or_else(|| LifecycleError::Usage("expected protocol required".into()))?,
        root: root.ok_or_else(|| LifecycleError::Usage("runtime root required".into()))?,
    };
    if !result.root.is_absolute() {
        return Err(LifecycleError::UnsafeRuntimePath(
            "absolute runtime path required".into(),
        ));
    }
    cryptexd::paths::assert_outside_repository(&result.root)?;
    // Do not depend on a runtime CARGO_MANIFEST_DIR environment variable.
    let repository = fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))?;
    let runtime = fs::canonicalize(&result.root)?;
    if runtime.starts_with(repository) {
        return Err(LifecycleError::UnsafeRuntimePath(
            "health observation cannot use repository runtime".into(),
        ));
    }
    Ok(result)
}

fn positive(value: &str) -> Result<u32, LifecycleError> {
    value
        .parse::<u32>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| LifecycleError::Usage("positive u32 required".into()))
}

// The shared client bounds connect retries, not response reads. Bound the whole
// experiment query so an existing unresponsive endpoint cannot stall the helper.
fn bounded_ping() -> Result<io::Result<HealthResponse>, LifecycleError> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let response = cryptexd::ipc::desktop_health_ping(Duration::from_millis(300));
        let _ = sender.send(response);
    });
    receiver.recv_timeout(Duration::from_secs(2)).map_err(|_| {
        LifecycleError::Attention(
            "package health IPC response timed out; endpoint not assumed absent".into(),
        )
    })
}

fn validate(response: &HealthResponse, pid: u32, protocol: u32) -> Result<(), LifecycleError> {
    if response.daemon_pid != pid {
        return Err(LifecycleError::Attention(format!(
            "IPC PID mismatch: expected {pid}, observed {}",
            response.daemon_pid
        )));
    }
    if response.protocol_version != protocol {
        return Err(LifecycleError::Incompatible(format!(
            "expected protocol {protocol}, observed {}",
            response.protocol_version
        )));
    }
    if !response.accepted
        || response.operation.as_deref() != Some("health.pong")
        || response.error.is_some()
    {
        return Err(LifecycleError::Attention(
            "IPC health response was not accepted".into(),
        ));
    }
    Ok(())
}

pub(crate) fn run(args: &[String]) -> Result<(), LifecycleError> {
    if args == ["--expect-absent"] {
        match bounded_ping()? {
            Ok(response) => {
                return Err(LifecycleError::AlreadyRunning {
                    holder_pid: Some(response.daemon_pid),
                });
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) =>
            {
                println!("{}", json!({"schema": 1, "endpoint_absent": true}));
                return Ok(());
            }
            Err(error) => {
                return Err(LifecycleError::Attention(format!(
                    "endpoint state ambiguous: {error}"
                )));
            }
        }
    }
    let opts = options(args)?;
    let response = bounded_ping()?.map_err(LifecycleError::from)?;
    validate(&response, opts.pid, opts.protocol)?;
    let observation = cryptexd::instance::observe_instance(&opts.root)?;
    if !observation.lock_held || observation.holder_pid != Some(opts.pid) {
        return Err(LifecycleError::Attention(
            "IPC PID and held runtime lock do not match".into(),
        ));
    }
    println!(
        "{}",
        json!({"schema": 1, "daemon_pid": response.daemon_pid,
        "protocol_version": response.protocol_version, "operation": "health.pong",
        "client_role": "desktop", "lock_held": true})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response() -> HealthResponse {
        HealthResponse {
            protocol_version: 1,
            request_id: Some("fixture".into()),
            accepted: true,
            error: None,
            operation: Some("health.pong".into()),
            daemon_pid: 123,
        }
    }

    #[test]
    fn valid_health_is_accepted() {
        validate(&response(), 123, 1).expect("valid response");
    }
    #[test]
    fn another_daemon_pid_is_rejected() {
        assert!(matches!(
            validate(&response(), 124, 1),
            Err(LifecycleError::Attention(_))
        ));
    }
    #[test]
    fn actual_protocol_mismatch_is_rejected() {
        assert!(matches!(
            validate(&response(), 123, 2),
            Err(LifecycleError::Incompatible(_))
        ));
    }
    #[test]
    fn malformed_acceptance_is_rejected() {
        let mut value = response();
        value.accepted = false;
        assert!(validate(&value, 123, 1).is_err());
        value.accepted = true;
        value.operation = Some("other".into());
        assert!(validate(&value, 123, 1).is_err());
    }
    #[test]
    fn invalid_cli_does_not_fall_back_to_defaults() {
        assert!(options(&[]).is_err());
        assert!(options(&["--expected-pid".into(), "0".into()]).is_err());
        assert!(options(&["--unknown".into(), "1".into()]).is_err());
        assert!(
            options(&[
                "--expected-pid".into(),
                "1".into(),
                "--expected-pid".into(),
                "2".into()
            ])
            .is_err()
        );
    }
    #[test]
    fn repository_runtime_is_rejected_without_environment_hint() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let args = vec![
            "--expected-pid".into(),
            "1".into(),
            "--expected-protocol".into(),
            "1".into(),
            "--runtime-root".into(),
            root.display().to_string(),
        ];
        assert!(matches!(
            options(&args),
            Err(LifecycleError::UnsafeRuntimePath(_))
        ));
    }
}
