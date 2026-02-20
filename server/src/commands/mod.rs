// SpiritStream Commands
// Domain command modules extracted from main.rs invoke_command()

mod system;
pub mod profile;
pub mod streaming;
pub mod settings;
pub mod theme;
pub mod device;
pub mod source;
pub mod scene;
pub mod layer;
pub mod mixer;
pub mod capture;

pub use system::*;
