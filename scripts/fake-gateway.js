#!/usr/bin/env node
// Fake Bifrost gateway for tests and offline development (C9, ai-requirements R12).
//
//   node scripts/fake-gateway.js [--port 0] [--fixtures tests/fixtures/ai] [--scenario happy]
//
// Prints one line `PORT <n>` on stdout once listening (port 0 = ephemeral).
// Routes, all on 127.0.0.1:
//   GET  /v1/models            the readiness probe
//   POST /v1/chat/completions  answered from <fixtures>/<feature>/<scenario>.json, where
//                              <feature> is the request's `x-simf-feature` header
//   GET  /__requests           every chat request received, as [{feature, body}]
//   POST /__reset              forget recorded requests and per-feature call counts
// A fixture is one of
//   { "content": "text" }                 a completion; streamed as SSE when the request
//                                         says "stream": true, else one JSON body
//   { "status": 500, "error": "message" } an error body in the OpenAI shape
//   { "raw": { ... } }                    a verbatim JSON body (malformed-reply tests)
//   { "timeout": true }                   accept the request and never answer
//   { "sequence": [fixture, fixture] }    the n-th call to that feature gets the n-th
//                                         entry; the last one repeats
// Plain node:http, no packages, no package.json.
'use strict';
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');

const args = process.argv.slice(2);
function flag(name, fallback) {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] !== undefined ? args[i + 1] : fallback;
}
const port = Number(flag('--port', '0'));
const fixtures = flag('--fixtures', path.join(__dirname, '..', 'tests', 'fixtures', 'ai'));
const scenario = flag('--scenario', 'happy');

const requests = [];
const calls = {};

function fixtureFor(feature) {
  const file = path.join(fixtures, feature, `${scenario}.json`);
  let fx;
  try {
    fx = JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch (e) {
    return { status: 404, error: `no fixture ${file}` };
  }
  if (Array.isArray(fx.sequence)) {
    const n = calls[feature] || 0;
    fx = fx.sequence[Math.min(n, fx.sequence.length - 1)];
  }
  return fx;
}

function sendJson(res, status, body) {
  const s = JSON.stringify(body);
  res.writeHead(status, { 'Content-Type': 'application/json', 'Content-Length': Buffer.byteLength(s) });
  res.end(s);
}

function completion(content) {
  return {
    id: 'chatcmpl-fake',
    object: 'chat.completion',
    model: 'fake/model',
    choices: [{ index: 0, message: { role: 'assistant', content }, finish_reason: 'stop' }],
  };
}

function streamContent(res, content) {
  res.writeHead(200, { 'Content-Type': 'text/event-stream', 'Cache-Control': 'no-cache' });
  const pieces = content.match(/[\s\S]{1,24}/g) || [];
  let i = 0;
  const tick = () => {
    if (i < pieces.length) {
      res.write(`data: ${JSON.stringify({ choices: [{ index: 0, delta: { content: pieces[i] } }] })}\n\n`);
      i += 1;
      setTimeout(tick, 5);
    } else {
      res.write('data: [DONE]\n\n');
      res.end();
    }
  };
  tick();
}

function chat(req, res, raw) {
  let body;
  try {
    body = JSON.parse(raw);
  } catch (e) {
    return sendJson(res, 400, { error: { message: 'request body is not JSON' } });
  }
  const feature = req.headers['x-simf-feature'] || 'chronicle';
  const fx = fixtureFor(feature);
  calls[feature] = (calls[feature] || 0) + 1;
  requests.push({ feature, body });
  if (fx.timeout) return undefined; // never answer
  if (fx.status && fx.status >= 400) return sendJson(res, fx.status, { error: { message: fx.error || 'error' } });
  if (fx.raw !== undefined) return sendJson(res, fx.status || 200, fx.raw);
  const content = typeof fx.content === 'string' ? fx.content : '';
  if (body.stream) return streamContent(res, content);
  return sendJson(res, 200, completion(content));
}

const server = http.createServer((req, res) => {
  let raw = '';
  req.on('data', (c) => { raw += c; });
  req.on('end', () => {
    const url = req.url.split('?')[0];
    if (req.method === 'GET' && url === '/v1/models') {
      return sendJson(res, 200, { object: 'list', data: [{ id: 'fake/model', object: 'model' }] });
    }
    if (req.method === 'GET' && url === '/__requests') return sendJson(res, 200, requests);
    if (req.method === 'POST' && url === '/__reset') {
      requests.length = 0;
      for (const k of Object.keys(calls)) delete calls[k];
      return sendJson(res, 200, {});
    }
    if (req.method === 'POST' && url === '/v1/chat/completions') return chat(req, res, raw);
    return sendJson(res, 404, { error: { message: `no route ${req.method} ${url}` } });
  });
});

server.listen(port, '127.0.0.1', () => {
  process.stdout.write(`PORT ${server.address().port}\n`);
});
