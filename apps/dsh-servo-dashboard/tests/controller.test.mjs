import test from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { PassThrough, Writable } from 'node:stream';
import { ServoController, validateRequest } from '../controller.js';

const state = (controlledIds = []) => ({connected:true,port:'/dev/cu.usbmodem1101',baud:1000000,updatedAt:1000,servos:[{id:1,model:1200,positionDeg:180,online:true,lastSeen:1000},{id:11,torque:true,online:true,lastSeen:1000}],controlledIds,error:null});
function fixture(handler = () => state()) {
  const calls = [];
  class Worker extends EventEmitter {
    constructor() {
      super(); this.stdout = new PassThrough(); this.stderr = new PassThrough();
      this.stdin = new Writable({write:(bytes, _encoding, callback) => {
        const req = JSON.parse(bytes); calls.push(req); callback();
        Promise.resolve().then(() => handler(req, this)).then(data => {
          if (data !== undefined) this.stdout.write(JSON.stringify({requestId:req.requestId,ok:true,data})+'\n');
        }, error => this.stdout.write(JSON.stringify({requestId:req.requestId,ok:false,error:error.message})+'\n'));
      }});
      this.stdin.on('finish', () => queueMicrotask(() => this.emit('exit', 0, null)));
    }
    publish(data) { this.stdout.write(JSON.stringify({event:'state',data})+'\n'); }
    kill(signal) { this.emit('exit', null, signal); }
  }
  const worker = new Worker();
  const controller = new ServoController({spawnWorker:()=>worker,clock:()=>1000});
  return { controller, worker, calls };
}
const agent = {actor:'agent',ownerId:'test-agent'};
const human = {actor:'human',ownerId:'test-human'};
const move = {action:'move',id:1,positionDeg:181,commandId:'test-command-001'};

test('agent safety bounds reject all invalid inputs before any I/O', async () => {
  const {controller,calls} = fixture();
  for(const input of [
    {...move,id:4},{...move,id:0},{...move,id:1.5}, {...move,positionDeg:190.001}, {...move,positionDeg:169.99},
    {...move,positionDeg:NaN},{...move,positionDeg:Infinity},{...move,positionDeg:null},{...move,positionDeg:'180'},
    {...move,commandId:''},{action:'anything'}, {action:'connect',port:'/tmp/x',baud:1000000},
    {action:'connect',port:'/dev/cu.usbmodem1101',baud:999}, {action:'scan',maxId:255},
  ]) await assert.rejects(controller.execute(input, agent));
  assert.equal(calls.length,0); await controller.dispose();
});

test('actor and unsupported policy fields cannot be injected by callers', () => {
  assert.deepEqual(validateRequest({...move,actor:'human',allowAll:true}, 'agent'), {action:'move',actor:'agent',id:1,positionDeg:181});
  assert.throws(()=>validateRequest({...move,id:11,actor:'human'},'agent'));
  assert.equal(validateRequest({...move,id:11}, 'human').id,11);
});

test('native read-only envelopes omit actor to satisfy the strict worker contract', () => {
  for (const input of [{action:'ports'},{action:'status'},{action:'scan',maxId:252},{action:'disconnect'},{action:'connect',port:'/dev/cu.usbmodem1101',baud:1000000}]) {
    assert.equal(Object.hasOwn(validateRequest(input,'agent'),'actor'),false);
  }
  assert.equal(validateRequest({...move,id:0},'human').id,0);
});

test('read-only status neither spawns worker nor sends heartbeat', async () => {
  const {controller,calls} = fixture();
  const result = await controller.execute({action:'status'},agent);
  assert.equal(result.transportAlive,false); assert.equal(calls.length,0);
  assert.deepEqual(await controller.execute({action:'heartbeat'},agent),{renewed:false});
  await controller.dispose();
});

test('moves share native operation and unique request IDs; duplicate never resends', async () => {
  const {controller,calls} = fixture(req => ({state:state([req.id]),command:{arrivalVerified:true}}));
  await controller.execute(move,agent); await controller.execute(move,agent);
  assert.equal(calls.length,1); assert.equal(calls[0].actor,'agent');
  assert.equal(controller.snapshot().owner,'agent');
  await assert.rejects(controller.execute({...move,positionDeg:182},agent),/different target/);
  assert.equal(calls.length,1); await controller.dispose();
});

