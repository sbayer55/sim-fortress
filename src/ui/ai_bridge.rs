//! Where the live app meets the `Ai` handle (C9): the season trigger for the
//! chronicle and the per-frame reply pump. Screens never call the gateway.

use std::cell::RefCell;

use crate::ai::prompt;
use crate::ai::sanitize::to_cp437;
use crate::ai::{AiError, Feature, Output, Reply, RequestId};
use crate::sim::chronicle::{self, Source};

use super::app::AppState;

/// A chronicle request in flight: which entry it fills and the text so far.
#[derive(Debug)]
pub struct ChroniclePending {
    pub id: RequestId,
    pub entry: usize,
    pub template: String,
    pub text: String,
}

/// Replies routed to the species designer modal, which drains them on render.
pub type DesignerInbox = RefCell<Vec<Reply>>;

impl AppState {
    /// A season ended in the last batch: push its template entry and, if the
    /// chronicle feature is on, ask the model to rewrite it.
    pub fn enqueue_chronicle(&mut self) {
        let Some((year, season)) = self.season_ended.take() else { return };
        let Some(sim) = self.sim.as_mut() else { return };
        let entry = chronicle::template_entry(sim, year, season);
        let template = entry.text.clone();
        sim.chronicle.push(entry);
        let idx = sim.chronicle.len().saturating_sub(1);
        if !self.ai.feature_on(Feature::Chronicle) {
            return;
        }
        if let Some(old) = self.chronicle_pending.take() {
            self.ai.cancel(old.id);
        }
        let previous = idx.checked_sub(1).and_then(|i| sim.chronicle.get(i)).map(|e| e.text.as_str());
        let input = prompt::chronicle::from_sim(sim, year, season, previous);
        match self.ai.request(Feature::Chronicle, prompt::chronicle::messages(&input), None, true) {
            Ok(id) => self.chronicle_pending = Some(ChroniclePending { id, entry: idx, template, text: String::new() }),
            Err(e) => self.ai_notice = Some(e.to_string()),
        }
    }

    /// Drain replies; returns true when something changed and a redraw is due.
    pub fn pump_ai(&mut self) -> bool {
        let replies = self.ai.poll();
        if replies.is_empty() {
            return false;
        }
        for r in replies {
            match r.feature {
                Feature::Chronicle => self.on_chronicle_reply(r),
                Feature::Designer => self.designer_inbox.borrow_mut().push(r),
            }
        }
        true
    }

    fn on_chronicle_reply(&mut self, r: Reply) {
        let Some(pending) = self.chronicle_pending.as_mut() else { return };
        if pending.id != r.id {
            return;
        }
        let Some(sim) = self.sim.as_mut() else { return };
        let Some(entry) = sim.chronicle.get_mut(pending.entry) else {
            self.chronicle_pending = None;
            return;
        };
        match r.result {
            Ok(Output::Chunk(c)) => {
                pending.text.push_str(&c);
                entry.text = to_cp437(&pending.text);
            }
            Ok(Output::Text(t)) => {
                pending.text = t;
                Self::finish_chronicle(entry, &pending.text);
                self.chronicle_pending = None;
                self.ai_notice = None;
            }
            Ok(Output::Done) => {
                Self::finish_chronicle(entry, &pending.text);
                self.chronicle_pending = None;
                self.ai_notice = None;
            }
            Err(e) => {
                entry.text.clone_from(&pending.template);
                entry.source = Source::Template;
                self.ai_notice = Some(match e {
                    AiError::Timeout | AiError::Unreachable(_) => "chronicle: gateway did not answer; template kept".to_string(),
                    other => format!("chronicle: {other}; template kept"),
                });
                self.chronicle_pending = None;
            }
        }
    }

    fn finish_chronicle(entry: &mut crate::sim::ChronicleEntry, text: &str) {
        let clean = to_cp437(text.trim());
        if !clean.is_empty() {
            entry.text = clean;
            entry.source = Source::Model;
        }
    }
}
