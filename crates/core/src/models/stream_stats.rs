// StreamStats Model
// Real-time FFmpeg statistics for stream monitoring

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Real-time statistics from FFmpeg output
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct StreamStats {
    /// Output group ID this stats belong to
    pub group_id: String,

    /// Current frame number. JSON `number` (well below 2^53 for any
    /// realistic stream duration).
    #[ts(type = "number")]
    pub frame: u64,

    /// Frames per second
    pub fps: f64,

    /// Current bitrate in kbps
    pub bitrate: f64,

    /// Encoding speed (e.g., 1.0x = real-time)
    pub speed: f64,

    /// Total size in bytes. JSON `number` (2^53 bytes = ~9 PB —
    /// indistinguishable from infinite for any single broadcast).
    #[ts(type = "number")]
    pub size: u64,

    /// Elapsed time in seconds
    pub time: f64,

    /// Number of dropped frames
    #[ts(type = "number")]
    pub dropped_frames: u64,

    /// Number of duplicate frames
    #[ts(type = "number")]
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

        num_str
            .trim()
            .parse::<f64>()
            .ok()
            .map(|v| (v * scale) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::StreamStats;

    #[test]
    fn new_seeds_group_id_with_zeroed_defaults() {
        let s = StreamStats::new("g1".into());
        assert_eq!(s.group_id, "g1");
        assert_eq!(s.frame, 0);
        assert_eq!(s.fps, 0.0);
        assert_eq!(s.size, 0);
    }

    #[test]
    fn parse_line_reads_a_full_ffmpeg_stderr_line() {
        let mut s = StreamStats::new("g1".into());
        let parsed = s.parse_line(
            "frame= 1234 fps= 60 q=28.0 size=   12345kB time=00:01:23.45 bitrate=1234.5kbits/s speed=1.5x",
        );
        assert!(parsed);
        assert_eq!(s.frame, 1234);
        assert_eq!(s.fps, 60.0);
        // 12345 kB is parsed as KiB (1024 multiplier).
        assert_eq!(s.size, 12345 * 1024);
        assert!((s.time - (60.0 + 23.45)).abs() < 1e-6);
        assert!((s.bitrate - 1234.5).abs() < 1e-6);
        assert!((s.speed - 1.5).abs() < 1e-6);
    }

    #[test]
    fn parse_line_handles_progress_microsecond_and_total_size_keys() {
        let mut s = StreamStats::new("g1".into());
        assert!(s.parse_line("total_size=2048 out_time_us=2500000 drop_frames=3 dup_frames=7"));
        assert_eq!(s.size, 2048);
        assert!((s.time - 2.5).abs() < 1e-6);
        assert_eq!(s.dropped_frames, 3);
        assert_eq!(s.dup_frames, 7);
    }

    #[test]
    fn parse_bitrate_understands_mbit_and_na() {
        let mut s = StreamStats::new("g1".into());
        assert!(s.parse_line("bitrate=2.0mbits/s"));
        assert!((s.bitrate - 2000.0).abs() < 1e-6);

        // "N/A" leaves the prior value untouched and contributes no parse.
        let mut idle = StreamStats::new("g1".into());
        assert!(!idle.parse_line("bitrate=N/A"));
        assert_eq!(idle.bitrate, 0.0);
    }

    #[test]
    fn parse_line_returns_false_for_unrecognized_lines() {
        let mut s = StreamStats::new("g1".into());
        assert!(!s.parse_line("Press [q] to stop, [?] for help"));
    }

    #[test]
    fn parse_bitrate_accepts_every_unit_suffix() {
        let cases: &[(&str, f64)] = &[
            ("bitrate=500kbit/s", 500.0),
            ("bitrate=500kb/s", 500.0),
            ("bitrate=500kbps", 500.0),
            ("bitrate=2mbit/s", 2000.0),
            ("bitrate=2mb/s", 2000.0),
            ("bitrate=2mbps", 2000.0),
            ("bitrate=8000bits/s", 8.0),
            ("bitrate=750", 750.0),
        ];
        for (line, want) in cases {
            let mut s = StreamStats::new("g".into());
            assert!(s.parse_line(line), "should parse {line}");
            assert!(
                (s.bitrate - want).abs() < 1e-6,
                "{line} → {} (want {want})",
                s.bitrate
            );
        }
    }

    #[test]
    fn parse_size_accepts_every_unit_suffix() {
        let cases: &[(&str, u64)] = &[
            ("size=4KiB", 4 * 1024),
            ("size=2MiB", 2 * 1024 * 1024),
            ("size=2MB", 2 * 1024 * 1024),
            ("size=512B", 512),
            ("size=999", 999),
            ("size=N/A", 0),
        ];
        for (line, want) in cases {
            let mut s = StreamStats::new("g".into());
            s.parse_line(line);
            assert_eq!(s.size, *want, "{line} → {} (want {want})", s.size);
        }
    }

    #[test]
    fn parse_line_handles_out_time_and_out_time_ms_keys() {
        let mut a = StreamStats::new("g".into());
        assert!(a.parse_line("out_time=01:00:30.0"));
        assert!((a.time - 3630.0).abs() < 1e-6);

        let mut b = StreamStats::new("g".into());
        assert!(b.parse_line("out_time_ms=4500000"));
        assert!((b.time - 4.5).abs() < 1e-6);
    }

    #[test]
    fn parse_line_handles_short_drop_and_dup_keys() {
        let mut s = StreamStats::new("g".into());
        assert!(s.parse_line("drop=5 dup=9"));
        assert_eq!(s.dropped_frames, 5);
        assert_eq!(s.dup_frames, 9);
    }

    #[test]
    fn parse_time_rejects_malformed_clock() {
        // Only HH:MM:SS (three parts) is accepted; anything else → no parse.
        let mut s = StreamStats::new("g".into());
        assert!(!s.parse_line("time=01:30"));
        assert_eq!(s.time, 0.0);
    }
}
