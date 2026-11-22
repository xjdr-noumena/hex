//! Shim for openai/codex's `codex-utils-pty` (Apache-2.0), reduced to `spawn_pipe_process`, the
//! only function the voice client uses. It keeps the behaviour the client relies on
//! (codex-rs/utils/pty/src/pipe.rs at 943a21829d):
//! - the child gets exactly the given environment (nothing inherited);
//! - stdin is fed from an mpsc channel; stdout and stderr arrive in chunks of at most
//!   8,192 bytes (`MessageReader` depends on that bound);
//! - the child runs in its own process group (Unix) and dies with the parent on Linux;
//!   on Windows it gets no console window;
//! - the exit code arrives on a oneshot, and `terminate` kills the process (group).

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;

use anyhow::Result;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

pub struct ProcessHandle {
    writer: mpsc::Sender<Vec<u8>>,
    kill: Option<oneshot::Sender<()>>,
    #[cfg(unix)]
    pgid: Option<i32>,
}

impl ProcessHandle {
    pub fn writer_sender(&self) -> mpsc::Sender<Vec<u8>> {
        self.writer.clone()
    }

    pub fn terminate(&mut self) {
        #[cfg(unix)]
        if let Some(pgid) = self.pgid {
            // SAFETY: signalling our own child's process group.
            unsafe {
                libc::killpg(pgid, libc::SIGKILL);
            }
        }
        if let Some(kill) = self.kill.take() {
            let _ = kill.send(());
        }
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        self.terminate();
    }
}

pub struct SpawnedProcess {
    pub session: ProcessHandle,
    pub stdout_rx: mpsc::Receiver<Vec<u8>>,
    pub stderr_rx: mpsc::Receiver<Vec<u8>>,
    pub exit_rx: oneshot::Receiver<i32>,
}

async fn pump<R: AsyncRead + Unpin>(mut reader: R, tx: mpsc::Sender<Vec<u8>>) {
    let mut buf = vec![0u8; 8_192];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if tx.send(buf[..n].to_vec()).await.is_err() {
                    // Receiver dropped (stderr is discarded by the client): keep draining.
                    let mut sink = tokio::io::sink();
                    let _ = tokio::io::copy(&mut reader, &mut sink).await;
                    break;
                }
            }
        }
    }
}

/// Spawn `program` with piped stdio. `arg0` and `inherited_fds` are accepted for signature
/// compatibility; the voice client passes `None` and `[]`.
pub async fn spawn_pipe_process(
    program: impl AsRef<OsStr>,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    _arg0: &Option<String>,
    _inherited_fds: &[i32],
) -> Result<SpawnedProcess> {
    let mut command = Command::new(program.as_ref());
    command
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    {
        command.process_group(0);
        #[cfg(target_os = "linux")]
        // SAFETY: prctl is async-signal-safe and only affects the child being spawned.
        unsafe {
            command.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn()?;
    #[cfg(unix)]
    let pgid = child.id().map(|id| id as i32);

    let (writer_tx, mut writer_rx) = mpsc::channel::<Vec<u8>>(128);
    let (stdout_tx, stdout_rx) = mpsc::channel::<Vec<u8>>(128);
    let (stderr_tx, stderr_rx) = mpsc::channel::<Vec<u8>>(128);
    if let Some(mut stdin) = child.stdin.take() {
        tokio::spawn(async move {
            while let Some(bytes) = writer_rx.recv().await {
                if stdin.write_all(&bytes).await.is_err() || stdin.flush().await.is_err() {
                    break;
                }
            }
        });
    }
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(pump(stdout, stdout_tx));
    }
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(pump(stderr, stderr_tx));
    }
    let (exit_tx, exit_rx) = oneshot::channel::<i32>();
    let (kill_tx, kill_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let status = tokio::select! {
            status = child.wait() => status,
            _ = kill_rx => {
                let _ = child.start_kill();
                child.wait().await
            }
        };
        let code = match status {
            Ok(status) => status.code().unwrap_or(-1),
            Err(_) => -1,
        };
        let _ = exit_tx.send(code);
    });
    Ok(SpawnedProcess {
        session: ProcessHandle {
            writer: writer_tx,
            kill: Some(kill_tx),
            #[cfg(unix)]
            pgid,
        },
        stdout_rx,
        stderr_rx,
        exit_rx,
    })
}
