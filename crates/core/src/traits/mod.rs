//! Cross-cutting trait contracts that decouple core services from any specific
//! transport, storage, or platform. Concrete implementations live in transport
//! crates (`transport-http`, `transport-cli`) or in shell crates (the Tauri 2
//! mobile shell, for example, supplies its own `MediaProcessor` impl).
//!
//! See `.claude/rules/architecture.md` for the design rationale.

pub mod clock;
pub mod event_sink;
pub mod identity;
pub mod media_processor;
pub mod secret_store;
pub mod transport;

pub use clock::Clock;
pub use event_sink::EventSink;
pub use identity::{Credential, Identity, IdentityProvider};
pub use media_processor::{MediaProcessor, StreamHandle};
pub use secret_store::SecretStore;
pub use transport::Transport;
