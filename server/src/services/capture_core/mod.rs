// Capture Core — shared infrastructure for all capture services
//
// Extracts duplicated patterns (session management, timeout helpers, frame types)
// from screen_capture, camera_capture, audio_capture, and h264_capture into
// reusable generic components.

pub mod session;
pub mod timeout;
pub mod frame;
