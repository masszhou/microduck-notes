import test from 'node:test';
import assert from 'node:assert/strict';
import { ServoController } from '../controller.js';

test('stop can overtake a move without leaving the Host permanently busy', async () => {
  const controller = new ServoController();
  const pending = new Map();
  controller.request = req => new Promise(resolve => pending.set(req.action, resolve));
  const move = controller.execute({action:'move',id:1,positionDeg:181,commandId:'concurrent-move'}, {actor:'agent',ownerId:'a'});
  const stop = controller.execute({action:'stop'}, {actor:'human',ownerId:'h'});
  assert.equal(controller.busy,'move');
  pending.get('move')({command:{outcome:'unknown',arrivalVerified:false}});
  await move;
  assert.equal(controller.busy,'stop');
  pending.get('stop')({controlledIds:[]}); await stop;
  assert.equal(controller.busy,null);
});

test('a rejected new target does not silently reuse a previous target or nonce', async () => {
  const controller = new ServoController(); let writes=0;
  controller.request = async () => {writes++; return {command:{outcome:'arrivalVerified'}};};
  await controller.execute({action:'move',id:1,positionDeg:181,commandId:'stable-command'}, {actor:'agent',ownerId:'a'});
  await assert.rejects(controller.execute({action:'move',id:1,positionDeg:182,commandId:'stable-command'}, {actor:'agent',ownerId:'a'}),/different target/);
  assert.equal(writes,1);
});
