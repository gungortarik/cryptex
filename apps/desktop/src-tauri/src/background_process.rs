//! Platform process settings for Desktop-owned background daemon commands.

use std::ffi::OsStr;
use std::process::Command;

pub(crate) fn command(program: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // Redirecting stdio alone does not suppress a console for a console
        // executable launched by the Windows GUI subsystem.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::command;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetConsoleWindow() -> *mut std::ffi::c_void;
    }

    #[test]
    fn child_console_probe() {
        if std::env::var_os("CRYPTEX_TEST_BACKGROUND_CONSOLE").is_some() {
            // This API takes no pointer arguments and is safe to query here.
            assert!(unsafe { GetConsoleWindow() }.is_null());
            if let Some(report) = std::env::var_os("CRYPTEX_TEST_BACKGROUND_REPORT") {
                std::fs::write(report, "no-console").expect("write console probe report");
            }
            println!("cryptex_child_has_no_console");
        }
    }

    #[test]
    fn background_child_has_no_console_and_keeps_captured_output() {
        let output = command(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "background_process::tests::child_console_probe",
                "--nocapture",
            ])
            .env("CRYPTEX_TEST_BACKGROUND_CONSOLE", "1")
            .output()
            .expect("launch background test child");
        assert!(output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("cryptex_child_has_no_console"));
    }

    #[test]
    fn spawned_background_child_has_no_console_with_discarded_output() {
        let report = std::env::temp_dir().join(format!(
            "cryptex-console-probe-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let status = command(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "background_process::tests::child_console_probe",
                "--nocapture",
            ])
            .env("CRYPTEX_TEST_BACKGROUND_CONSOLE", "1")
            .env("CRYPTEX_TEST_BACKGROUND_REPORT", &report)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn background test child")
            .wait()
            .expect("wait for background child");
        let observation = std::fs::read_to_string(&report);
        let _ = std::fs::remove_file(&report);
        assert!(status.success(), "background child exited {status}");
        assert_eq!(observation.expect("child console report"), "no-console");
    }
}
