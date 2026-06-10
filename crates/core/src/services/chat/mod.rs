mod endpoints;
mod facebook;
mod kick;
mod platform;
mod tiktok;
mod trovo;
pub(crate) mod twitch;
mod youtube;

#[cfg(test)]
mod connector_tests;
#[cfg(test)]
mod integration_harness;

pub use endpoints::ChatEndpoints;
pub use facebook::FacebookConnector;
pub use kick::KickConnector;
pub use platform::{BoxedPlatform, ChatPlatform, PlatformResult};
pub use tiktok::TikTokConnector;
pub use trovo::TrovoConnector;
pub use twitch::TwitchConnector;
pub use youtube::YouTubeConnector;
