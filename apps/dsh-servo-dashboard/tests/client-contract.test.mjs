import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const source=readFileSync(new URL('../client.js',import.meta.url),'utf8');
test('browser readiness check is read-only and shares the actual dashboard request helper',()=>{
  assert.match(source,/requestHost\(\{ action: 'status' \}, `probe_/);
  assert.match(source,/await requestHost\(input, clientId\.current, options\)/);
  assert.equal(source.match(/fetch\('\/api\/servo-dashboard'/g).length,1);
});
test('connected port remains visible and recovered poll faults retain the scan requirement',()=>{
  assert.match(source,/portOptions\.length \? portOptions\.map/);
  assert.match(source,/button\(t\('refreshPorts'\), refreshPorts, !!busy\)/);
  assert.match(source,/pollRecovered \? 'servo-warning' : 'servo-error'/);
  assert.match(source,/A full scan is still required before motion/);
  assert.match(source,/h\('tr', \{ key: s\.id, onClick: \(\) => select\(s\.id\)/);
});
test('each locale registration returns its disposer to a context effect',()=>{
  const lines=source.split('\n').filter(line=>line.includes('ctx.locale.register('));
  assert.equal(lines.length,2);
  for(const line of lines) assert.match(line,/ctx\.effect\(\(\) => ctx\.locale\.register\('servo-dashboard\.ui'/);
  assert.match(source,/ctx\.locale\.bind\('servo-dashboard\.ui'\)/);
});
test('client uses owned native slots and no iframe, root replacement, or internal UI import',()=>{
  assert.match(source,/ctx\.slots\.inject\('sidebar\.panellist'/);
  assert.match(source,/ctx\.slots\.inject\('main'/);
  assert.doesNotMatch(source,/require\(['"]@deepseek-ai\//);
  assert.doesNotMatch(source,/document\.body|createRoot|iframe/);
});
test('client preserves explicit send and blocks stale or unchecked actions',()=>{
  assert.match(source,/if \(stale \|\| !supported \|\| !valid \|\| !ack \|\| !state\.fullScanValid/);
  assert.match(source,/onSubmit: submit/);
  assert.match(source,/setNetworkOk\(false\)/);
  assert.match(source,/crypto\.randomUUID\(\)/);
});
