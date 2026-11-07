import test from 'node:test';
import assert from 'node:assert/strict';
import { apply, summarizeToolResult } from '../index.js';

function hostFixture() {
  let route, tool, service;
  apply({
    provide(name,value){assert.equal(name,'servoDashboard');service=value;},
    effect(){}, connection:{fetch:{register(value){route=value;}}},
    tools:{register(value){tool=value;}},
  });
  return {route,tool,service};
}
// The real Connection bridge rewrites Request.url, but preserves Host and Origin.
const request = (body, headers={}) => new Request('http://dsh.internal/api/servo-dashboard', {method:'POST',headers:{host:'127.0.0.1:3080','content-type':'application/json','x-servo-dashboard':'1',origin:'http://127.0.0.1:3080',...headers},body:typeof body==='string'?body:JSON.stringify(body)});

test('motion output preserves command facts and selected telemetry without unrelated rows', () => {
  const value={command:{arrivalVerified:false,unknown:true,error:'timeout'},state:{connected:true,controlledIds:[1],servos:[{id:1,online:true,positionDeg:180},{id:11,online:true,positionDeg:170}]}};
  const compact=summarizeToolResult({action:'move',id:1},value);
  assert.deepEqual(compact.command,value.command);
  assert.deepEqual(compact.state.servos,[value.state.servos[0]]);
  assert.deepEqual(compact.state.onlineIds,[1,11]);
  assert.strictEqual(summarizeToolResult({action:'status'},value),value);
  assert.ok(JSON.stringify(compact).length < JSON.stringify({...value,state:{...value.state,audit:new Array(50).fill('unrelated history')}}).length);
});

test('native route registers only scoped API path and JSON-only mutations', async () => {
  const {route,service} = hostFixture();
  assert.equal(route.path,'/api/servo-dashboard'); assert.deepEqual(route.methods,['GET','POST']);
  assert.equal((await route.fetch(request({action:'status',clientId:'test-client'},{'content-type':'text/plain'}))).status,403);
  assert.equal((await route.fetch(request({action:'status',clientId:'test-client'},{'x-servo-dashboard':'0'}))).status,403);
  assert.equal((await route.fetch(request({action:'status',clientId:'test-client'},{'sec-fetch-site':'cross-site'}))).status,403);
  assert.equal(service.child,null);
});

test('admitted browser POST works with rewritten carrier URL and does not open serial ports', async () => {
  const {route,service}=hostFixture();
  const response=await route.fetch(request({action:'status',clientId:'browser-probe'}));
  assert.equal(response.status,200);
  assert.equal((await response.json()).data.connected,false);
  assert.equal(service.child,null);
  assert.equal(service.snapshot().uiTransport.successfulPosts,1);
  assert.equal(service.snapshot().uiTransport.browserOrigin,'http://127.0.0.1:3080');
  assert.equal(service.snapshot().uiTransport.carrierOrigin,'http://dsh.internal');
});

test('GET is strictly read-only and does not open serial ports', async () => {
  const {route,service} = hostFixture();
  const response = await route.fetch(new Request('http://127.0.0.1:3080/api/servo-dashboard'));
  assert.equal(response.status,200); assert.equal(response.headers.get('cache-control'),'no-store');
  assert.equal((await response.json()).data.connected,false); assert.equal(service.child,null);
});

test('malformed, oversized and missing client identity requests fail closed', async () => {
  const {route,service} = hostFixture();
  for(const body of ['{',JSON.stringify({action:'status'}),JSON.stringify(null),JSON.stringify([])]) {
    const response = await route.fetch(request(body)); assert.equal(response.status,400);
  }
  assert.equal((await route.fetch(request('x'.repeat(4097)))).status,413);
  assert.equal(service.child,null);
});

test('UI and tool call the same controller with immutable caller identity', async () => {
  const {route,tool,service} = hostFixture();
  const calls=[]; service.execute=async (args,caller)=>{calls.push({args,caller});return {test:true};};
  await route.fetch(request({action:'status',clientId:'test-client',actor:'agent',ownerId:'injected'}));
  await tool.execute({action:'status',actor:'human'},{agent:{id:'real-agent'}});
  assert.deepEqual(calls[0].caller,{actor:'human',ownerId:'test-client'});
  assert.deepEqual(calls[1].caller,{actor:'agent',ownerId:'real-agent'});
  assert.equal(tool.name,'servo_dashboard');
  assert.deepEqual(tool.output.render({}, {ok:true}),[{type:'text',text:'{"ok":true}'}]);
});
