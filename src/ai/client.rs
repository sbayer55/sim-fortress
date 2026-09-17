//! One blocking HTTP client speaking OpenAI-style chat completions to the
//! gateway (ai-requirements R5). One request per call, no retries; per-request
//! `fallbacks` in the body are configuration, not code.

use std::io::{BufRead, BufReader};
use std::time::Duration;

use serde_json::{json, Value};

use super::{AiError, Message};
use crate::sim::params::AiConfig;

/// Request header naming the feature, so the gateway (and the fake) can route
/// or record by feature without parsing the prompt.
pub const FEATURE_HEADER: &str = "x-simf-feature";

#[derive(Clone, Debug)]
pub struct Client {
    agent: ureq::Agent,
    base: String,
    token: String,
}

/// One chat call.
#[derive(Debug)]
pub struct ChatCall<'a> {
    pub model: &'a str,
    pub fallbacks: &'a [String],
    pub feature: &'static str,
    pub messages: &'a [Message],
    pub schema: Option<&'a str>,
    pub stream: bool,
}

impl Client {
    pub fn new(cfg: &AiConfig) -> Self {
        let timeout = Duration::from_secs(u64::from(cfg.timeout_secs.max(1)));
        let config = ureq::config::Config::builder()
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
            base: cfg.base_url.trim_end_matches('/').to_string(),
            token: cfg.token.0.clone(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    /// `GET /v1/models`: the readiness probe. Any 2xx is ready.
    pub fn probe(&self) -> Result<(), AiError> {
        let mut req = self.agent.get(format!("{}/models", self.base));
        if !self.token.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.token));
        }
        let resp = req.call().map_err(|e| self.map_err(&e))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(AiError::Gateway(format!("probe returned HTTP {}", resp.status().as_u16())))
        }
    }

    /// `POST /v1/chat/completions`. Streams through `on_chunk` when `call.stream`;
    /// returns the whole assistant text either way.
    pub fn chat(&self, call: &ChatCall<'_>, on_chunk: &mut dyn FnMut(&str)) -> Result<String, AiError> {
        let body = Self::body(call);
        let mut req = self
            .agent
            .post(format!("{}/chat/completions", self.base))
            .header(FEATURE_HEADER, call.feature);
        if !self.token.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.token));
        }
        let mut resp = req.send_json(&body).map_err(|e| self.map_err(&e))?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.body_mut().read_to_string().unwrap_or_default();
            let msg = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.get("message")).and_then(Value::as_str).map(str::to_string))
                .unwrap_or(text);
            return Err(AiError::Gateway(format!("HTTP {status}: {}", msg.trim())));
        }
        let text = if call.stream {
            Self::read_stream(BufReader::new(resp.body_mut().as_reader()), on_chunk).map_err(|e| self.map_err(&e))?
        } else {
            let v: Value = resp.body_mut().read_json().map_err(|e| self.map_err(&e))?;
            Self::message_content(&v).ok_or_else(|| AiError::Invalid("no choices[0].message.content in reply".into()))?
        };
        if text.trim().is_empty() {
            return Err(AiError::Invalid("empty reply".into()));
        }
        Ok(text)
    }

    fn body(call: &ChatCall<'_>) -> Value {
        let messages: Vec<Value> = call.messages.iter().map(|m| json!({ "role": m.role, "content": m.content })).collect();
        let mut body = json!({ "model": call.model, "messages": messages, "stream": call.stream });
        if !call.fallbacks.is_empty() {
            body["fallbacks"] = json!(call.fallbacks);
        }
        if let Some(schema) = call.schema.and_then(|s| serde_json::from_str::<Value>(s).ok()) {
            body["response_format"] = json!({
                "type": "json_schema",
                "json_schema": { "name": call.feature, "strict": true, "schema": schema }
            });
        }
        body
    }

    fn message_content(v: &Value) -> Option<String> {
        v.get("choices")?.get(0)?.get("message")?.get("content")?.as_str().map(str::to_string)
    }

    /// Server-sent events: `data: {json}` lines, `data: [DONE]` at the end.
    fn read_stream(reader: impl BufRead, on_chunk: &mut dyn FnMut(&str)) -> Result<String, ureq::Error> {
        let mut full = String::new();
        for line in reader.lines() {
            let line = line?;
            let Some(data) = line.strip_prefix("data:") else { continue };
            let data = data.trim();
            if data == "[DONE]" {
                break;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
            let piece = v
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("delta"))
                .and_then(|d| d.get("content"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if !piece.is_empty() {
                full.push_str(piece);
                on_chunk(piece);
            }
        }
        Ok(full)
    }

    fn map_err(&self, e: &ureq::Error) -> AiError {
        match e {
            ureq::Error::Timeout(_) => AiError::Timeout,
            ureq::Error::Io(io) if matches!(io.kind(), std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::NotFound) => {
                AiError::Unreachable(self.base.clone())
            }
            ureq::Error::ConnectProxyFailed(_) | ureq::Error::BadUri(_) => AiError::Unreachable(self.base.clone()),
            ureq::Error::Json(j) => AiError::Invalid(j.to_string()),
            other => {
                let text = other.to_string();
                if text.contains("connection refused") || text.contains("failed to lookup") || text.contains("dns") {
                    AiError::Unreachable(self.base.clone())
                } else {
                    AiError::Gateway(text)
                }
            }
        }
    }
}
