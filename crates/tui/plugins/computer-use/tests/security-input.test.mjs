import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import { create as windows } from '../src/backends/win32.mjs';
import { create as harmony } from '../src/backends/harmonyos.mjs';
import { create as linux } from '../src/backends/linux.mjs';

test('Windows launch URLs are a single data argument, including PowerShell-looking text', async () => {
  const scripts = [];
  const b = windows({ exec: { run: async (_, args) => {
    scripts.push(Buffer.from(args[args.indexOf('-EncodedCommand') + 1], 'base64').toString('utf16le'));
    return {code:0, stdout:'', stderr:''};
  } } });
  for (const [url, quoted] of [
    ['https://example.test/$(Write-Output marker)`whoami`?q=中 😀', '"https://example.test/$(Write-Output marker)`whoami`?q=中 😀"'],
    ['https://example.test/"quoted"', '"https://example.test/\\"quoted\\""'],
    ['https://example.test/end\\', '"https://example.test/end\\\\"'],
  ]) {
    assert.equal((await b.open_application({name:'msedge',url})).launched,true);
    const script=scripts.at(-1);
    assert.ok(!script.includes(url));
    assert.doesNotMatch(script,/Write-Output marker|whoami/);
    const encoded=script.match(/FromBase64String\('([^']+)'\)/)?.[1];
    assert.ok(encoded, 'URL data must be carried separately from executable script');
    assert.equal(Buffer.from(encoded,'base64').toString('utf16le'),quoted);
    assert.match(script,/Start-Process -FilePath "msedge" -ArgumentList \$launchArg/);
  }
  const before=scripts.length;
  for(const url of [123,{},'', '-Command whoami','https://example.test/\nwhoami']) await assert.rejects(b.open_application({name:'msedge',url}), /absolute URL/);
  assert.equal(scripts.length,before);
});

test('Harmony launch rejects shell syntax before dispatch and quotes admitted identifiers', async () => {
  const calls=[];
  const b=harmony({exec:{shell:async args=>{calls.push(args);return {code:0,stdout:'',stderr:''};}}});
  for(const value of ['com.example;id','$(id)','com.example\nid','com.example app','"com.example"','-debug',123,{},'']) {
    await assert.rejects(b.open_application({bundle_id:value}),/bundle identifier/);
    await assert.rejects(b.open_application({bundle_id:'com.example',ability:value}),/ability identifier/);
  }
  assert.equal(calls.length,0);
  const result=await b.open_application({bundle_id:'com.example.app',ability:'MainAbility'});
  assert.equal(result.launched,true);
  assert.deepEqual(calls,[['aa','start','-b',"'com.example.app'",'-a',"'MainAbility'"]]);
});

test('Linux screenshot rejects option-shaped destinations before probing or capture', async t => {
  const dir=fs.mkdtempSync(path.join(os.tmpdir(),'cu-path-security-'));
  t.after(()=>fs.rmSync(dir,{recursive:true,force:true}));
  const saved={};
  for(const key of ['DISPLAY','WAYLAND_DISPLAY','XDG_SESSION_TYPE','CODEWHALE_CU_RECORDINGS_DIR']) saved[key]=process.env[key];
  t.after(()=>{for(const [key,value] of Object.entries(saved)) if(value===undefined)delete process.env[key];else process.env[key]=value;});
  process.env.DISPLAY=':fixture';delete process.env.WAYLAND_DISPLAY;
  process.env.XDG_SESSION_TYPE='x11';process.env.CODEWHALE_CU_RECORDINGS_DIR=dir;
  const calls=[];let probes=0;
  const b=linux({exec:{have:async name=>{probes++;return name==='scrot';},run:async (cmd,args)=>{calls.push({cmd,args});return {code:1,stdout:'',stderr:'fixture capture reached'};}}});
  for(const file of ['--exec=anything','relative.png','',123,{},'/tmp/a\0b']) await assert.rejects(b.screenshot({path:file}),/absolute filename/);
  assert.equal(probes,0);assert.equal(calls.length,0);
  const file=path.join(dir,'--exec=ordinary-filename.png');
  await assert.rejects(b.screenshot({path:file}),/fixture capture reached/);
  assert.deepEqual(calls,[{cmd:'scrot',args:['-z',file]}]);
});
