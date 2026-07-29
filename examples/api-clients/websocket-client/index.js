#!/usr/bin/env node

const WebSocket = require('ws');

const TX_HASH = process.argv[2];
const WS_URL = process.env.WS_URL || 'ws://localhost:8080';

if (!TX_HASH) {
  console.error('Usage: node websocket-client.js <tx-hash>');
  process.exit(1);
}

console.log(`Connecting to ${WS_URL}...`);

// Persistent synchronization cursor, declared OUTSIDE the connection scope so it
// survives across reconnects. It tracks the index of the most recent trace node
// the server has delivered to us. When the socket drops and we reconnect, we hand
// this value back to the server via `resume_from` so the trace is fast-forwarded
// to where it left off instead of replayed from the very beginning.
let lastSeenNodeId = null;

let ws = null;
let nodeCount = 0;
let startTime = null;
let stopped = false; // set once the trace completes/errors so we stop reconnecting
let reconnectAttempts = 0;
let isReconnecting = false;

const MAX_RECONNECT_DELAY = 5000; // cap exponential backoff at 5s
const MAX_RECONNECT_ATTEMPTS = 10;
const INITIAL_RECONNECT_DELAY = 1000;
const HANDSHAKE_TIMEOUT = 5000;

// Build the request payload, dynamically augmenting it with the resume cursor
// once we have received at least one trace node.
function buildRequest() {
  const request = { tx_hash: TX_HASH };
  if (lastSeenNodeId !== null) {
    request.resume_from = lastSeenNodeId;
  }
  return request;
}

function connectToTraceStream() {
  const socket = new WebSocket(WS_URL, { handshakeTimeout: HANDSHAKE_TIMEOUT });
  ws = socket;

  socket.on('open', () => {
    if (ws !== socket) return;
    reconnectAttempts = 0;
    isReconnecting = false;
    if (startTime === null) startTime = Date.now();

    if (lastSeenNodeId === null) {
      console.log('✓ Connected to Grat WebSocket server');
    } else {
      console.log(`✓ Reconnected to Grat WebSocket server (resuming from node ${lastSeenNodeId})`);
    }
    console.log(`Requesting trace for: ${TX_HASH}\n`);

    socket.send(JSON.stringify(buildRequest()));
  });

  socket.on('message', (data) => {
    if (ws !== socket) return;
    try {
      const message = JSON.parse(data.toString());

      switch (message.type) {
        case 'trace_started':
          console.log('🚀 Trace started');
          console.log(`   Transaction: ${message.tx_hash}`);
          console.log(`   Ledger: ${message.ledger_sequence}\n`);
          break;

        case 'trace_node': {
          // Defensive deduplication: if the server hasn't (yet) honored
          // `resume_from`, or we somehow receive a node we already processed
          // before the disconnect, skip it rather than re-processing it.
          const nodeId =
            Array.isArray(message.path) && message.path.length > 0
              ? message.path[0]
              : null;

          if (
            nodeId !== null &&
            lastSeenNodeId !== null &&
            nodeId <= lastSeenNodeId
          ) {
            break;
          }

          nodeCount++;
          if (nodeId !== null) lastSeenNodeId = nodeId;
          process.stdout.write(`\r📦 Received ${nodeCount} trace nodes...`);
          break;
        }

        case 'resource_update':
          const cpuPercent = (message.cpu_used / message.cpu_limit * 100).toFixed(1);
          const memPercent = (message.memory_used / message.memory_limit * 100).toFixed(1);
          console.log(`\n📊 Resources: CPU ${cpuPercent}%, Memory ${memPercent}%`);
          break;

        case 'state_diff_entry':
          console.log(`\n📝 State change: ${message.key} (${message.change_type})`);
          break;

        case 'trace_completed':
          const duration = Date.now() - startTime;
          console.log('\n\n✅ Trace completed!');
          console.log(`   Total nodes: ${message.total_nodes}`);
          console.log(`   Server duration: ${message.duration_ms}ms`);
          console.log(`   Client duration: ${duration}ms`);
          stopped = true;
          socket.close();
          break;

        case 'trace_error':
          console.error('\n\n❌ Trace error:', message.error);
          stopped = true;
          socket.close();
          process.exit(1);
          break;

        default:
          console.log('\n⚠ Unknown message type:', message.type);
      }
    } catch (err) {
      console.error('Failed to parse message:', err);
    }
  });

  socket.on('error', (err) => {
    if (ws !== socket) return;
    console.error(`WebSocket error: ${err.message || err}`);
    reconnect();
  });

  socket.on('close', () => {
    if (ws !== socket) return;
    if (stopped) {
      process.exit(0);
    }
    reconnect();
  });
}

function reconnect() {
  if (stopped || isReconnecting) return;
  isReconnecting = true;

  reconnectAttempts++;
  if (reconnectAttempts > MAX_RECONNECT_ATTEMPTS) {
    console.error(`\n❌ Maximum reconnect attempts (${MAX_RECONNECT_ATTEMPTS}) reached. Exiting.`);
    process.exit(1);
    return;
  }

  const delay = Math.min(
    MAX_RECONNECT_DELAY,
    INITIAL_RECONNECT_DELAY * Math.pow(2, reconnectAttempts - 1)
  );

  console.log(`Connection lost. Reconnecting in ${delay / 1000}s...`);

  setTimeout(() => {
    connectToTraceStream();
    isReconnecting = false;
  }, delay);
}

process.on('SIGINT', () => {
  console.log('\n\nClosing connection...');
  stopped = true;
  if (ws) ws.close();
  process.exit(0);
});

connectToTraceStream();
