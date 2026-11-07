import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import { randomUUID, createHash } from 'node:crypto';

export const VERSION = '1.0.5';
const ACTIONS = new Set(['ports', 'connect', 'scan', 'status', 'move', 'release', 'stop', 'disconnect', 'heartbeat']);
const BAUDS = [57600, 115200, 1000000, 2000000, 3000000, 4000000];
const EMPTY = { connected: false, port: null, baud: 1000000, updatedAt: 0, servos: [], error: null, commands: [], controlledIds: [], scanning: false, version: VERSION };

export function validateRequest(input, actor) {
  if (!input || typeof input !== 'object' || Array.isArray(input) || !ACTIONS.has(input.action)) throw new Error('Unknown servo action');
  const { action } = input;
  const req = { action };
  if (['move', 'release', 'stop', 'heartbeat'].includes(action)) req.actor = actor;
  if (action === 'connect') {
    if (typeof input.port !== 'string' || !/^\/dev\/cu\.[A-Za-z0-9_.-]+$/.test(input.port)) throw new Error('Select an enumerated macOS callout serial port');
    if (!BAUDS.includes(input.baud)) throw new Error('Unsupported baud rate');
    req.port = input.port; req.baud = input.baud;
  }
  if (action === 'scan') {
    if (input.maxId !== undefined && ![20, 252].includes(input.maxId)) throw new Error('Scan maxId must be 20 or 252');
    req.maxId = input.maxId ?? 20;
  }
  if (action === 'move' || action === 'release') {
    if (!Number.isInteger(input.id) || input.id < 0 || input.id > 252) throw new Error('Servo ID must be 0..252');
    if (actor === 'agent' && ![1, 2, 3].includes(input.id)) throw new Error('Agent motion is restricted to IDs 1, 2, 3');
    req.id = input.id;
  }
  if (action === 'move') {
    if (typeof input.positionDeg !== 'number' || !Number.isFinite(input.positionDeg) || input.positionDeg < 0 || input.positionDeg > 4095 * 360 / 4096) throw new Error('Position must be finite and inside 0..359.912 degrees');
    if (actor === 'agent' && (input.positionDeg < 170 || input.positionDeg > 190)) throw new Error('Agent test position must remain inside 170..190 degrees');
    req.positionDeg = input.positionDeg;
  }
  return req;
}

