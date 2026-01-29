// go2rtc Manager Service
// Manages the go2rtc WebRTC server process lifecycle

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use std::io::Write;

use super::go2rtc_client::Go2RtcClient;

const DEFAULT_GO2RTC_PORT: u16 = 1984;
const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(5);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Manages the go2rtc WebRTC server process
pub struct Go2RtcManager {
    child: RwLock<Option<Child>>,
    client: Go2RtcClient,
    port: u16,
    is_available: AtomicBool,
    binary_path: Option<PathBuf>,
}

impl Go2RtcManager {
    /// Create a new Go2RtcManager with the default port
    pub fn new() -> Self {
        Self::with_port(DEFAULT_GO2RTC_PORT)
    }

    /// Create a new Go2RtcManager with a custom port
    pub fn with_port(port: u16) -> Self {
        let base_url = format!("http://127.0.0.1:{}", port);
        Self {
            child: RwLock::new(None),
            client: Go2RtcClient::with_url(base_url),
            port,
            is_available: AtomicBool::new(false),
            binary_path: None,
        }
    }

    /// Set the path to the go2rtc binary
    pub fn with_binary_path(mut self, path: PathBuf) -> Self {
        self.binary_path = Some(path);
        self
    }

    /// Find the go2rtc binary path
    /// Checks in order: env var, explicit path, sidecar location, dev location, PATH
    pub fn find_binary(&self) -> Option<PathBuf> {
        let sidecar_names = if cfg!(target_os = "windows") {
            vec!["go2rtc.exe", "go2rtc-x86_64-pc-windows-msvc.exe"]
        } else if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                vec!["go2rtc", "go2rtc-aarch64-apple-darwin"]
            } else {
                vec!["go2rtc", "go2rtc-x86_64-apple-darwin"]
            }
        } else {
            if cfg!(target_arch = "aarch64") {
                vec!["go2rtc", "go2rtc-aarch64-unknown-linux-gnu"]
            } else {
                vec!["go2rtc", "go2rtc-x86_64-unknown-linux-gnu"]
            }
        };

        // 1. Check SPIRITSTREAM_GO2RTC_PATH environment variable
        if let Ok(env_path) = std::env::var("SPIRITSTREAM_GO2RTC_PATH") {
            let path = PathBuf::from(&env_path);
            if path.exists() {
                log::debug!("Found go2rtc via SPIRITSTREAM_GO2RTC_PATH: {:?}", path);
                return Some(path);
            }
        }

        // 2. Check explicit path
        if let Some(ref path) = self.binary_path {
            if path.exists() {
                return Some(path.clone());
            }
        }

        // 3. Check sidecar location (next to executable)
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                for name in &sidecar_names {
                    let sidecar_path = exe_dir.join(name);
                    if sidecar_path.exists() {
                        log::debug!("Found go2rtc as sidecar: {:?}", sidecar_path);
                        return Some(sidecar_path);
                    }
                }

                // 4. Check development location relative to server binary
                // In dev mode, exe is at target/debug/spiritstream-server
                // Binary is at apps/desktop/src-tauri/binaries/
                if let Some(target_dir) = exe_dir.parent() {
                    if let Some(project_root) = target_dir.parent() {
                        let dev_binaries = project_root.join("apps/desktop/src-tauri/binaries");
                        for name in &sidecar_names {
                            let dev_path = dev_binaries.join(name);
                            if dev_path.exists() {
                                log::debug!("Found go2rtc in dev location: {:?}", dev_path);
                                return Some(dev_path);
                            }
                        }
                    }
                }
            }
        }

        // 5. Check current working directory's Tauri binaries (for standalone server)
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_binaries = cwd.join("apps/desktop/src-tauri/binaries");
            for name in &sidecar_names {
                let cwd_path = cwd_binaries.join(name);
                if cwd_path.exists() {
                    log::debug!("Found go2rtc relative to cwd: {:?}", cwd_path);
                    return Some(cwd_path);
                }
            }
        }

        // 6. Check PATH
        if let Ok(which_path) = which::which("go2rtc") {
            log::debug!("Found go2rtc in PATH: {:?}", which_path);
            return Some(which_path);
        }

        log::warn!("go2rtc binary not found in any location");
        None
    }

    /// Start the go2rtc process
    pub async fn start(&self) -> Result<(), String> {
        // Check if already running
        if self.is_available() {
            log::debug!("go2rtc is already running");
            return Ok(());
        }

        // Find binary
        let binary_path = self.find_binary()
            .ok_or_else(|| "go2rtc binary not found".to_string())?;

        log::info!("Starting go2rtc from: {:?}", binary_path);

        // Create a temporary config file - go2rtc requires a file to allow dynamic stream registration
        // Using inline config with -c disables the ability to add streams via API
        let config_content = format!(r#"api:
  listen: ":{}"
  origin: "*"
rtsp:
  listen: ":{}"
webrtc:
  listen: ":{}"
ffmpeg:
  bin: ffmpeg
log:
  level: info
"#,
            self.port,
            self.port + 1,  // RTSP on port+1
            self.port + 2   // WebRTC on port+2
        );

        // Write config to temp file
        let config_path = std::env::temp_dir().join(format!("go2rtc-{}.yaml", self.port));
        let mut config_file = std::fs::File::create(&config_path)
            .map_err(|e| format!("Failed to create go2rtc config file: {}", e))?;
        config_file.write_all(config_content.as_bytes())
            .map_err(|e| format!("Failed to write go2rtc config: {}", e))?;

        log::debug!("go2rtc config written to: {:?}", config_path);

        // Spawn the process with the config file
        let child = Command::new(&binary_path)
            .arg("-c")
            .arg(&config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Failed to spawn go2rtc: {}", e))?;

        *self.child.write().await = Some(child);

        // Wait for health check to pass
        let start_time = std::time::Instant::now();
        loop {
            if start_time.elapsed() > STARTUP_TIMEOUT {
                self.stop().await;
                return Err("go2rtc startup timeout".to_string());
            }

            if self.client.health_check().await {
                self.is_available.store(true, Ordering::SeqCst);
                log::info!("go2rtc started successfully on port {}", self.port);
                return Ok(());
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Stop the go2rtc process
    pub async fn stop(&self) {
        self.is_available.store(false, Ordering::SeqCst);

        let mut child_guard = self.child.write().await;
        if let Some(mut child) = child_guard.take() {
            log::info!("Stopping go2rtc process");
            let _ = child.kill().await;
        }
    }

    /// Check if go2rtc is available
    pub fn is_available(&self) -> bool {
        self.is_available.load(Ordering::SeqCst)
    }

    /// Get the go2rtc client
    pub fn client(&self) -> &Go2RtcClient {
        &self.client
    }

    /// Get the API port go2rtc is running on
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the RTSP port (API port + 1)
    pub fn rtsp_port(&self) -> u16 {
        self.port + 1
    }

    /// Register a source for WebRTC streaming using go2rtc's native source format
    pub async fn register_source(&self, source_id: &str, go2rtc_source: &str) -> Result<String, String> {
        if !self.is_available() {
            return Err("go2rtc is not available".to_string());
        }

        self.client.register_stream(source_id, go2rtc_source).await
    }

    /// Register a camera source using go2rtc's native ffmpeg:device source
    pub async fn register_camera(&self, source_id: &str, device_index: u32) -> Result<String, String> {
        if !self.is_available() {
            return Err("go2rtc is not available".to_string());
        }

        // Use go2rtc's native ffmpeg:device source type
        // Format: ffmpeg:device?video={index}&video_size={WxH}&framerate={fps}#video={codec}
        let source = if cfg!(target_os = "macos") {
            // macOS uses AVFoundation indices
            format!("ffmpeg:device?video={}&video_size=1280x720&framerate=30#video=h264", device_index)
        } else if cfg!(target_os = "windows") {
            // Windows uses DirectShow device index
            format!("ffmpeg:device?video={}&video_size=1280x720&framerate=30#video=h264", device_index)
        } else {
            // Linux uses V4L2 device index
            format!("ffmpeg:device?video={}&video_size=1280x720&framerate=30#video=h264", device_index)
        };

        self.register_source(source_id, &source).await
    }

    /// Register a screen capture source
    pub async fn register_screen(&self, source_id: &str, display_id: u32) -> Result<String, String> {
        if !self.is_available() {
            return Err("go2rtc is not available".to_string());
        }

        // Screen capture requires different handling per platform
        // On macOS, AVFoundation can capture screens - the display index comes after camera indices
        // On Windows/Linux, we may need different approaches
        let source = if cfg!(target_os = "macos") {
            // macOS: screen capture devices are listed after cameras in AVFoundation
            // Typically screen 0 would be at a higher index
            format!("ffmpeg:device?video={}&framerate=30#video=h264", display_id)
        } else if cfg!(target_os = "windows") {
            // Windows: gdigrab for desktop capture - use pipe transport
            format!("ffmpeg:desktop#video=h264")
        } else {
            // Linux: x11grab for desktop capture
            format!("ffmpeg:display#video=h264")
        };

        self.register_source(source_id, &source).await
    }

    /// Unregister a source
    pub async fn unregister_source(&self, source_id: &str) -> Result<(), String> {
        if !self.is_available() {
            return Ok(()); // Nothing to do if not available
        }

        self.client.unregister_stream(source_id).await
    }

    /// Perform a health check
    pub async fn health_check(&self) -> bool {
        if !self.is_available.load(Ordering::SeqCst) {
            return false;
        }

        let healthy = tokio::time::timeout(HEALTH_CHECK_TIMEOUT, self.client.health_check())
            .await
            .unwrap_or(false);

        if !healthy {
            self.is_available.store(false, Ordering::SeqCst);
        }

        healthy
    }
}

impl Default for Go2RtcManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Go2RtcManager {
    fn drop(&mut self) {
        // Attempt to kill the process synchronously on drop
        // This is best-effort since we can't await in Drop
        if let Ok(mut guard) = self.child.try_write() {
            if let Some(ref mut child) = *guard {
                // Send SIGKILL on Unix, TerminateProcess on Windows
                let _ = child.start_kill();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let manager = Go2RtcManager::new();
        assert_eq!(manager.port(), DEFAULT_GO2RTC_PORT);
        assert!(!manager.is_available());
    }

    #[test]
    fn test_custom_port() {
        let manager = Go2RtcManager::with_port(9000);
        assert_eq!(manager.port(), 9000);
    }
}
