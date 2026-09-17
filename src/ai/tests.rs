//! Tests named in `docs/ai-requirements.md` R5, R7 and R8. The gateway ones need
//! Node on PATH and run only with `--features ai`.

use super::sanitize::to_cp437;

/// R8: anything outside CP437 becomes `?`, typographic punctuation is folded.
#[test]
fn non_cp437_output_is_sanitised() {
    let s = to_cp437("The voles \u{2014} \u{201c}quiet\u{201d} \u{2026} thrived \u{1f43e} in caf\u{e9}\n");
    assert_eq!(s, "The voles - \"quiet\" ... thrived ? in caf\u{e9}\n");
    assert!(s.chars().all(|c| c == '\n' || crate::glyphs::is_cp437(c)));
}

#[test]
fn off_handle_is_inert() {
    use super::{Ai, AiError, Feature, Message, Status};
    let ai = Ai::start(&crate::sim::params::AiConfig::default());
    assert!(!ai.is_on());
    assert_eq!(ai.status(), Status::Off);
    assert_eq!(ai.status_note(), "");
    assert!(!ai.feature_on(Feature::Chronicle));
    assert_eq!(ai.request(Feature::Chronicle, vec![Message::user("x")], None, false), Err(AiError::Off));
    assert!(ai.poll().is_empty());
}

