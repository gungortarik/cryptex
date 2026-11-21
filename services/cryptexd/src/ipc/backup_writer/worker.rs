use super::{Error, SearchContext, local_anchor, recipient};
use crate::backup_scheduler::{Outcome, journal::DeviceJobs};
use crate::import_staging::export_fs::Directory;
use crate::knowledge_materialization::backup;
use sha2::{Digest, Sha256};
use std::io::{self, Write};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(in crate::ipc) struct Worker {
    stop: Sender<()>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if self
            .worker
            .take()
            .is_some_and(|worker| worker.join().is_err())
        {
            eprintln!("cryptexd: scheduled backup worker ended unexpectedly");
        }
    }
}

pub(in crate::ipc) fn start(context: Option<&SearchContext>) -> io::Result<Option<Worker>> {
    let Some(context) = context.filter(|ctx| ctx.backup_export.is_some()).cloned() else {
        return Ok(None);
    };
    let (stop, stopped) = mpsc::channel();
    let worker = thread::Builder::new()
        .name("cryptex-backup-schedule".into())
        .spawn(move || {
            let mut previous = None;
            loop {
                let error = tick(&context).err();
                if error != previous
                    && let Some(reason) = error
                {
                    eprintln!("cryptexd: scheduled backup held ({reason:?})");
                }
                previous = error;
                if stopped.recv_timeout(Duration::from_secs(10))
                    != Err(mpsc::RecvTimeoutError::Timeout)
                {
                    return;
                }
            }
        })?;
    Ok(Some(Worker {
        stop,
        worker: Some(worker),
    }))
}

fn tick(context: &SearchContext) -> Result<(), Error> {
    let ctx = context.admitted().map_err(|_| Error::Unavailable)?;
    let config = ctx.backup_export.as_ref().ok_or(Error::Unavailable)?;
    let local = local_anchor(&ctx)?;
    let queue = DeviceJobs::new(ctx.store.as_ref(), ctx.environment, &local);
    if !queue.enabled().map_err(|_| Error::History)? {
        return Ok(());
    }
    let _guard = config.busy.try_lock().map_err(|_| Error::Busy)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Unavailable)?;
    let Some(job) = queue.due(now.as_secs()).map_err(|_| Error::History)? else {
        return Ok(());
    };
    for path in [&job.target, &job.checkpoints] {
        match std::fs::metadata(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Ok(meta) if meta.is_dir() => (),
            _ => return Err(Error::UnsafeTarget),
        }
        crate::paths::assert_outside_repository(path).map_err(|_| Error::UnsafeTarget)?;
        backup::independent_folders(&ctx.runtime_root, path).map_err(|_| Error::UnsafeTarget)?;
    }
    backup::independent_folders(&job.target, &job.checkpoints).map_err(|_| Error::UnsafeTarget)?;
    backup::role::DeviceRole {
        local: &local,
        store: ctx.store.as_ref(),
        env: ctx.environment,
    }
    .verify_local_writer(&ctx.runtime_root, &job.target)
    .map_err(|_| Error::Authority)?;
    let recipient = recipient(&ctx)?;
    let checkpoints = Directory::open(&job.checkpoints).map_err(|_| Error::UnsafeTarget)?;
    // The outer timer retries an absent disk. Never hold profile admission while waiting for it.
    let (cancel, stopped) = mpsc::channel();
    drop(cancel);
    let outcome = queue
        .resume(
            &local,
            &stopped,
            || {},
            |staged| {
                let prepared = backup::prepare(
                    &ctx.runtime_root,
                    ctx.store.as_ref(),
                    ctx.environment,
                    job.generation,
                )
                .map_err(|_| io::Error::other("backup history held"))?;
                let cipher = prepared
                    .encrypt(&config.codec, &recipient)
                    .map_err(|_| io::Error::other("backup encryption failed"))?;
                let mut file = std::fs::File::create_new(staged)?;
                io::copy(&mut cipher.reopen()?, &mut file)?;
                let mut digest = Sha256::new();
                io::copy(&mut cipher.reopen()?, &mut digest)?;
                let name = format!("backup-{:020}-{:x}.json", job.generation, digest.finalize());
                let part = format!("{name}.partial");
                let json = serde_json::to_vec(&prepared.selection)?;
                let error = |_| io::Error::other("backup checkpoint publication failed");
                let mut output = checkpoints.open_file(&part, true).map_err(error)?;
                output.write_all(&json)?;
                checkpoints
                    .promote(&part, &output, &checkpoints, &name)
                    .map_err(error)
            },
        )
        .map_err(|_| Error::Uncertain)?;
    match outcome {
        Some(Outcome::Published | Outcome::Stopped) => Ok(()),
        _ => Err(Error::Authority),
    }
}
