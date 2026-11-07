// Run with the actual DSH Connection source directory as the first argument.
// Exercises its real trust fence and HTTP bridge, not a copied implementation.
import assert from 'node:assert/strict';
import { Readable } from 'node:stream';
import { EventEmitter } from 'node:events';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { apply } from '../index.js';
assert.ok(process.argv[2], 'Supply the DSH packages/client/connection/src directory');
const root=resolve(process.argv[2]);
const { bridge }=await import(pathToFileURL(resolve(root,'http-bridge.ts')));
const { isTrustedApiRequest }=await import(pathToFileURL(resolve(root,'api-request-trust.ts')));
let route,controller;
apply({provide(_name,value){controller=value;},effect(){},connection:{fetch:{register(value){route=value;}}},tools:{register(){}}});
const headers={host:'127.0.0.1:3080',origin:'http://127.0.0.1:3080','content-type':'application/json','x-servo-dashboard':'1','sec-fetch-site':'same-origin'};
assert.equal(isTrustedApiRequest({headers},[]),true);
for(const overrides of [{origin:'http://evil.invalid'},{origin:'null'},{host:'evil.invalid'},{'sec-fetch-site':'cross-site'}]) {
  assert.equal(isTrustedApiRequest({headers:{...headers,...overrides}},[]),false);
}
const req=Readable.from([Buffer.from(JSON.stringify({action:'status',clientId:'bridge-regression'}))]);
Object.assign(req,{url:'/api/servo-dashboard',method:'POST',headers});
const res=new EventEmitter();const chunks=[];
Object.assign(res,{destroyed:false,writableEnded:false,writeHead(status){this.status=status;},write(chunk){chunks.push(Buffer.from(chunk));return true;},end(){this.writableEnded=true;}});
let observedUrl;
await bridge(req,res,{requestBodyMode(){return 'buffered';},fetch(request){observedUrl=request.url;return route.fetch(request);}},{});
assert.equal(observedUrl,'http://dsh.internal/api/servo-dashboard');
assert.equal(res.status,200);
assert.equal(JSON.parse(Buffer.concat(chunks).toString()).ok,true);
assert.equal(controller.child,null,'read-only POST must not start the native worker');
assert.equal(controller.uiTransport.successfulPosts,1);
assert.equal(controller.uiTransport.browserOrigin,'http://127.0.0.1:3080');
assert.equal(controller.uiTransport.carrierOrigin,'http://dsh.internal');
console.log('PASS actual DSH trust fence + HTTP bridge + dashboard route: valid browser origin succeeds; cross-site/opaque/rebound origins rejected; no serial I/O.');
