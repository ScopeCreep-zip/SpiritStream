mod platform;
mod stripchat;
mod tiktok;
mod trovo;
mod twitch;
mod youtube;

pub use platform::{BoxedPlatform, ChatPlatform};
pub use stripchat::StripchatConnector;
pub use tiktok::TikTokConnector;
pub use trovo::TrovoConnector;
pub use twitch::TwitchConnector;
pub use youtube::YouTubeConnector;
