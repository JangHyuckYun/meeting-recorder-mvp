//! ASR v2 domain types. The domain is the single source of truth for caption state; any
//! module depending on caption lifecycle re-exports these through `crate::models`.

pub mod domain;
pub mod live_stt;
pub mod server_proxy;