/** One native worker owns the serial port; all callers use this controller. */
export class ServoController {
  constructor({ spawnWorker = () => spawn(fileURLToPath(new URL('./bin/servo-worker', import.meta.url)), [], { stdio: ['pipe', 'pipe', 'pipe'] }), clock = Date.now } = {}) {
    this.spawnWorker = spawnWorker; this.clock = clock;
    this.state = structuredClone(EMPTY); this.pending = new Map(); this.idempotency = new Map();
    this.child = null; this.disposed = false; this.busy = null; this.owner = null; this.stderr = ''; this.sequence = 0;
    this.audit = []; this.operations = [];
    this.uiTransport = { successfulPosts: 0, lastAction: null, lastSuccessAt: null, browserOrigin: null, carrierOrigin: null };
  }
  snapshot() {
    const now = this.clock();
    return { ...this.state, version: VERSION, hostRevision: 'r4-commissioned-controls', workerVersion: this.state.updatedAt ? this.state.version : null, serverTime: now, busy: this.busy, owner: this.owner?.actor ?? null,
      transportAlive: !!this.child, sequence: this.sequence, audit: this.audit.slice(-30), uiTransport: { ...this.uiTransport },
      servos: (this.state.servos ?? []).map(row => ({ ...row, stale: !row.online || now - row.lastSeen > 2500 })) };
  }
  note(action, actor, outcome, detail) {
    this.audit.push({ at: this.clock(), action, actor, outcome, detail });
    if (this.audit.length > 100) this.audit.shift();
  }
  acceptState(state) {
    if (!state || typeof state.connected !== 'boolean' || !Array.isArray(state.servos)) return;
    this.state = state; this.sequence++;
    if (!(state.controlledIds?.length) && !this.operations.some(op => ['move', 'release'].includes(op.action))) this.owner = null;
  }
  ensureWorker() {
    if (this.disposed) throw new Error('Servo plugin is unloading');
    if (this.child) return;
    if (this.state.controlledIds?.length) throw new Error('Previous worker lost control; physical state UNKNOWN. Check power and hardware before reconnecting.');
    const child = this.spawnWorker(); this.child = child;
    const lines = createInterface({ input: child.stdout });
    lines.on('line', line => {
      let msg;
      try { msg = JSON.parse(line); } catch { this.state.error = 'Invalid worker output'; return; }
      if (msg.event === 'state') { this.acceptState(msg.data); return; }
      const waiter = this.pending.get(msg.requestId);
      if (!waiter) return;
      this.pending.delete(msg.requestId); clearTimeout(waiter.timer);
      if (msg.data?.state) this.acceptState(msg.data.state);
      else if (typeof msg.data?.connected === 'boolean') this.acceptState(msg.data);
      if (msg.ok) waiter.resolve(msg.data);
      else waiter.reject(new Error(msg.error || 'Serial worker rejected the request'));
    });
    child.stderr.on('data', chunk => { this.stderr = (this.stderr + chunk).slice(-6000); });
    const failed = reason => {
      if (this.child !== child) return;
      this.child = null; lines.close();
      this.state = { ...this.state, connected: false, error: reason, servos: this.state.servos.map(s => ({ ...s, online: false, error: reason })) };
      for (const waiter of this.pending.values()) { clearTimeout(waiter.timer); waiter.reject(new Error(reason)); }
      this.pending.clear(); this.sequence++;
    };
    child.on('error', error => failed(`Worker error: ${error.message}`));
    child.on('exit', (code, signal) => failed(`Serial worker exited (${signal || code}); motor state may be UNKNOWN. ${this.stderr.slice(-600)}`));
    child.stdin.on('error', error => failed(`Serial worker input failed: ${error.message}`));
  }
  request(req, timeoutMs = 45000, requestId = randomUUID()) {
    this.ensureWorker();
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(requestId);
        this.state.error = `${req.action} timeout; outcome UNKNOWN. Do not automatically resend.`;
        reject(new Error(this.state.error));
      }, timeoutMs);
      this.pending.set(requestId, { resolve, reject, timer });
      this.child.stdin.write(JSON.stringify({ ...req, requestId }) + '\n', error => {
        if (!error) return;
        const waiter = this.pending.get(requestId);
        if (waiter) { clearTimeout(timer); this.pending.delete(requestId); reject(error); }
      });
    });
  }
  async execute(input, { actor = 'agent', ownerId = 'agent' } = {}) {
    if (!['human', 'agent'].includes(actor)) throw new Error('Invalid caller');
    const req = validateRequest(input, actor);
    if (req.action === 'status') return this.snapshot();
    if (req.action === 'heartbeat') {
      if (!this.owner || this.owner.actor !== actor || this.owner.id !== ownerId) return { renewed: false };
      return this.request(req, 12000);
    }
    if (req.action === 'ports') return this.request(req);
    if (this.owner && (this.owner.actor !== actor || this.owner.id !== ownerId)) {
      // A human may explicitly stop plugin-controlled motors, but cannot silently take over.
      if (!(actor === 'human' && req.action === 'stop')) throw new Error('Another caller owns motion control. Release it before continuing.');
    }
    if (req.action === 'scan' && this.state.controlledIds?.length) throw new Error('Release plugin-controlled motors before scanning');
    if (req.action === 'connect' && this.state.connected) throw new Error('Already connected; disconnect before changing ports or baud');
    if (this.busy && !['stop', 'release'].includes(req.action)) throw new Error(`Serial operation in progress: ${this.busy}`);
    const nonce = input.commandId;
    if (req.action === 'move') {
      if (typeof nonce !== 'string' || !/^[a-zA-Z0-9_-]{8,100}$/.test(nonce)) throw new Error('move requires a unique commandId (8..100 characters)');
      const key = `${actor}:${ownerId}:${nonce}`;
      const previous = this.idempotency.get(key);
      if (previous) {
        if (previous.id !== req.id || previous.positionDeg !== req.positionDeg) throw new Error('commandId was already used for a different target');
        return previous.promise;
      }
      if (this.idempotency.size >= 2048) throw new Error('Command history is full; release motors and restart the plugin before starting a new command series.');
      const requestId = createHash('sha256').update(`${actor}\0${ownerId}\0${nonce}`).digest('hex');
      const promise = this.perform(req, { actor, ownerId, requestId });
      this.idempotency.set(key, { id: req.id, positionDeg: req.positionDeg, promise });
      // Retain rejected attempts too: an uncertain write is never automatically retried.
      promise.catch(() => {});
      return promise;
    }
    return this.perform(req, { actor, ownerId });
  }
  async perform(req, { actor, ownerId, requestId }) {
    const operation = { action: req.action };
    this.operations.push(operation);
    this.busy = this.operations[0].action;
    if (req.action === 'move' || (req.action === 'release' && !this.owner)) this.owner = { actor, id: ownerId };
    try {
      if (req.action === 'connect') {
        const ports = await this.request({ action: 'ports' });
        if (!ports.ports?.some(p => p.path === req.port)) throw new Error('Serial port is no longer available');
      }
      const data = await this.request(req, 45000, requestId);
      this.note(req.action, actor, 'completed', req.action === 'move' ? { id: req.id, positionDeg: req.positionDeg, result: data.command } : null);
      return typeof data?.connected === 'boolean' ? { state: this.snapshot() } : { ...data, state: this.snapshot() };
    } catch (error) {
      this.note(req.action, actor, 'failed', { id: req.id, message: error.message });
      throw error;
    } finally {
      this.operations = this.operations.filter(active => active !== operation);
      this.busy = this.operations[0]?.action ?? null;
      if (!this.state.controlledIds?.length) this.owner = null;
    }
  }
  async dispose() {
    this.disposed = true;
    const child = this.child;
    if (!child) return;
    // EOF is a native cleanup instruction, not a blanket torque-off on the bus.
    child.stdin.end();
    await new Promise(resolve => {
      const timer = setTimeout(() => { child.kill('SIGTERM'); resolve(); }, 5000);
      child.once('exit', () => { clearTimeout(timer); resolve(); });
    });
  }
}
