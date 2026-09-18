import {test,after} from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {handle,closeAllSessions} from '../src/app-handler.mjs';

const root=fileURLToPath(new URL('..',import.meta.url));
const dir=fs.mkdtempSync(path.join(os.tmpdir(),'cu-app-targeting-'));
const binary=path.join(dir,'Contents','MacOS','accessibility');
const log=path.join(dir,'native.jsonl');
const saved={};
for(const key of ['CODEWHALE_CU_TEST_BACKEND','CODEWHALE_CU_APP_BUNDLE','CODEWHALE_CU_RECORDINGS_DIR','CU_TARGETING_CALLS','CU_TARGETING_NATIVE']) saved[key]=process.env[key];
process.env.CODEWHALE_CU_TEST_BACKEND=path.join(root,'tests/fixtures/darwin-targeting-backend.mjs');
process.env.CODEWHALE_CU_APP_BUNDLE=dir;
process.env.CODEWHALE_CU_RECORDINGS_DIR=path.join(dir,'recordings');
process.env.CU_TARGETING_CALLS=log;
delete process.env.CU_TARGETING_NATIVE;
fs.mkdirSync(path.dirname(binary),{recursive:true});fs.writeFileSync(binary,'');
after(async()=>{
  await closeAllSessions();
  for(const [key,value] of Object.entries(saved)) if(value===undefined) delete process.env[key]; else process.env[key]=value;
  fs.rmSync(dir,{recursive:true,force:true});
});
const call=(tool,args)=>handle({tool,args},{sessionId:'targeting-fixture'});
const calls=()=>fs.readFileSync(log,'utf8').trim().split('\n').map(JSON.parse);

test('app-handler forwards public list_windows arguments through the real Darwin backend',async()=>{
  const ref={pid:123,name:'Fixture',bundle_id:'test.fixture'};
  const reply=await call('list_windows',{app_ref:ref});
  assert.equal(reply.ok,true,JSON.stringify(reply));
  assert.deepEqual(calls().at(-1).args.app_ref,ref);
  await call('list_windows',{});
  assert.equal(Object.hasOwn(calls().at(-1).args,'app_ref'),false,'omission must reach the resolver as omission');
  for(const tool of ['get_app_state','resolve_element']) {
    await call(tool,{});
    assert.equal(Object.hasOwn(calls().at(-1).args,'app_ref'),false,`${tool} must not synthesize an empty reference`);
  }
});

test('explicit null screenshot target reaches app resolution before capture',async()=>{
  const reply=await call('screenshot',{app_ref:null});
  assert.equal(reply.ok,false,'the fixture refuses any attempt to run a capture command');
  assert.equal(calls().at(-1).tool,'window_info');
  assert.equal(calls().at(-1).args.app_ref,null);
});

test('real handler/backend/native resolver never redirects an explicit app reference to frontmost',{skip:process.platform!=='darwin'},async()=>{
  const build=spawnSync('clang',['-fobjc-arc','-Os','-framework','Cocoa','-framework','ApplicationServices','-framework','ScreenCaptureKit','-framework','AVFoundation','-framework','CoreMedia','-framework','Vision',path.join(root,'tests/fixtures/darwin-targeting.m'),'-o',binary],{encoding:'utf8'});
  assert.equal(build.status,0,build.stderr);
  const guarded=spawnSync(binary,[JSON.stringify({tool:'inspect_pointer_guard',args:{lock_dir:dir}})],{encoding:'utf8'});
  assert.equal(guarded.status,0,guarded.stderr);
  const guard=JSON.parse(guarded.stdout);
  assert.match(guard.refusal,/foreground changed to Other/);
  assert.equal(guard.posts,0,'a stale foreground binding cannot post a global mouse gesture');
  assert.equal(guard.activations,0,'a pointer gesture cannot reclaim the user foreground');
  process.env.CU_TARGETING_NATIVE='1';
  try {
    const implicit=await call('list_windows',{});
    assert.equal(implicit.ok,true,JSON.stringify(implicit));assert.equal(implicit.data.pid,999);
    for(const app_ref of [{pid:123},{name:'fixture'},{bundle_id:'test.fixture'},{pid:123,name:'Fixture',bundle_id:'test.fixture'}]) {
      const reply=await call('list_windows',{app_ref});
      assert.equal(reply.ok,true,JSON.stringify(reply));assert.equal(reply.data.pid,123);
    }
    const invalid=[null,{},[],false,'Fixture',123,{app_ref:{pid:123}},{unexpected:123},{pid:123,unexpected:true},{pid:null},{pid:true},{pid:'123'},{pid:0},{pid:-1},{pid:1.5},{pid:4294967419},{name:''},{name:'  \n'},{name:123},{bundle_id:[]},{bundle_id:''},{pid:123,name:null},{pid:321},{name:'missing'},{bundle_id:'missing'},{pid:123,bundle_id:'test.other'}];
    for(const tool of ['list_windows','get_app_state','resolve_element','screenshot']) {
      for(const app_ref of invalid) {
        const reply=await call(tool,{app_ref});
        assert.equal(reply.ok,false,`${tool} must reject ${JSON.stringify(app_ref)}`);
        assert.match(reply.error.message,/application not found/);
      }
    }
  } finally { delete process.env.CU_TARGETING_NATIVE; }
});
