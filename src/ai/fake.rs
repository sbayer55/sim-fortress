//! Launcher for the fake gateway (ai-requirements R12).
//!
//! `scripts/fake-gateway.js` run by Node on an ephemeral port. Every test that
//! needs a gateway starts one of these; no test tier contacts a real gateway.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::sim::params::AiConfig;

#[derive(Debug)]
pub struct Fake {
    child: Child,
    port: u16,
}

impl Fake {
    /// Start the script with `--scenario <scenario>`, serving `tests/fixtures/ai`.
    pub fn start(scenario: &str) -> Result<Self, String> {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts").join("fake-gateway.js");
        let mut child = Command::new("node")
            .arg(&script)
            .args(["--port", "0", "--scenario", scenario])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("cannot start `node {}`: {e}. The AI tests need Node on PATH.", script.display()))?;
        let stdout = child.stdout.take().ok_or("no stdout from the fake gateway")?;
        let mut line = String::new();
        BufReader::new(stdout).read_line(&mut line).map_err(|e| e.to_string())?;
        let port = line
            .trim()
            .strip_prefix("PORT ")
            .and_then(|p| p.parse::<u16>().ok())
            .ok_or_else(|| format!("fake gateway did not print a port: {line:?}"))?;
        Ok(Self { child, port })
    }

    /// The base URL for `[ai] base_url`.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.port)
    }

    /// An enabled `[ai]` table pointing at this fake, with the given models.
    pub fn config(&self, chronicle: &str, designer: &str, timeout_secs: u32) -> AiConfig {
        let mut cfg = AiConfig { enabled: true, base_url: self.url(), timeout_secs, ..AiConfig::default() };
        cfg.features.chronicle = chronicle.to_string();
        cfg.features.designer = designer.to_string();
        cfg
    }

    /// Every chat request the fake has received, as `{feature, body}`.
    pub fn requests(&self) -> Vec<serde_json::Value> {
        let url = format!("http://127.0.0.1:{}/__requests", self.port);
        ureq::get(url)
            .call()
            .ok()
            .and_then(|mut r| r.body_mut().read_json::<Vec<serde_json::Value>>().ok())
            .unwrap_or_default()
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
