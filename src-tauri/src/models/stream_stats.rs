// StreamStats Model
// Real-time FFmpeg statistics for stream monitoring

use serde::{Deserialize, Serialize};

/// Real-time statistics from FFmpeg output
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StreamStats {
    /// Output group ID this stats belong to
    pub group_id: String,

    /// Current frame number
    pub frame: u64,

    /// Frames per second
    pub fps: f64,

    /// Current bitrate in kbps
    pub bitrate: f64,

    /// Encoding speed (e.g., 1.0x = real-time)
    pub speed: f64,

    /// Total size in bytes
    pub size: u64,

    /// Elapsed time in seconds
    pub time: f64,

    /// Number of dropped frames
    pub dropped_frames: u64,

    /// Number of duplicate frames
    pub dup_frames: u64,
}

impl StreamStats {
    /// Create new stats for a group
    pub fn new(group_id: String) -> Self {
        Self {
            group_id,
            ..Default::default()
        }
    }

    /// Parse FFmpeg stderr line for statistics
    /// FFmpeg outputs lines like:
    /// frame= 1234 fps= 60 q=28.0 size=   12345kB time=00:01:23.45 bitrate=1234.5kbits/s speed=1.0x
    pub fn parse_line(&mut self, line: &str) -> bool {
        let mut parsed = false;

        // Parse frame count
        if let Some(frame) = Self::extract_value(line, "frame=") {
            if let Ok(f) = frame.parse::<u64>() {
                self.frame = f;
                parsed = true;
            }
        }

        // Parse FPS
        if let Some(fps) = Self::extract_value(line, "fps=") {
            if let Ok(f) = fps.parse::<f64>() {
                self.fps = f;
                parsed = true;
            }
        }

        // Parse bitrate (remove "kbits/s" suffix)
        if let Some(bitrate_str) = Self::extract_value(line, "bitrate=") {
            let bitrate = bitrate_str.replace("kbits/s", "").replace("kbit/s", "");
            if let Ok(b) = bitrate.trim().parse::<f64>() {
                self.bitrate = b;
                parsed = true;
            }
        }

        // Parse speed (remove "x" suffix)
        if let Some(speed_str) = Self::extract_value(line, "speed=") {
            let speed = speed_str.replace('x', "");
            if let Ok(s) = speed.trim().parse::<f64>() {
                self.speed = s;
                parsed = true;
            }
        }

        // Parse size (remove "kB" suffix and convert to bytes)
        if let Some(size_str) = Self::extract_value(line, "size=") {
            let size = size_str.replace("kB", "").replace("KB", "").replace("mB", "").replace("MB", "");
            if let Ok(s) = size.trim().parse::<f64>() {
                // Assume kB if contains kB
                self.size = if size_str.contains("kB") || size_str.contains("KB") {
                    (s * 1024.0) as u64
                } else if size_str.contains("mB") || size_str.contains("MB") {
                    (s * 1024.0 * 1024.0) as u64
                } else {
                    s as u64
                };
                parsed = true;
            }
        }

        // Parse time (format: HH:MM:SS.ms)
        if let Some(time_str) = Self::extract_value(line, "time=") {
            if let Some(seconds) = Self::parse_time(&time_str) {
                self.time = seconds;
                parsed = true;
            }
        }

        // Parse dropped frames
        if let Some(drop_str) = Self::extract_value(line, "drop=") {
            if let Ok(d) = drop_str.parse::<u64>() {
                self.dropped_frames = d;
                parsed = true;
            }
        }

        // Parse duplicate frames
        if let Some(dup_str) = Self::extract_value(line, "dup=") {
            if let Ok(d) = dup_str.parse::<u64>() {
                self.dup_frames = d;
                parsed = true;
            }
        }

        parsed
    }

    /// Extract value after a key from FFmpeg output
    fn extract_value(line: &str, key: &str) -> Option<String> {
        let start = line.find(key)?;
        let value_start = start + key.len();
        let rest = &line[value_start..];

        // Find the end of the value (next space or end of string)
        let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
        Some(rest[..end].trim().to_string())
    }

    /// Parse time string (HH:MM:SS.ms) to seconds
    fn parse_time(time_str: &str) -> Option<f64> {
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 3 {
            return None;
        }

        let hours: f64 = parts[0].parse().ok()?;
        let minutes: f64 = parts[1].parse().ok()?;
        let seconds: f64 = parts[2].parse().ok()?;

        Some(hours * 3600.0 + minutes * 60.0 + seconds)
    }
}
