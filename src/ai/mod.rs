//! The language-model layer (C9). Bound by `docs/ai-requirements.md`.
//!
//! Only the plain types in this file, the prompt builders and the CP437 filter
//! compile by default. The HTTP client, the worker thread, the headless flag
//! bodies and the fake gateway sit behind the `ai` Cargo feature (R14); without it `Ai::start` always
//! yields `Ai::Off`, so screens and the runner need no `cfg` of their own.
//!
//! Nothing here is reachable from `src/sim` (R3): the step never sees a model.

#[cfg(feature = "ai")]
pub mod client;
#[cfg(feature = "ai")]
pub mod fake;
pub mod headless;
pub mod prompt;
pub mod sanitize;
#[cfg(feature = "ai")]
pub mod worker;
#[cfg(test)]
mod tests;

use std::fmt;

use crate::sim::params::AiConfig;

/// A feature id, stable and `snake_case` in config (`[ai.features]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    Chronicle,
    Designer,
}

impl Feature {
    /// The `[ai.features]` key.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Chronicle => "chronicle",
            Self::Designer => "designer",
        }
    }
}

pub type RequestId = u64;

/// One chat message; `role` is `"system"` or `"user"`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub role: &'static str,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system", content: content.into() }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user", content: content.into() }
    }
}

/// A request the worker performs: one gateway call, never retried (R5).
#[derive(Clone, Debug)]
pub struct Request {
    pub id: RequestId,
    pub feature: Feature,
    pub messages: Vec<Message>,
    /// JSON schema text for structured output (`response_format`), upstream features.
    pub schema: Option<String>,
    /// Stream the reply as `Chunk`s followed by `Done` instead of one `Text`.
    pub stream: bool,
}

/// What a reply carries. A streamed request yields `Chunk`s then `Done`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Output {
    Text(String),
    Chunk(String),
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AiError {
    /// `[ai] enabled = false` or the handle is `Ai::Off`.
    Off,
    /// The binary was built without `--features ai`.
    NotCompiled,
    /// The feature names no model in `[ai.features]`.
    NoModel(&'static str),
    /// The gateway could not be reached; carries the base URL.
    Unreachable(String),
    Timeout,
    /// The gateway answered with an error; carries its message.
    Gateway(String),
    /// The reply could not be used (bad JSON, empty, failed validation).
    Invalid(String),
}

impl fmt::Display for AiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => write!(f, "AI is not enabled in ui.toml"),
            Self::NotCompiled => write!(f, "built without the ai feature"),
            Self::NoModel(k) => write!(f, "no model for ai.features.{k} in ui.toml"),
            Self::Unreachable(url) => write!(f, "gateway unreachable at {url}"),
            Self::Timeout => write!(f, "gateway request timed out"),
            Self::Gateway(m) => write!(f, "gateway error: {m}"),
            Self::Invalid(m) => write!(f, "invalid reply: {m}"),
        }
    }
}

impl std::error::Error for AiError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    pub id: RequestId,
    pub feature: Feature,
    pub result: Result<Output, AiError>,
}

/// Derived by the worker from the `GET /v1/models` probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Off,
    Offline,
    Ready,
}

impl Status {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Offline => "offline",
            Self::Ready => "ready",
        }
    }
}

/// The one handle. `App` (live) or the headless flags construct it from the
/// `[ai]` table of `ui.toml`; screens enqueue with `request` and read replies
/// with `poll` on later frames (R7).
#[derive(Debug)]
pub enum Ai {
    Off,
    #[cfg(feature = "ai")]
    On(worker::Handle),
}

// `missing_const_for_fn`: these are only const-able in the build without the
// `ai` feature, where the `On` arm does not exist.
#[allow(clippy::missing_const_for_fn)]
impl Ai {
    /// `Off` unless the table is enabled and the feature is compiled in.
    pub fn start(config: &AiConfig) -> Self {
        #[cfg(feature = "ai")]
        if config.enabled {
            return Self::On(worker::Handle::start(config.clone()));
        }
        let _ = config;
        Self::Off
    }

    pub const fn is_on(&self) -> bool {
        !matches!(self, Self::Off)
    }

    pub fn status(&self) -> Status {
        match self {
            Self::Off => Status::Off,
            #[cfg(feature = "ai")]
            Self::On(h) => h.status(),
        }
    }

    /// The status-bar note: only `Offline` draws anything (R2).
    pub fn status_note(&self) -> &'static str {
        if self.status() == Status::Offline {
            "AI: offline"
        } else {
            ""
        }
    }

    /// The feature names a model and the master switch is on.
    pub fn feature_on(&self, feature: Feature) -> bool {
        let _ = feature;
        match self {
            Self::Off => false,
            #[cfg(feature = "ai")]
            Self::On(h) => h.config().feature_on(feature.key()),
        }
    }

    /// The model string shown beside a feature in Options.
    pub fn model_of(&self, feature: Feature) -> Option<String> {
        let _ = feature;
        match self {
            Self::Off => None,
            #[cfg(feature = "ai")]
            Self::On(h) => h.config().features.model(feature.key()).map(str::to_string),
        }
    }

    /// Enqueue one gateway call. Returns immediately with the request id.
    pub fn request(&self, feature: Feature, messages: Vec<Message>, schema: Option<String>, stream: bool) -> Result<RequestId, AiError> {
        match self {
            Self::Off => {
                let _ = (feature, messages, schema, stream);
                Err(AiError::Off)
            }
            #[cfg(feature = "ai")]
            Self::On(h) => h.request(feature, messages, schema, stream),
        }
    }

    /// Forget a request: any reply still to come for it is dropped (R7).
    pub fn cancel(&self, id: RequestId) {
        let _ = id;
        match self {
            Self::Off => {}
            #[cfg(feature = "ai")]
            Self::On(h) => h.cancel(id),
        }
    }

    /// Replies that arrived since the last poll, stale ones removed.
    pub fn poll(&self) -> Vec<Reply> {
        match self {
            Self::Off => Vec::new(),
            #[cfg(feature = "ai")]
            Self::On(h) => h.poll(),
        }
    }

    /// Block until the request completes; chunks are concatenated. Headless only.
    pub fn wait(&self, id: RequestId) -> Result<String, AiError> {
        match self {
            Self::Off => {
                let _ = id;
                Err(AiError::Off)
            }
            #[cfg(feature = "ai")]
            Self::On(h) => h.wait(id),
        }
    }
}
