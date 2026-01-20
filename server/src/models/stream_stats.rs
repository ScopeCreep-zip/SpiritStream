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

        // Parse bitrate
        if let Some(bitrate_str) = Self::extract_value(line, "bitrate=") {
            if let Some(b) = Self::parse_bitrate_kbps(&bitrate_str) {
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

        // Parse size and convert to bytes
        if let Some(size_str) = Self::extract_value(line, "size=") {
            if let Some(bytes) = Self::parse_size_bytes(&size_str) {
                self.size = bytes;
                parsed = true;
            }
        }

        // Parse total size from progress output (bytes)
        if let Some(size_str) = Self::extract_value(line, "total_size=") {
            if let Ok(s) = size_str.trim().parse::<u64>() {
                self.size = s;
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

        // Parse progress time (format: HH:MM:SS.ms)
        if let Some(time_str) = Self::extract_value(line, "out_time=") {
            if let Some(seconds) = Self::parse_time(&time_str) {
                self.time = seconds;
                parsed = true;
            }
        }

        // Parse progress time in microseconds
        if let Some(time_str) = Self::extract_value(line, "out_time_ms=") {
            if let Ok(us) = time_str.trim().parse::<u64>() {
                self.time = us as f64 / 1_000_000.0;
                parsed = true;
            }
        }

        // Parse progress time in microseconds (alternate key)
        if let Some(time_str) = Self::extract_value(line, "out_time_us=") {
            if let Ok(us) = time_str.trim().parse::<u64>() {
                self.time = us as f64 / 1_000_000.0;
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

        if let Some(drop_str) = Self::extract_value(line, "drop_frames=") {
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

        if let Some(dup_str) = Self::extract_value(line, "dup_frames=") {
            if let Ok(d) = dup_str.parse::<u64>() {
                self.dup_frames = d;
                parsed = true;
            }
        }

        // Fallback: compute average bitrate from size and time if FFmpeg doesn't report one.
        if self.bitrate == 0.0 && self.size > 0 && self.time > 0.0 {
            let avg_kbps = (self.size as f64 * 8.0) / 1000.0 / self.time;
            if avg_kbps.is_finite() && avg_kbps > 0.0 {
                self.bitrate = avg_kbps;
                parsed = true;
            }
        }

        parsed
    }

    /// Extract value after a key from FFmpeg output
    fn extract_value(line: &str, key: &str) -> Option<String> {
        let start = line.find(key)?;
        let value_start = start + key.len();
        let rest = line[value_start..].trim_start();

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

    /// Parse bitrate string to kbps.
    fn parse_bitrate_kbps(value: &str) -> Option<f64> {
        let trimmed = value.trim();
        if trimmed.eq_ignore_ascii_case("N/A") {
            return None;
        }

        let lower = trimmed.to_ascii_lowercase();
        let (num_str, scale) = if let Some(v) = lower.strip_suffix("kbits/s") {
            (v, 1.0)
        } else if let Some(v) = lower.strip_suffix("kbit/s") {
            (v, 1.0)
        } else if let Some(v) = lower.strip_suffix("kb/s") {
            (v, 1.0)
        } else if let Some(v) = lower.strip_suffix("kbps") {
            (v, 1.0)
        } else if let Some(v) = lower.strip_suffix("mbits/s") {
            (v, 1000.0)
        } else if let Some(v) = lower.strip_suffix("mbit/s") {
            (v, 1000.0)
        } else if let Some(v) = lower.strip_suffix("mb/s") {
            (v, 1000.0)
        } else if let Some(v) = lower.strip_suffix("mbps") {
            (v, 1000.0)
        } else if let Some(v) = lower.strip_suffix("bits/s") {
            (v, 1.0 / 1000.0)
        } else {
            (trimmed, 1.0)
        };

        num_str.trim().parse::<f64>().ok().map(|v| v * scale)
    }

    /// Parse size string to bytes.
    fn parse_size_bytes(value: &str) -> Option<u64> {
        let trimmed = value.trim();
        if trimmed.eq_ignore_ascii_case("N/A") {
            return None;
        }

        let lower = trimmed.to_ascii_lowercase();
        let (num_str, scale) = if let Some(v) = lower.strip_suffix("kib") {
            (v, 1024.0)
        } else if let Some(v) = lower.strip_suffix("kb") {
            (v, 1024.0)
        } else if let Some(v) = lower.strip_suffix("mib") {
            (v, 1024.0 * 1024.0)
        } else if let Some(v) = lower.strip_suffix("mb") {
            (v, 1024.0 * 1024.0)
        } else if let Some(v) = lower.strip_suffix('b') {
            (v, 1.0)
        } else {
            (trimmed, 1.0)
        };

        num_str.trim().parse::<f64>().ok().map(|v| (v * scale) as u64)
    }
}
