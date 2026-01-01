// MagillaStream Commands
// Tauri command handlers for frontend communication

mod profile;
mod stream;
mod system;

pub use profile::*;
pub use stream::*;
pub use system::*;

/// Simple greet command for testing
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to MagillaStream.", name)
}