test('uncertain or rejected command is cached and not retried', async () => {
  const {controller,calls} = fixture(()=>{throw new Error('UNKNOWN readback');});
  await assert.rejects(controller.execute(move,agent),/UNKNOWN/);
  await assert.rejects(controller.execute(move,agent),/UNKNOWN/);
  assert.equal(calls.length,1); assert.equal(controller.snapshot().audit[0].outcome,'failed');
  await controller.dispose();
});

test('only owning caller heartbeat renews a control lease', async () => {
  const {controller,calls} = fixture(req=>req.action==='heartbeat'?{renewed:true}:{state:state([1]),command:{arrivalVerified:true}});
  await controller.execute(move,agent);
  assert.deepEqual(await controller.execute({action:'heartbeat'},human),{renewed:false});
  await controller.execute({action:'heartbeat'},agent);
  assert.equal(calls.length,2); assert.equal(calls[1].action,'heartbeat');
  await assert.rejects(controller.execute({...move,commandId:'different-command'},human),/Another caller/);
  await controller.dispose();
});

test('progress snapshots before first write do not drop the motion owner', async () => {
  let finish;
  const {controller,worker} = fixture(req => req.action==='move'?new Promise(resolve=>{finish=resolve;}):{renewed:true});
  const pending = controller.execute(move,agent);
  await new Promise(resolve=>setImmediate(resolve));
  worker.publish(state([]));
  assert.equal(controller.snapshot().owner,'agent');
  assert.deepEqual(await controller.execute({action:'heartbeat'},agent),{renewed:true});
  finish({state:state([1]),command:{arrivalVerified:true}}); await pending;
  await controller.dispose();
});

test('human stop can stop agent-owned motors; agent cannot take over human control', async () => {
  const {controller,calls} = fixture(req=>({state:state(req.action==='stop'?[]:[1]),command:{arrivalVerified:true}}));
  await controller.execute(move,agent); await controller.execute({action:'stop'},human);
  assert.equal(calls.at(-1).actor,'human'); assert.equal(controller.snapshot().owner,null);
  await controller.execute({...move,commandId:'human-command'},human);
  await assert.rejects(controller.execute({action:'stop'},agent),/Another caller/);
  await assert.rejects(controller.execute({action:'disconnect'},agent),/Another caller/);
  await controller.dispose();
});

test('failed cleanup after explicit human release retains human ownership', async () => {
  const {controller,calls} = fixture(()=>({state:state([11])}));
  await controller.execute({action:'release',id:11},human);
  assert.equal(controller.snapshot().owner,'human');
  await assert.rejects(controller.execute({action:'disconnect'},agent),/Another caller/);
  assert.equal(calls.length,1); await controller.dispose();
});

test('full scan is refused while any plugin motor remains controlled', async () => {
  const {controller,calls} = fixture(()=>({state:state([1]),command:{arrivalVerified:true}}));
  await controller.execute(move,agent);
  await assert.rejects(controller.execute({action:'scan',maxId:252},agent),/Release/);
  assert.equal(calls.length,1); await controller.dispose();
});

test('worker loss retains unknown control and refuses automatic restart', async () => {
  const {controller,worker,calls} = fixture(()=>({state:state([1]),command:{arrivalVerified:true}}));
  await controller.execute(move,agent); worker.emit('exit',1,null);
  assert.equal(controller.snapshot().connected,false);
  assert.equal(controller.snapshot().servos.every(s=>s.stale),true);
  await assert.rejects(controller.execute({action:'ports'},agent),/UNKNOWN/);
  assert.equal(calls.length,1); await controller.dispose();
});

test('stale measurement flag depends on measurement timestamp, not status request', () => {
  const controller = new ServoController({clock:()=>5000});
  controller.acceptState(state());
  assert.equal(controller.snapshot().servos[0].stale,true);
  assert.equal(controller.snapshot().servos[0].lastSeen,1000);
});
