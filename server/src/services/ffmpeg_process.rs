/// Lightweight wrapper around `std::process::Child` that guarantees FFmpeg's
/// stderr is always consumed, preventing the 64 KB OS pipe buffer deadlock.
///
/// When FFmpeg writes to stderr and nothing reads it, the pipe buffer fills up.
/// FFmpeg then blocks on the stderr write, and if it's also waiting on stdin
/// data the whole pipeline deadlocks. This is the root cause of WHEP 500 errors
/// when multiple screen captures are active.
///
/// Usage:
/// ```ignore
/// let proc = FfmpegProcess::spawn("ffmpeg", &args, "h264-src1")?;
/// let mut stdin = proc.take_stdin().unwrap();
/// // ... write frames to stdin ...
/// drop(stdin);
/// proc.wait();
/// ```

use std::collections::VecDeque;
use std::io::BufRead;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc;

use super::process_util::configure_hidden_window;

const STDERR_HISTORY_LINES: usize = 20;

/// Information about an FFmpeg process exit, including recent stderr output.
pub struct FfmpegExitInfo {
    /// Last N lines of stderr output (for error diagnostics)
    pub recent_stderr: Vec<String>,
    /// Whether any stderr line contained "error" (case-insensitive)
    pub had_error: bool,
}

/// A managed FFmpeg child process with automatic stderr draining.
pub struct FfmpegProcess {
    child: Child,
    exit_rx: mpsc::Receiver<FfmpegExitInfo>,
    /// Handle kept alive to ensure the stderr thread runs; dropped on `FfmpegProcess` drop.
    _stderr_thread: std::thread::JoinHandle<()>,
}

impl FfmpegProcess {
    /// Spawn an FFmpeg process with automatic stderr consumption.
    ///
    /// - `program`: Path to the FFmpeg binary.
    /// - `args`: Command-line arguments.
    /// - `label`: Short identifier for log messages (e.g., `"h264-src1"`).
    /// - `stdin_cfg`: How to configure stdin (`Stdio::piped()` or `Stdio::null()`).
    /// - `stdout_cfg`: How to configure stdout (`Stdio::piped()` or `Stdio::null()`).
    pub fn spawn(
        program: &str,
        args: &[String],
        label: &str,
        stdin_cfg: Stdio,
        stdout_cfg: Stdio,
    ) -> Result<Self, String> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(stdin_cfg)
            .stdout(stdout_cfg)
            .stderr(Stdio::piped());

        configure_hidden_window(&mut cmd);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn FFmpeg ({}): {}", label, e))?;

        let stderr = child
            .stderr
            .take()
            .expect("stderr must be piped (we configured it above)");

        let (exit_tx, exit_rx) = mpsc::channel();
        let thread_label = label.to_string();

        let stderr_thread = std::thread::Builder::new()
            .name(format!("ss-ffmpeg-stderr-{}", label))
            .spawn(move || {
                Self::drain_stderr(stderr, &thread_label, exit_tx);
            })
            .map_err(|e| format!("Failed to spawn stderr thread ({}): {}", label, e))?;

        log::debug!("[FFmpeg:{}] Process started (PID: {})", label, child.id());

        Ok(Self {
            child,
            exit_rx,
            _stderr_thread: stderr_thread,
        })
    }

    /// Take ownership of FFmpeg's stdin handle.
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }

    /// Take ownership of FFmpeg's stdout handle.
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    /// Get the process ID.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Wait for FFmpeg to exit and return the exit status.
    pub fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.child.wait()
    }

    /// Kill the FFmpeg process.
    pub fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill()
    }

    /// Kill the FFmpeg process and wait for it to exit (best-effort).
    pub fn kill_and_wait(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Try to receive exit info (non-blocking).
    /// Returns `Some` after the stderr thread finishes (i.e., FFmpeg closed stderr).
    pub fn try_exit_info(&self) -> Option<FfmpegExitInfo> {
        self.exit_rx.try_recv().ok()
    }

    /// Background thread that drains stderr line-by-line.
    fn drain_stderr(
        stderr: std::process::ChildStderr,
        label: &str,
        exit_tx: mpsc::Sender<FfmpegExitInfo>,
    ) {
        let reader = std::io::BufReader::new(stderr);
        let mut history = VecDeque::with_capacity(STDERR_HISTORY_LINES);
        let mut had_error = false;

        for line in reader.lines() {
            match line {
                Ok(line) => {
                    let lower = line.to_lowercase();
                    if lower.contains("error") || lower.contains("fatal") {
                        log::warn!("[FFmpeg:{}] {}", label, line);
                        had_error = true;
                    } else if lower.contains("warning") || lower.contains("discarding") {
                        log::debug!("[FFmpeg:{}] {}", label, line);
                    } else {
                        log::trace!("[FFmpeg:{}] {}", label, line);
                    }

                    if history.len() >= STDERR_HISTORY_LINES {
                        history.pop_front();
                    }
                    history.push_back(line);
                }
                Err(e) => {
                    log::debug!("[FFmpeg:{}] stderr read error: {}", label, e);
                    break;
                }
            }
        }

        let _ = exit_tx.send(FfmpegExitInfo {
            recent_stderr: history.into(),
            had_error,
        });
    }
}
