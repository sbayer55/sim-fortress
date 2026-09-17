//! The one background thread that owns the HTTP client (ai-requirements R7).
//!
//! Requests go in on a channel, replies come out on another; the thread
//! probes `GET /v1/models` on start and every 60 s while offline, and exits when
//! the handle is dropped (the channel closes; an in-flight call finishes and its
//! reply is discarded with the receiver).

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;

use super::client::{ChatCall, Client};
use super::{AiError, Feature, Message, Output, Reply, Request, RequestId, Status};
use crate::sim::params::AiConfig;

const PROBE_EVERY: Duration = Duration::from_secs(60);

const S_OFFLINE: u8 = 0;
const S_READY: u8 = 1;

/// The live side of `Ai::On`.
#[derive(Debug)]
pub struct Handle {
    config: AiConfig,
    tx: Sender<Request>,
    rx: Receiver<Reply>,
    status: Arc<AtomicU8>,
    next_id: Cell<RequestId>,
    cancelled: RefCell<BTreeSet<RequestId>>,
}

impl Handle {
    pub fn start(config: AiConfig) -> Self {
        let (tx, worker_rx) = mpsc::channel::<Request>();
        let (worker_tx, rx) = mpsc::channel::<Reply>();
        let status = Arc::new(AtomicU8::new(S_OFFLINE));
        let thread_status = Arc::clone(&status);
        let thread_config = config.clone();
        let spawned = std::thread::Builder::new()
            .name("ai-worker".into())
            .spawn(move || run(&thread_config, &worker_rx, &worker_tx, &thread_status));
        if spawned.is_err() {
            status.store(S_OFFLINE, Ordering::Relaxed);
        }
        Self { config, tx, rx, status, next_id: Cell::new(1), cancelled: RefCell::new(BTreeSet::new()) }
    }

    pub const fn config(&self) -> &AiConfig {
        &self.config
    }

    pub fn status(&self) -> Status {
        if self.status.load(Ordering::Relaxed) == S_READY {
            Status::Ready
        } else {
            Status::Offline
        }
    }

    pub fn request(&self, feature: Feature, messages: Vec<Message>, schema: Option<String>, stream: bool) -> Result<RequestId, AiError> {
        if self.config.features.model(feature.key()).is_none() {
            return Err(AiError::NoModel(feature.key()));
        }
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        self.tx
            .send(Request { id, feature, messages, schema, stream })
            .map_err(|_| AiError::Unreachable(self.config.base_url.clone()))?;
        Ok(id)
    }

    pub fn cancel(&self, id: RequestId) {
        self.cancelled.borrow_mut().insert(id);
    }

    pub fn poll(&self) -> Vec<Reply> {
        let cancelled = self.cancelled.borrow();
        self.rx.try_iter().filter(|r| !cancelled.contains(&r.id)).collect()
    }

    /// Block until `id` finishes. Chunks are concatenated; other ids are dropped.
    pub fn wait(&self, id: RequestId) -> Result<String, AiError> {
        let mut text = String::new();
        loop {
            let reply = self.rx.recv().map_err(|_| AiError::Unreachable(self.config.base_url.clone()))?;
            if reply.id != id {
                continue;
            }
            match reply.result? {
                Output::Text(t) => return Ok(t),
                Output::Chunk(c) => text.push_str(&c),
                Output::Done => return Ok(text),
            }
        }
    }
}

fn run(config: &AiConfig, rx: &Receiver<Request>, tx: &Sender<Reply>, status: &Arc<AtomicU8>) {
    let client = Client::new(config);
    probe(&client, status);
    loop {
        match rx.recv_timeout(PROBE_EVERY) {
            Ok(req) => {
                let result = perform(&client, config, &req, tx);
                match &result {
                    Err(AiError::Unreachable(_) | AiError::Timeout) => status.store(S_OFFLINE, Ordering::Relaxed),
                    _ => status.store(S_READY, Ordering::Relaxed),
                }
                let output = match result {
                    Ok(_) if req.stream => Ok(Output::Done),
                    Ok(text) => Ok(Output::Text(text)),
                    Err(e) => Err(e),
                };
                if tx.send(Reply { id: req.id, feature: req.feature, result: output }).is_err() {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if status.load(Ordering::Relaxed) == S_OFFLINE {
                    probe(&client, status);
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn probe(client: &Client, status: &Arc<AtomicU8>) {
    let s = if client.probe().is_ok() { S_READY } else { S_OFFLINE };
    status.store(s, Ordering::Relaxed);
}

fn perform(client: &Client, config: &AiConfig, req: &Request, tx: &Sender<Reply>) -> Result<String, AiError> {
    let key = req.feature.key();
    let model = config.features.model(key).ok_or(AiError::NoModel(key))?;
    let call = ChatCall {
        model,
        fallbacks: config.features.fallbacks(key),
        feature: key,
        messages: &req.messages,
        schema: req.schema.as_deref(),
        stream: req.stream,
    };
    let mut on_chunk = |piece: &str| {
        let _ = tx.send(Reply { id: req.id, feature: req.feature, result: Ok(Output::Chunk(piece.to_string())) });
    };
    client.chat(&call, &mut on_chunk)
}