/// R8: an overlay that fails the loader is rejected with the loader's message.
#[cfg(feature = "ai")]
#[test]
fn designer_rejects_invalid_overlay() {
    use super::prompt::designer::{overlay_from_reply, validated};
    use crate::sim::Params;
    let base = Params::default();
    let bad = overlay_from_reply(r#"{"name":"boar","plural":"Boars","glyph":"B","kind":"prey","base_genome":{"size":1.4}}"#).unwrap();
    assert_eq!(bad.name, "boar");
    assert!(bad.toml.starts_with("[[species]]"), "{}", bad.toml);
    let err = validated(&base, &bad.toml).unwrap_err();
    assert!(err.contains("glyph") || err.contains("outside"), "{err}");
    assert!(overlay_from_reply("Sure! Here is a boar.").unwrap_err().contains("not JSON"));
    assert!(overlay_from_reply(r#"{"plural":"Boars"}"#).unwrap_err().contains("name"));
    let unknown = overlay_from_reply(r#"{"name":"boar","plural":"Boars","glyph":"b","kind":"prey","teeth":3}"#).unwrap();
    assert!(validated(&base, &unknown.toml).unwrap_err().contains("teeth"));

    let good = overlay_from_reply("```json\n{\"name\":\"boar\",\"plural\":\"Boars\",\"glyph\":\"b\",\"kind\":\"prey\",\"initial_count\":30}\n```").unwrap();
    let p = validated(&base, &good.toml).unwrap();
    assert_eq!(p.species.len(), base.species.len() + 1, "a new name appends");
    assert_eq!(p.species.0.last().map(|s| s.name.as_str()), Some("boar"));
    let edit = overlay_from_reply(r#"{"name":"vole","plural":"Voles","glyph":"v","kind":"prey","initial_count":999}"#).unwrap();
    let p2 = validated(&base, &edit.toml).unwrap();
    assert_eq!(p2.species.len(), base.species.len(), "a known name edits in place");
    assert_eq!(p2.species.0[0].initial_count, 999);
}

#[cfg(feature = "ai")]
mod gateway {
    use std::time::{Duration, Instant};

    use crate::ai::fake::Fake;
    use crate::ai::{Ai, AiError, Feature, Message, Output, Reply, Status};
    use crate::sim::params::AiConfig;

    fn fake(scenario: &str) -> Fake {
        Fake::start(scenario).unwrap()
    }

    fn wait_all(ai: &Ai, id: u64, timeout: Duration) -> Vec<Reply> {
        let start = Instant::now();
        let mut out = Vec::new();
        while start.elapsed() < timeout {
            for r in ai.poll().into_iter().filter(|r| r.id == id) {
                let done = matches!(r.result, Ok(Output::Text(_) | Output::Done) | Err(_));
                out.push(r);
                if done {
                    return out;
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        out
    }

    fn wait_status(ai: &Ai, want: Status) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if ai.status() == want {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// A URL nothing listens on: bind, read the port, drop the listener.
    fn closed_port_url() -> String {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        format!("http://127.0.0.1:{port}/v1")
    }

    #[test]
    fn probe_marks_ready_then_offline() {
        let f = fake("happy");
        let ai = Ai::start(&f.config("fake/m", "", 5));
        assert!(ai.is_on());
        assert!(wait_status(&ai, Status::Ready), "probe should reach the fake");
        assert_eq!(ai.status_note(), "");
        assert!(ai.feature_on(Feature::Chronicle));
        assert!(!ai.feature_on(Feature::Designer));
        assert_eq!(ai.request(Feature::Designer, vec![], None, false), Err(AiError::NoModel("designer")));

        let mut cfg = AiConfig { enabled: true, base_url: closed_port_url(), timeout_secs: 2, ..AiConfig::default() };
        cfg.features.chronicle = "fake/m".into();
        let off = Ai::start(&cfg);
        assert!(wait_status(&off, Status::Offline));
        assert_eq!(off.status_note(), "AI: offline");
        let id = off.request(Feature::Chronicle, vec![Message::user("x")], None, false).unwrap();
        let replies = wait_all(&off, id, Duration::from_secs(5));
        assert!(matches!(replies.last().map(|r| &r.result), Some(Err(AiError::Unreachable(_)))), "{replies:?}");
    }

    /// R5: one HTTP request per action, whether it succeeds or fails.
    #[test]
    fn one_request_per_action() {
        let f = fake("happy");
        let ai = Ai::start(&f.config("fake/m", "fake/m", 5));
        let id = ai.request(Feature::Chronicle, vec![Message::system("s"), Message::user("u")], None, false).unwrap();
        let replies = wait_all(&ai, id, Duration::from_secs(5));
        assert!(matches!(replies.last().map(|r| &r.result), Some(Ok(Output::Text(t))) if t.contains("voles")), "{replies:?}");
        let reqs = f.requests();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0]["feature"], "chronicle");
        assert_eq!(reqs[0]["body"]["model"], "fake/m");
        assert_eq!(reqs[0]["body"]["messages"][1]["content"], "u");

        let err = fake("error");
        let ai = Ai::start(&err.config("fake/m", "", 5));
        let id = ai.request(Feature::Chronicle, vec![Message::user("u")], None, false).unwrap();
        let replies = wait_all(&ai, id, Duration::from_secs(5));
        assert!(matches!(replies.last().map(|r| &r.result), Some(Err(AiError::Gateway(m))) if m.contains("not served")), "{replies:?}");
        assert_eq!(err.requests().len(), 1, "a failed call is never retried");
    }

    #[test]
    fn streaming_delivers_chunks_then_done() {
        let f = fake("happy");
        let ai = Ai::start(&f.config("fake/m", "", 5));
        let id = ai.request(Feature::Chronicle, vec![Message::user("u")], None, true).unwrap();
        let replies = wait_all(&ai, id, Duration::from_secs(5));
        let chunks: String = replies.iter().filter_map(|r| match &r.result { Ok(Output::Chunk(c)) => Some(c.as_str()), _ => None }).collect();
        assert!(replies.len() > 2, "expected several chunks: {replies:?}");
        assert!(chunks.contains("quiet valley"));
        assert_eq!(replies.last().unwrap().result, Ok(Output::Done));
        assert_eq!(f.requests()[0]["body"]["stream"], true);
    }

    /// R7: a reply for an id the caller no longer waits on is dropped.
    #[test]
    fn stale_reply_is_dropped() {
        let f = fake("happy");
        let ai = Ai::start(&f.config("fake/m", "", 5));
        let id = ai.request(Feature::Chronicle, vec![Message::user("u")], None, false).unwrap();
        ai.cancel(id);
        let start = Instant::now();
        while f.requests().is_empty() && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(200));
        assert!(ai.poll().is_empty(), "cancelled reply must not surface");
        let id2 = ai.request(Feature::Chronicle, vec![Message::user("u")], None, false).unwrap();
        assert!(!wait_all(&ai, id2, Duration::from_secs(5)).is_empty(), "later requests still flow");
    }

    /// R7: a timeout is an error reply, never a retry.
    #[test]
    fn timeout_yields_error_reply() {
        let f = fake("timeout");
        let ai = Ai::start(&f.config("fake/m", "", 1));
        let id = ai.request(Feature::Chronicle, vec![Message::user("u")], None, false).unwrap();
        let replies = wait_all(&ai, id, Duration::from_secs(10));
        assert_eq!(replies.last().map(|r| r.result.clone()), Some(Err(AiError::Timeout)), "{replies:?}");
        assert_eq!(f.requests().len(), 1);
    }

    #[test]
    fn malformed_reply_is_invalid() {
        let f = fake("malformed");
        let ai = Ai::start(&f.config("fake/m", "", 5));
        let id = ai.request(Feature::Chronicle, vec![Message::user("u")], None, false).unwrap();
        let replies = wait_all(&ai, id, Duration::from_secs(5));
        assert!(matches!(replies.last().map(|r| &r.result), Some(Err(AiError::Invalid(_)))), "{replies:?}");
    }
}
