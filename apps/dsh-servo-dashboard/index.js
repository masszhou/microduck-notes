import { ServoController, VERSION } from './controller.js';

export const inject = ['connection', 'tools'];
export const name = 'servo-dashboard';

/** Keep motion replies small enough to inspect and renew a five-second lease. */
export function summarizeToolResult(args, value) {
  if (!['move', 'release', 'stop', 'heartbeat', 'connect', 'scan', 'disconnect'].includes(args?.action)) return value;
  const state = value.state || value;
  if (!Array.isArray(state.servos)) return value;
  const selected = state.servos.filter(s => s.id === args.id || state.controlledIds?.includes(s.id));
  const summary = {
    connected: state.connected, port: state.port, baud: state.baud, version: state.version,
    workerVersion: state.workerVersion, fullScanValid: state.fullScanValid,
    error: state.error, controlledIds: state.controlledIds, serverTime: state.serverTime || state.updatedAt,
    onlineIds: state.servos.filter(s => s.online).map(s => s.id),
    servos: ['connect', 'scan'].includes(args.action) ? state.servos.map(s => ({ id:s.id, model:s.model, positionDeg:s.positionDeg, goalDeg:s.goalDeg, torque:s.torque, stale:s.stale, lastSeen:s.lastSeen })) : selected,
  };
  return value.command ? { command: value.command, state: summary } : { state: summary };
}

export function apply(ctx) {
  const controller = new ServoController();
  ctx.provide('servoDashboard', controller);
  ctx.effect(() => () => controller.dispose());
  const headers = { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store' };
  const reply = (body, status = 200) => new Response(JSON.stringify(body), { status, headers });
  ctx.connection.fetch.register({
    path: '/api/servo-dashboard', methods: ['GET', 'POST'], requestBody: 'buffered',
    async fetch(request) {
      if (request.method === 'GET') return reply({ ok: true, data: controller.snapshot(), version: VERSION });
      // Connection admits Host/Origin and authenticated Peer before this route.
      // Its bridge rewrites Request.url to http://dsh.internal; it is NOT the
      // browser origin. Keep the framework trust fence and JSON/CSRF barriers.
      if (!request.headers.get('content-type')?.startsWith('application/json') || request.headers.get('x-servo-dashboard') !== '1' || request.headers.get('sec-fetch-site') === 'cross-site') return reply({ ok: false, error: 'Expected dashboard JSON request' }, 403);
      try {
        const text = await request.text();
        if (text.length > 4096) return reply({ ok: false, error: 'Request too large' }, 413);
        const input = JSON.parse(text);
        if (typeof input.clientId !== 'string' || !/^[a-zA-Z0-9_-]{8,100}$/.test(input.clientId)) throw new Error('Missing dashboard clientId');
        const data = await controller.execute(input, { actor: 'human', ownerId: input.clientId });
        controller.uiTransport = { successfulPosts: controller.uiTransport.successfulPosts + 1, lastAction: input.action, lastSuccessAt: Date.now(), browserOrigin: request.headers.get('origin'), carrierOrigin: new URL(request.url).origin };
        return reply({ ok: true, data });
      } catch (error) {
        return reply({ ok: false, error: error.message, state: controller.snapshot() }, 400);
      }
    },
  });
  ctx.tools.register({
    name: 'servo_dashboard',
    description: 'Discover and monitor DYNAMIXEL servos through OpenRB-150, or perform explicitly authorized bounded position tests. Movement is limited to IDs 1–3 with present and target inside 170–190°. Never changes EEPROM. move retains torque until release/stop or the 5-second heartbeat lease expires. Scan all IDs before motion. status returns live cached telemetry with freshness; a successful send is not arrival. An UNKNOWN outcome must not be retried automatically.',
    parameters: {
      type: 'object', properties: {
        action: { type: 'string', enum: ['ports', 'connect', 'scan', 'status', 'move', 'release', 'stop', 'disconnect', 'heartbeat'] },
        port: { type: 'string', description: 'connect: exact path returned by ports.' },
        baud: { type: 'integer', description: 'connect: bus baud. Current hardware uses 1000000. Does not change servo EEPROM.' },
        maxId: { type: 'integer', enum: [20, 252], description: 'scan: last ID, default 20. Use 252 before motion to check alias safety.' },
        id: { type: 'integer', description: 'move/release: ID 1, 2, or 3.' },
        positionDeg: { type: 'number', description: 'move: absolute encoder degrees, 170..190; maximum 10° from fresh measured position.' },
        commandId: { type: 'string', description: 'move: unique 8..100-character alphanumeric/hyphen/underscore ID. Reusing it within this plugin instance never resends. Never retry UNKNOWN after a restart.' },
      }, required: ['action'], additionalProperties: false,
    },
    output: { schema: { type: 'object', additionalProperties: true }, render: (args, value) => [{ type: 'text', text: JSON.stringify(summarizeToolResult(args, value)) }] },
    timeoutMs: 60000,
    isConcurrencySafe: input => ['status', 'heartbeat'].includes(input?.action),
    async execute(args, exec) { return controller.execute(args, { actor: 'agent', ownerId: exec.agent?.id ?? 'agent' }); },
  });
}
