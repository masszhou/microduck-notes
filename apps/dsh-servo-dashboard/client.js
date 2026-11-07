window.__ModuleLoader__.load({
  id: '@local/dsh-servo-dashboard',
  factory(require) {
    const React = require('react');
    const h = React.createElement;
    const { useState, useEffect, useRef } = React;
    const dictionary = {
      zh: {
        title: '舵机操作台', subtitle: 'OpenRB-150 · DYNAMIXEL XL330-M288 · Protocol 2.0',
        port: '串口', baud: 'Baud rate', refreshPorts: '刷新串口', connect: '连接并搜索', disconnect: '断开连接',
        quickScan: '搜索 ID 1–20', fullScan: '完整搜索 ID 0–252', scanning: '正在搜索',
        online: '在线', offline: '离线', connected: '已连接', disconnected: '未连接', controlled: '本插件控制',
        torqueOn: 'Torque 开启', refresh: '刷新约 3 Hz', readOnly: '连接、搜索和监测均只读。不会自动使能 Torque 或修改 EEPROM。',
        safety: '手动推送每次最多移动 10°。先确认机构周围没有障碍物。运动前的完整搜索检查有效期为 60 秒。',
        scanRequired: '运动检查未完成或已过期。请先释放本插件控制的 Torque，再执行完整搜索。',
        scanReady: '运动前总线检查有效',
        pollRecovered: '状态读取已恢复。此前发生通信故障；运动前仍需完整搜索。故障记录：',
        stop: '释放本插件控制的 Torque', stopHint: '仅关闭本插件接管的舵机 Torque。不是硬件急停；通信中断时请切断舵机电源。',
        preserve: '其他程序已开启的 Torque 不受连接、搜索或监测影响。',
        noPorts: '没有检测到串口', noServos: '连接 OpenRB-150 后，操作台在这里显示各个舵机的 Position。',
        noServosConnected: '没有找到在线舵机。请检查电源、Baud rate，以及 Wizard 是否已 Disconnect。',
        present: 'Present Position', goal: 'Goal Position', delta: '偏离 180°', voltage: '电压', temperature: '温度', current: '电流', velocity: '速度', torque: 'Torque',
        select: '选择舵机', target: '目标 Position', draft: '这里只编辑目标。点击推送后才会运动。',
        usePresent: '使用当前位置', acknowledge: '我已确认此舵机运动或卸力不会造成危险', send: '推送 Position', sending: '正在执行并核对',
        release: '释放此舵机 Torque', disableTorque: '关闭此舵机 Torque（卸力）', releaseWarning: '关闭 Torque 会卸力。对于其他程序已开启的 Torque，先完成总线检查并勾选安全确认。', stale: '数据过期', seconds: '秒前', lastSeen: '最近读取',
        details: '配置与诊断（只读）', mode: 'Operating Mode', model: '型号', firmware: 'Firmware', limits: 'Position limits',
        offset: 'Homing Offset', error: 'Hardware Error', led: 'LED', acceleration: 'Profile Acceleration', profileVelocity: 'Profile Velocity',
        history: '最近操作', noHistory: '尚未推送命令', result: '执行反馈', ready: '选择一个舵机以编辑目标 Position。',
        hold: '推送成功后保持 Torque。离开此操作台或停止心跳约 5 秒后，插件尝试释放接管的舵机。',
        staleWarning: '状态过期或舵机离线，已禁用推送。', invalidTarget: '目标必须是有效角度，且距离最新 Present Position 不超过 10°。',
        changed: '已读取状态', busy: '串口正在执行操作', all: '全部舵机', filter: '按 ID 搜索', clear: '清除',
        footer: 'v1.0.5 · 角度 = encoder ticks × 360 / 4096 · 不修改 ID、Baud rate、模式或限位',
        notSupported: '此型号或模式不支持 Position 推送', pulse: '状态时间',
      },
      en: {
        title: 'Servo dashboard', subtitle: 'OpenRB-150 · DYNAMIXEL XL330-M288 · Protocol 2.0',
        port: 'Serial port', baud: 'Baud rate', refreshPorts: 'Refresh ports', connect: 'Connect and discover', disconnect: 'Disconnect',
        quickScan: 'Scan IDs 1–20', fullScan: 'Full scan IDs 0–252', scanning: 'Scanning',
        online: 'Online', offline: 'Offline', connected: 'Connected', disconnected: 'Disconnected', controlled: 'Plugin controlled',
        torqueOn: 'Torque on', refresh: 'Telemetry ~3 Hz', readOnly: 'Connecting, scanning and monitoring are read-only. They never enable torque or change EEPROM.',
        safety: 'Each manual move is limited to 10°. Clear the mechanism before sending. Full-scan motion checks remain valid for 60 seconds.',
        scanRequired: 'Motion check missing or expired. Release plugin-controlled torque, then run a full scan.',
        scanReady: 'Pre-motion bus check valid',
        pollRecovered: 'Telemetry has recovered after a communication fault. A full scan is still required before motion. Previous fault:',
        stop: 'Release plugin-controlled torque', stopHint: 'Only releases motors taken over by this plugin. Not a hardware emergency stop. Cut servo power if communication is lost.',
        preserve: 'Connecting and monitoring preserve torque enabled by other software.',
        noPorts: 'No serial ports detected', noServos: 'Connect OpenRB-150 to see every servo position here.',
        noServosConnected: 'No online servos found. Check power, baud rate and that Wizard is disconnected.',
        present: 'Present Position', goal: 'Goal Position', delta: 'Offset from 180°', voltage: 'Voltage', temperature: 'Temperature', current: 'Current', velocity: 'Velocity', torque: 'Torque',
        select: 'Select servo', target: 'Target Position', draft: 'Editing only. The motor moves only when you press Send.',
        usePresent: 'Use present position', acknowledge: 'I have checked that moving or releasing this servo is safe', send: 'Send position', sending: 'Executing and verifying',
        release: 'Release this servo torque', disableTorque: 'Disable this servo torque', releaseWarning: 'Disabling torque removes holding force. For torque enabled by other software, complete the bus check and safety confirmation first.', stale: 'Stale', seconds: 's ago', lastSeen: 'Last read',
        details: 'Configuration and diagnostics (read-only)', mode: 'Operating Mode', model: 'Model', firmware: 'Firmware', limits: 'Position limits',
        offset: 'Homing Offset', error: 'Hardware Error', led: 'LED', acceleration: 'Profile Acceleration', profileVelocity: 'Profile Velocity',
        history: 'Recent operations', noHistory: 'No commands sent yet', result: 'Execution feedback', ready: 'Select a servo to edit its target position.',
        hold: 'Successful moves hold torque. Leaving this dashboard or losing its heartbeat for about 5 seconds releases plugin-controlled motors.',
        staleWarning: 'Telemetry is stale or the servo is offline. Sending is disabled.', invalidTarget: 'Enter a valid angle within 10° of the latest present position.',
        changed: 'State read', busy: 'Serial operation in progress', all: 'All servos', filter: 'Filter by ID', clear: 'Clear',
        footer: 'v1.0.5 · degrees = encoder ticks × 360 / 4096 · ID, baud, mode and limits are never changed',
        notSupported: 'Position control is unavailable for this model or mode', pulse: 'Telemetry timestamp',
      },
    };
    const CSS = `
      .servo-page{height:100%;min-height:0;overflow:auto;box-sizing:border-box;padding:28px clamp(18px,3vw,40px) 32px;color:var(--dsw-alias-label-primary);background:var(--dsw-alias-bg-base);font-size:13px;line-height:1.5}
      .servo-page *{box-sizing:border-box}.servo-wrap{max-width:1440px;margin:0 auto}.servo-head{display:flex;align-items:flex-start;justify-content:space-between;gap:16px;flex-wrap:wrap;margin-bottom:24px}
      .servo-page h1{font-size:22px;font-weight:600;line-height:30px;margin:0 0 4px}.servo-page h2{font-size:15px;font-weight:600;margin:0}.servo-muted{color:var(--dsw-alias-label-secondary)}
      .servo-toolbar{display:flex;gap:8px;align-items:flex-end;flex-wrap:wrap;padding:16px;border:1px solid var(--dsw-alias-border-l1);border-radius:12px;background:var(--dsw-alias-bg-layer-1)}
      .servo-page button,.servo-page input,.servo-page select{font:inherit;color:inherit}.servo-page button{border:1px solid var(--dsw-alias-border-l2);border-radius:7px;padding:7px 12px;min-height:34px;background:var(--dsw-alias-bg-base);cursor:pointer;white-space:nowrap}
      .servo-page button:disabled{opacity:.42;cursor:not-allowed}.servo-page button:hover:not(:disabled){background:var(--dsw-alias-bg-layer-2)}.servo-page button:focus-visible,.servo-page input:focus-visible,.servo-page select:focus-visible{outline:2px solid var(--dsw-alias-brand-primary);outline-offset:2px}
      .servo-page .servo-primary{border-color:var(--dsw-alias-brand-primary);color:var(--dsw-alias-brand-primary);font-weight:600}.servo-page .servo-stop{color:var(--dsw-alias-state-error-primary);border-color:var(--dsw-alias-state-error-primary)}
      .servo-page input:not([type=checkbox]),.servo-page select{border:1px solid var(--dsw-alias-border-l2);border-radius:7px;background:var(--dsw-alias-bg-base);min-height:34px;padding:6px 10px;max-width:100%}.servo-field{display:flex;flex-direction:column;gap:5px}.servo-field>span{font-size:11px;color:var(--dsw-alias-label-secondary)}
      .servo-port{min-width:210px;flex:1}.servo-note{font-size:12px;margin:10px 0;color:var(--dsw-alias-label-secondary)}.servo-summary{display:flex;flex-wrap:wrap;gap:10px;margin:20px 0}.servo-stat{padding:12px 18px;background:var(--dsw-alias-bg-layer-1);border:1px solid var(--dsw-alias-border-l1);border-radius:10px;min-width:120px}.servo-stat strong{font-size:22px;line-height:1.1;font-variant-numeric:tabular-nums;margin-right:8px}
      .servo-grid{display:grid;grid-template-columns:minmax(0,1fr) 300px;gap:20px;align-items:start}.servo-panel{border:1px solid var(--dsw-alias-border-l1);border-radius:12px;overflow:hidden;background:var(--dsw-alias-bg-layer-1)}.servo-section-head{display:flex;gap:10px;align-items:center;justify-content:space-between;padding:14px 16px;border-bottom:1px solid var(--dsw-alias-border-l1)}
      .servo-table-scroll{overflow:auto;max-height:720px}.servo-page table{border-collapse:collapse;width:100%;white-space:nowrap;font-variant-numeric:tabular-nums}.servo-page th{font-size:10px;font-weight:500;color:var(--dsw-alias-label-secondary);background:var(--dsw-alias-bg-layer-1);position:sticky;top:0;z-index:1;padding:10px;text-align:right;border-bottom:1px solid var(--dsw-alias-border-l1)}.servo-page th:first-child{text-align:left;padding-left:16px}.servo-page td{padding:9px 10px;border-bottom:1px solid var(--dsw-alias-border-l1);text-align:right}.servo-page tr:last-child td{border-bottom:none}.servo-page tr[data-selected=true]{background:var(--dsw-alias-bg-layer-2)}.servo-page tr[data-stale=true]{color:var(--dsw-alias-label-secondary)}.servo-page td:first-child{text-align:left;padding-left:12px}.servo-page .servo-id{padding:4px 6px;min-height:28px;border:0;background:none;text-align:left}.servo-position{font-size:17px;font-weight:600;color:var(--dsw-alias-label-primary)}.servo-small{font-size:10px}.servo-dot{display:inline-block;width:6px;height:6px;border-radius:50%;margin-right:6px;background:var(--dsw-alias-state-idle-primary)}.servo-good{color:var(--dsw-alias-state-success-primary)}.servo-dot[data-live=true]{background:var(--dsw-alias-state-success-primary)}
      .servo-badge{display:inline-flex;align-items:center;padding:3px 8px;border:1px solid var(--dsw-alias-border-l1);border-radius:20px;font-size:11px}.servo-bad{color:var(--dsw-alias-state-error-primary)}.servo-warn{color:var(--dsw-alias-state-warn-primary)}.servo-control{padding:18px;position:sticky;top:0}.servo-control h2{margin-bottom:12px}.servo-live-value{font-size:38px;font-weight:600;line-height:1.3;letter-spacing:-1px;font-variant-numeric:tabular-nums}.servo-live-value small{font-size:16px;letter-spacing:0}.servo-pair{display:flex;justify-content:space-between;gap:8px;margin:6px 0;font-variant-numeric:tabular-nums}.servo-rule{height:1px;background:var(--dsw-alias-border-l1);margin:16px 0}.servo-target{display:flex;gap:6px;align-items:center;margin:8px 0}.servo-target input{min-width:0;width:100%;font-size:20px!important;text-align:center;font-variant-numeric:tabular-nums}.servo-target button{padding:6px 9px}.servo-check{display:flex;align-items:flex-start;gap:7px;font-size:12px;margin:14px 0}.servo-check input{margin-top:3px;accent-color:var(--dsw-alias-brand-primary)}.servo-wide{width:100%;margin:4px 0}.servo-message{padding:10px 12px;border:1px solid var(--dsw-alias-border-l2);border-radius:8px;margin:12px 0;overflow-wrap:anywhere}.servo-error{border-color:var(--dsw-alias-state-error-primary);color:var(--dsw-alias-state-error-primary)}.servo-warning{border-color:var(--dsw-alias-state-warn-primary);color:var(--dsw-alias-state-warn-primary)}
      .servo-empty{padding:50px 24px;text-align:center;color:var(--dsw-alias-label-secondary);white-space:normal}.servo-page details{margin-top:14px}.servo-page summary{cursor:pointer;font-size:12px}.servo-page dl{display:grid;grid-template-columns:1fr auto;gap:6px;font-size:11px;max-width:100%}.servo-page dt{color:var(--dsw-alias-label-secondary)}.servo-page dd{margin:0;overflow-wrap:anywhere;text-align:right}.servo-history{margin-top:20px}.servo-log{padding:8px 16px;border-top:1px solid var(--dsw-alias-border-l1);font-size:12px;display:flex;gap:14px;align-items:flex-start}.servo-log pre{font:inherit;white-space:pre-wrap;overflow-wrap:anywhere;margin:0}.servo-footer{font-size:11px;margin-top:20px;color:var(--dsw-alias-label-secondary)}
      @media(max-width:1000px){.servo-grid{grid-template-columns:minmax(0,1fr)}.servo-control{position:static}.servo-table-scroll{max-height:580px}.servo-port{min-width:180px}}
      @media(pointer:coarse){.servo-page input,.servo-page select{font-size:16px}}
      @media(prefers-reduced-motion:reduce){.servo-page *{scroll-behavior:auto}}
    `;
    const fmt = (number, digits = 1) => typeof number === 'number' && Number.isFinite(number) ? number.toFixed(digits) : '—';
    function Icon({ size = 20 }) {
      return h('svg', { width: size, height: size, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', strokeWidth: 1.6, 'aria-hidden': true },
        h('rect', { x: 2, y: 3, width: 20, height: 18, rx: 4 }), h('circle', { cx: 10, cy: 12, r: 5.5 }), h('path', { d: 'M10 12l3-4m4 2h2m-2 4h2', strokeLinecap: 'round' }));
    }
    async function requestHost(input, clientId, options = {}) {
      const response = await fetch('/api/servo-dashboard', {
        method: input ? 'POST' : 'GET', credentials: 'same-origin', cache: 'no-store',
        headers: input ? { 'content-type': 'application/json', 'x-servo-dashboard': '1' } : {},
        body: input ? JSON.stringify({ ...input, clientId }) : undefined,
        signal: options.signal,
      });
      return response.json();
    }
    return {
      inject: ['slots', 'locale'],
      apply(ctx) {
        ctx.effect(() => ctx.locale.register('servo-dashboard.ui', 'zh', dictionary.zh));
        ctx.effect(() => ctx.locale.register('servo-dashboard.ui', 'en', dictionary.en));
        const t = ctx.locale.bind('servo-dashboard.ui');
        // Verify the real authenticated browser POST path without opening the bus
        // or sending a heartbeat/motion. Reuse the exact dashboard request helper.
        ctx.effect(() => {
          const abort = new AbortController();
          requestHost({ action: 'status' }, `probe_${crypto.randomUUID()}`, { signal: abort.signal })
            .then(value => { if (!value.ok) throw new Error(value.error); })
            .catch(error => { if (error.name !== 'AbortError') console.warn('[servo-dashboard] POST readiness check failed:', error.message); });
          return () => abort.abort();
        });
        function Dashboard() {
          React.useSyncExternalStore(fn => ctx.locale.subscribe(fn), () => ctx.locale.getSnapshot());
          const [state, setState] = useState({ connected: false, servos: [], controlledIds: [], audit: [] });
          const [ports, setPorts] = useState([]); const [port, setPort] = useState(''); const [baud, setBaud] = useState(1000000);
          const [selected, setSelected] = useState(null); const [target, setTarget] = useState(''); const [ack, setAck] = useState(false);
          const [error, setError] = useState(''); const [feedback, setFeedback] = useState(null); const [busy, setBusy] = useState(''); const [filter, setFilter] = useState(''); const [networkOk, setNetworkOk] = useState(false);
          const clientId = useRef(crypto.randomUUID()); const mounted = useRef(true); const busyRef = useRef(false); const requestCount = useRef(0);
          useEffect(() => {
            if (state.connected) { setPort(state.port); setBaud(state.baud); }
          }, [state.connected, state.port, state.baud]);
          const accept = data => {
            const next = data?.state || (typeof data?.connected === 'boolean' ? data : null);
            if (next && mounted.current) setState(prev => (next.sequence ?? 0) < (prev.sequence ?? 0) ? prev : next);
          };
          const api = async (input, options = {}) => {
            const value = await requestHost(input, clientId.current, options);
            if (!value.ok) { accept(value); throw new Error(value.error || 'Dashboard request failed'); }
            accept(value.data); return value.data;
          };
          const refreshPorts = async () => {
            try {
              const result = await api({ action: 'ports' });
              if (!mounted.current) return;
              const found = result.ports.filter(p => p.path.startsWith('/dev/cu.'));
              setPorts(found); setPort(prev => found.some(p => p.path === prev) ? prev : (found.find(p => /openrb|robotis/i.test(`${p.product} ${p.manufacturer}`))?.path || found[0]?.path || ''));
            } catch (e) { if (mounted.current) setError(e.message); }
          };
          useEffect(() => {
            mounted.current = true;
            let timer, heartbeatTimer; const abort = new AbortController();
            const poll = async () => {
              try { await api(null, { signal: abort.signal }); if (mounted.current) setNetworkOk(true); }
              catch (e) { if (mounted.current && e.name !== 'AbortError') { setNetworkOk(false); setError(e.message); } }
              if (mounted.current) timer = setTimeout(poll, 500);
            };
            const heartbeat = async () => {
              if (document.visibilityState === 'visible') {
                try { await api({ action: 'heartbeat' }, { signal: abort.signal }); }
                catch (e) { if (mounted.current && e.name !== 'AbortError') setError(e.message); }
              }
              if (mounted.current) heartbeatTimer = setTimeout(heartbeat, 1200);
            };
            poll(); heartbeat(); refreshPorts();
            return () => { mounted.current = false; abort.abort(); clearTimeout(timer); clearTimeout(heartbeatTimer); };
          }, []);
          const run = async (input) => {
            if (busyRef.current && !['stop', 'release'].includes(input.action)) return;
            const generation = ++requestCount.current;
            busyRef.current = true; setBusy(input.action); setError('');
            try {
              const data = await api(input);
              if (mounted.current) setFeedback({ at: Date.now(), action: input.action, data: data.command || (['stop', 'release', 'disconnect'].includes(input.action) ? { outcome: data.state?.controlledIds?.length ? 'cleanupPending' : 'released', controlledIds: data.state?.controlledIds || [] } : { outcome: 'completed' }) });
            } catch (e) { if (mounted.current) setError(e.message); }
            finally { if (requestCount.current === generation) { busyRef.current = false; if (mounted.current) setBusy(''); } }
          };
          const rows = state.servos || []; const motor = rows.find(s => s.id === selected);
          const online = rows.filter(s => s.online && !s.stale); const controlled = state.controlledIds || [];
          const portOptions = state.connected && state.port && !ports.some(p => p.path === state.port) ? [{ path: state.port }, ...ports] : ports;
          const pollRecovered = networkOk && state.connected && /^ID \d+ poll failed:/.test(state.error || '') && rows.length > 0 && rows.every(s => s.online && !s.stale && !s.error) && controlled.length === 0;
          const stale = !networkOk || !state.connected || !motor?.online || motor?.stale || (state.serverTime - motor.lastSeen > 2500);
          const supported = motor?.model === 1200 && motor?.operatingMode === 3;
          const angle = target.trim() === '' ? NaN : Number(target);
          const valid = Number.isFinite(angle) && angle >= 0 && angle <= 4095 * 360 / 4096 && Math.abs(angle - motor?.positionDeg) <= 10;
          const select = id => {
            const row = rows.find(s => s.id === id); setSelected(id); setTarget(fmt(row?.positionDeg, 2)); setAck(false); setFeedback(null);
          };
          const step = delta => {
            const value = Number.isFinite(angle) ? angle : motor?.positionDeg;
            if (typeof value === 'number') setTarget((value + delta).toFixed(2));
          };
          const submit = e => {
            e.preventDefault();
            if (stale || !supported || !valid || !ack || !state.fullScanValid || busyRef.current) return;
            run({ action: 'move', id: selected, positionDeg: angle, commandId: crypto.randomUUID() });
          };
          const button = (text, action, disabled, className, title) => h('button', { type: 'button', onClick: action, disabled, className, title }, text);
          const pair = (label, value) => h('div', { className: 'servo-pair' }, h('span', { className: 'servo-muted' }, label), h('span', null, value));
          const detailRows = motor ? [
            [t('model'), motor.model === 1200 ? 'XL330-M288' : String(motor.model)], [t('firmware'), motor.firmware],
            [t('mode'), `${motor.operatingMode} ${motor.operatingMode === 3 ? '(Position)' : ''}`], ['Drive Mode', motor.driveMode],
            [t('limits'), `${fmt(motor.minPositionTicks * 360 / 4096)}° – ${fmt(motor.maxPositionTicks * 360 / 4096)}°`],
            [t('offset'), `${motor.homingOffset} ticks`], [t('profileVelocity'), `${motor.profileVelocity} raw`],
            [t('acceleration'), `${motor.profileAcceleration} raw`], ['Bus Watchdog', `${motor.watchdog} raw`],
            [t('error'), `0x${Number(motor.hardwareError || 0).toString(16).padStart(2, '0')}`], [t('led'), motor.led ? 'ON' : 'OFF'],
            ['PWM', `${fmt(motor.pwmPercent)} %`], ['Encoder', `${motor.positionTicks} ticks`],
          ] : [];
          return h('div', { className: 'servo-page', 'data-servo-dashboard-version': '1.0.5' }, h('style', null, CSS), h('div', { className: 'servo-wrap' },
            h('header', { className: 'servo-head' }, h('div', null, h('h1', null, t('title')), h('div', { className: 'servo-muted' }, t('subtitle'))),
              h('div', null, h('span', { className: 'servo-badge' }, h('span', { className: 'servo-dot', 'data-live': state.connected }), t(state.connected ? 'connected' : 'disconnected')), h('div', { className: 'servo-small servo-muted', style: { marginTop: 5 } }, state.port || t('refresh')))),
            h('section', { className: 'servo-toolbar', 'aria-label': t('connect') },
              h('label', { className: 'servo-field servo-port' }, h('span', null, t('port')), h('select', { value: port, onChange: e => setPort(e.target.value), disabled: state.connected || !!busy }, portOptions.length ? portOptions.map(p => h('option', { key: p.path, value: p.path }, `${p.product || p.path} · ${p.path}`)) : h('option', { value: '' }, t('noPorts')))),
              h('label', { className: 'servo-field' }, h('span', null, t('baud')), h('select', { value: baud, onChange: e => setBaud(Number(e.target.value)), disabled: state.connected || !!busy }, [57600,115200,1000000,2000000,3000000,4000000].map(b => h('option', { key: b, value: b }, b >= 1000000 ? `${b / 1000000} Mbps` : `${b} bps`)))),
              button(t('refreshPorts'), refreshPorts, !!busy),
              button(t(state.connected ? 'disconnect' : 'connect'), () => run(state.connected ? { action: 'disconnect' } : { action: 'connect', port, baud }), !!busy || (!state.connected && !port), 'servo-primary'),
              button(t('fullScan'), () => run({ action: 'scan', maxId: 252 }), !state.connected || !!busy || controlled.length > 0),
              button(t('quickScan'), () => run({ action: 'scan', maxId: 20 }), !state.connected || !!busy || controlled.length > 0)),
            h('p', { className: 'servo-note' }, t('readOnly')),
            error ? h('div', { className: 'servo-message servo-error', role: 'alert' }, error) : null,
            state.error ? h('div', { className: `servo-message ${pollRecovered ? 'servo-warning' : 'servo-error'}`, role: 'alert' }, pollRecovered ? `${t('pollRecovered')} ${state.error}` : state.error) : null,
            busy || state.busy ? h('div', { className: 'servo-message', role: 'status' }, t(busy === 'scan' || state.busy === 'scan' ? 'scanning' : 'busy'), ' · ', busy || state.busy) : null,
            h('div', { className: 'servo-summary' },
              h('div', { className: 'servo-stat' }, h('strong', { className: 'servo-good' }, online.length), h('span', { className: 'servo-muted' }, t('online'))),
              h('div', { className: 'servo-stat' }, h('strong', null, rows.filter(s => s.torque && s.online).length), h('span', { className: 'servo-muted' }, t('torqueOn'))),
              h('div', { className: 'servo-stat' }, h('strong', null, controlled.length), h('span', { className: 'servo-muted' }, t('controlled'))),
              h('div', { style: { marginLeft: 'auto', alignSelf: 'center' } }, button(t('stop'), () => run({ action: 'stop' }), !controlled.length, 'servo-stop', t('stopHint')))),
            h('div', { className: 'servo-grid' },
              h('section', { className: 'servo-panel' },
                h('div', { className: 'servo-section-head' }, h('h2', null, t('all')), h('input', { type: 'search', value: filter, onChange: e => setFilter(e.target.value), placeholder: t('filter'), 'aria-label': t('filter'), style: { width: 120 } })),
                rows.length ? h('div', { className: 'servo-table-scroll' }, h('table', null,
                  h('thead', null, h('tr', null, ['ID', t('present'), t('goal'), t('delta'), t('torque'), t('current'), t('voltage'), t('temperature')].map(label => h('th', { key: label, scope: 'col' }, label)))),
                  h('tbody', null, rows.filter(s => String(s.id).includes(filter.trim())).sort((a,b) => a.id-b.id).map(s => h('tr', { key: s.id, onClick: () => select(s.id), style: { cursor: 'pointer' }, 'data-selected': s.id === selected, 'data-stale': s.stale || !s.online },
                    h('td', null, h('button', { type: 'button', className: 'servo-id', onClick: () => select(s.id), 'aria-label': `${t('select')} ${s.id}`, 'aria-pressed': s.id === selected }, h('span', { className: 'servo-dot', 'data-live': s.online && !s.stale }), String(s.id).padStart(2, '0')), !s.online || s.stale ? h('div', { className: 'servo-small servo-warn' }, t(s.online ? 'stale' : 'offline')) : null),
                    h('td', { className: 'servo-position' }, fmt(s.positionDeg, 2), '°'), h('td', null, fmt(s.goalDeg, 2), '°'),
                    h('td', { className: 'servo-muted' }, fmt(s.positionDeg - 180, 2), '°'),
                    h('td', null, h('span', { className: s.torque ? 'servo-good' : 'servo-muted' }, s.torque ? 'ON' : 'OFF'), controlled.includes(s.id) ? h('div', { className: 'servo-small' }, t('controlled')) : null),
                    h('td', null, fmt(s.currentMa, 0), ' mA'), h('td', null, fmt(s.voltage), ' V'), h('td', { className: s.temperature >= 55 ? 'servo-bad' : '' }, fmt(s.temperature, 0), ' °C')))))) :
                  h('div', { className: 'servo-empty' }, t(state.connected ? 'noServosConnected' : 'noServos'))),
              h('aside', { className: 'servo-panel servo-control' }, motor ? h(React.Fragment, null,
                h('h2', null, `ID ${String(motor.id).padStart(2, '0')} · ${motor.model === 1200 ? 'XL330-M288' : motor.model}`),
                h('div', { className: 'servo-muted servo-small' }, t('present')),
                h('div', { className: 'servo-live-value' }, fmt(motor.positionDeg, 2), h('small', null, ' °')),
                pair(t('goal'), `${fmt(motor.goalDeg, 2)}°`), pair(t('velocity'), `${fmt(motor.velocityRpm, 2)} RPM`),
                pair(t('lastSeen'), `${fmt(Math.max(0, (state.serverTime - motor.lastSeen) / 1000), 1)} ${t('seconds')}`),
                h('div', { className: 'servo-rule' }),
                h('form', { onSubmit: submit },
                  h('label', { htmlFor: 'servo-target-position' }, t('target'), ' (°)'),
                  h('div', { className: 'servo-target' }, button('−1°', () => step(-1), !!busy),
                    h('input', { id: 'servo-target-position', type: 'number', step: '0.01', min: 0, max: 359.912, value: target, onChange: e => setTarget(e.target.value), disabled: !!busy, inputMode: 'decimal' }), button('+1°', () => step(1), !!busy)),
                  button(t('usePresent'), () => setTarget(fmt(motor.positionDeg, 2)), stale || !!busy),
                  h('p', { className: 'servo-note' }, t('draft')),
                  h('div', { className: 'servo-message servo-warning servo-small' }, t('safety')),
                  h('p', { className: state.fullScanValid ? 'servo-good servo-small' : 'servo-warn servo-small' }, t(state.fullScanValid ? 'scanReady' : 'scanRequired')),
                  state.aliasSafetyError ? h('p', { className: 'servo-small servo-warn' }, state.aliasSafetyError) : null,
                  stale ? h('p', { className: 'servo-warn', role: 'status' }, t('staleWarning')) : !supported ? h('p', { className: 'servo-warn' }, t('notSupported')) : !valid ? h('p', { className: 'servo-warn' }, t('invalidTarget')) : null,
                  motor.error ? h('p', { className: 'servo-bad' }, motor.error) : null,
                  h('label', { className: 'servo-check' }, h('input', { type: 'checkbox', checked: ack, onChange: e => setAck(e.target.checked), disabled: !!busy }), t('acknowledge')),
                  h('button', { type: 'submit', className: 'servo-primary servo-wide', disabled: !ack || !valid || stale || !supported || !state.fullScanValid || !!busy || !!state.busy }, `${t(busy === 'move' ? 'sending' : 'send')} → ID ${motor.id}`)),
                button(t(controlled.includes(motor.id) ? 'release' : 'disableTorque'), () => run({ action: 'release', id: motor.id }), !controlled.includes(motor.id) && (!motor.torque || !supported || !ack || !state.fullScanValid || stale || !!busy), 'servo-wide'),
                !controlled.includes(motor.id) && motor.torque ? h('p', { className: 'servo-note servo-warn' }, t('releaseWarning')) : null,
                h('p', { className: 'servo-note' }, t('hold')),
                h('details', null, h('summary', null, t('details')), h('dl', null, detailRows.flatMap(([key,value]) => [h('dt', { key: `k-${key}` }, key), h('dd', { key: `v-${key}` }, value)])))) : h('div', { className: 'servo-empty' }, t('ready')))),
            feedback ? h('div', { className: 'servo-message', role: 'status' }, h('strong', null, t('result')), ' · ', feedback.action, h('pre', { style: { whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', fontSize: 12, margin: '6px 0 0' } }, JSON.stringify(feedback.data, null, 2))) : null,
            h('section', { className: 'servo-panel servo-history' }, h('div', { className: 'servo-section-head' }, h('h2', null, t('history'))),
              state.audit?.length ? state.audit.slice(-8).reverse().map((entry, i) => h('div', { className: 'servo-log', key: `${entry.at}-${i}` }, h('span', { className: 'servo-muted' }, new Date(entry.at).toLocaleTimeString()), h('span', { className: entry.outcome === 'failed' ? 'servo-bad' : '' }, entry.action), h('pre', null, entry.detail ? JSON.stringify(entry.detail) : entry.outcome))) : h('div', { className: 'servo-log servo-muted' }, t('noHistory'))),
            h('p', { className: 'servo-note' }, t('stopHint')), h('p', { className: 'servo-note' }, t('preserve')),
            h('footer', { className: 'servo-footer' }, t('footer'))));
        }
        ctx.slots.inject('sidebar.panellist', () => ctx.slots.register({ name: 'sidebar.panellist', id: 'servo-dashboard', order: 20, label: () => t('title') }, Icon));
        ctx.slots.inject('main', () => ctx.slots.register({ name: 'main', key: 'servo-dashboard', locale: 'servo-dashboard.ui' }, Dashboard));
      },
    };
  },
});
