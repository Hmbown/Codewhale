import {test,after} from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {handle,closeAllSessions} from '../src/app-handler.mjs';
import {create as linux} from '../src/backends/linux.mjs';
import {calls} from './fixtures/selector-backend.mjs';

const fixtures=fileURLToPath(new URL('./fixtures/',import.meta.url));
const saved={};
for(const key of ['CODEWHALE_CU_TEST_BACKEND','CU_SELECTOR_PLATFORM','DISPLAY','WAYLAND_DISPLAY','XDG_SESSION_TYPE']) saved[key]=process.env[key];
process.env.CODEWHALE_CU_TEST_BACKEND=path.join(fixtures,'selector-backend.mjs');
after(async()=>{
  await closeAllSessions();
  for(const [key,value] of Object.entries(saved)) if(value===undefined)delete process.env[key];else process.env[key]=value;
});
const invalidRefs=[null,{},[],false,0,'Fixture',{pid:123},{bundle_id:'test.fixture'},{name:''},{name:'  '},{name:123},{name:'Fixture',pid:123},{unknown:'Fixture'}];
for(const platform of ['linux','harmonyos']) {
  test(`${platform}: real app-handler refuses unsupported selectors before any observation or input runner`,async()=>{
    process.env.CU_SELECTOR_PLATFORM=platform;
    const call=(tool,args)=>handle({tool,args},{sessionId:`selectors-${platform}`});
    async function rejected(tool,args) {
      const reply=await call(tool,args);
      assert.equal(reply.ok,false,`${platform}/${tool} should refuse ${JSON.stringify(args)}`);
      assert.equal(reply.error.code,'unsupported_selector',JSON.stringify(reply));
      assert.equal(calls.length,0,'refusal must precede even capability probes');
    }
    for(const tool of ['list_windows','screenshot']) {
      for(const app_ref of [...invalidRefs,{name:'Fixture'}]) await rejected(tool,{app_ref});
      for(const window_id of [null,0,1,'0']) await rejected(tool,{window_id});
    }
    for(const app_ref of platform==='harmonyos'?[...invalidRefs,{name:'Fixture'}]:invalidRefs) {
      await rejected('get_app_state',{app_ref});
      for(const tool of ['set_value','perform_action']) await rejected(tool,{target:{app_ref,path:[],index:0},value:'synthetic',action:'click'});
      if(platform==='linux') await rejected('resolve_element',{app_ref,path:[]});
    }
    for(const window_id of [null,0,1,'0']) await rejected('get_app_state',{window_id});
    for(const selector of [{windowIndex:1},{window_id:0},{window_id:null}]) {
      for(const tool of ['set_value','perform_action']) await rejected(tool,{target:{...selector,path:[],index:0},value:'synthetic',action:'click'});
      if(platform==='linux') await rejected('resolve_element',{...selector,path:[]});
    }
  });
}

test('Linux production Python uses the same exact unique app-name selection for observation, resolution and semantic actions',{skip:spawnSync('python3',['--version']).status!==0},async t=>{
  const dir=fs.mkdtempSync(path.join(os.tmpdir(),'cu-atspi-selector-'));
  t.after(()=>fs.rmSync(dir,{recursive:true,force:true}));
  const actions=path.join(dir,'actions.jsonl');
  process.env.DISPLAY='fixture';delete process.env.WAYLAND_DISPLAY;process.env.XDG_SESSION_TYPE='x11';
  let duplicate=false, editMode="text";
  const backend=linux({exec:{have:async()=>true,run:async(cmd,args)=>{
    assert.equal(cmd,'python3','only generated Python may run in this fixture');
    assert.equal(args[0],'-c');
    const prefix=`import importlib.util, sys\nspec = importlib.util.spec_from_file_location("pyatspi", ${JSON.stringify(path.join(fixtures,'pyatspi.py'))})\nmodule = importlib.util.module_from_spec(spec)\nsys.modules["pyatspi"] = module\nspec.loader.exec_module(module)\n`;
    const result=spawnSync('python3',['-I','-S','-B','-c',prefix+args[1],...args.slice(2)],{encoding:'utf8',env:{...process.env,CU_ATSPI_ACTIONS:actions,CU_ATSPI_DUPLICATE:duplicate?'1':'0',CU_ATSPI_EDIT_MODE:editMode}});
    assert.equal(result.status,0,result.stderr);
    return {code:result.status,stdout:result.stdout,stderr:result.stderr};
  }}});
  const explicit={name:'fixture'};
  const state=await backend.get_app_state({app_ref:explicit});
  assert.equal(state.name,'Fixture');assert.equal(state.elements[0].label,'Fixture');
  assert.equal((await backend.resolve_element({app_ref:explicit,path:[]})).element.label,'Fixture');
  await backend.perform_action({target:{app_ref:explicit,path:[],windowIndex:0},action:'click'});
  assert.deepEqual(fs.readFileSync(actions,'utf8').trim().split('\n').map(JSON.parse),[{name:'Fixture',action:'click'}]);
  assert.equal((await backend.get_app_state({})).name,'Fixture Extended','omission preserves existing default');
  for(const name of ['Fixt','missing','Fixture*']) {
    await assert.rejects(backend.get_app_state({app_ref:{name}}),/application not found/);
    assert.equal((await backend.resolve_element({app_ref:{name},path:[]})).found,false);
    await assert.rejects(backend.perform_action({target:{app_ref:{name},path:[]},action:'click'}),/app_not_found/);
  }
  duplicate=true;
  await assert.rejects(backend.get_app_state({app_ref:explicit}),/application not found/);
  assert.equal((await backend.resolve_element({app_ref:explicit,path:[]})).found,false);
  await assert.rejects(backend.perform_action({target:{app_ref:explicit,path:[]},action:'click'}),/app_not_found/);
  assert.equal(fs.readFileSync(actions,'utf8').trim().split('\n').length,1,'missing or ambiguous app names must never send a synthetic action');
  duplicate=false;
  const target={app_ref:explicit,path:[0],windowIndex:0};
  const edits=()=>fs.readFileSync(actions,'utf8').trim().split('\n').map(JSON.parse).filter(x=>'value' in x);
  const text='Updated 漢字 🌊';
  assert.deepEqual(await backend.set_value({target,value:text}),{action_sent:true,strategy:'a11y',verified:true,after:text});
  editMode='numeric';
  assert.equal((await backend.set_value({target,value:12.5})).after,12.5);
  assert.deepEqual(edits().map(x=>x.value),[text,12.5]);
  for(const [mode,error,sends] of [['readonly','element_read_only',0],['disabled','element_disabled',0],['query-failed','interface_failed',0],['rejected','value_rejected',1],['mismatch','value_verification_failed',1]]) {
    editMode=mode;
    const before=edits().length;
    await assert.rejects(backend.set_value({target,value:'rejected'}),new RegExp(error));
    assert.equal(edits().length-before,sends,`${mode}: never replay a failed or unverified edit`);
  }

});
