import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { deflateRawSync } from "node:zlib";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { newerVersion, releaseUpdate, validateReleaseZip, readUpdateResult } from "../app/updates.mjs";
import { replaceMacBundle } from "../app/install-macos.mjs";

const release = () => ({ tag_name:"v0.4.0",assets:[{name:"Codewhale-Computer-Use-0.4.0-macos-universal.zip",browser_download_url:"https://github.com/Hmbown/codewhale-cu-plugin/releases/download/v0.4.0/Codewhale-Computer-Use-0.4.0-macos-universal.zip",digest:`sha256:${"a".repeat(64)}`,size:1024}] });
test("updates only offer a newer stable installer with the exact release identity",()=>{
  assert.equal(newerVersion("0.10.0","0.9.13"),true);
  for(const value of ["0.9.13","0.8.0","0.10.0-beta","v0.10.0","nonsense"]) assert.equal(newerVersion(value,"0.9.13"),false);
  assert.equal(releaseUpdate(release(),"0.3.0").available,true);
  for(const change of [{draft:true},{prerelease:true},{tag_name:"v0.2.0"},{assets:[]}]) assert.equal(releaseUpdate({...release(),...change},"0.3.0").available,false);
  for(const change of [{digest:null},{size:Infinity},{size:512*1024*1024},{browser_download_url:"https://example.org/app.zip"},{name:"unexpected.zip"}]) {
    const data=release(); Object.assign(data.assets[0],change); assert.equal(releaseUpdate(data,"0.3.0").available,false);
  }
});
function zip(name,{kind=0x8000,localName=name,size=1,payload=Buffer.from("x"),method=0}={}) {
  const local=Buffer.alloc(30); local.writeUInt32LE(0x04034b50); local.writeUInt16LE(Buffer.byteLength(localName),26); local.writeUInt32LE(payload.length,18); local.writeUInt32LE(size,22); local.writeUInt16LE(method,8);
  const contents=Buffer.concat([local,Buffer.from(localName),payload]);
  const central=Buffer.alloc(46); central.writeUInt32LE(0x02014b50); central.writeUInt16LE(Buffer.byteLength(name),28); central.writeUInt32LE((kind*65536)>>>0,38); central.writeUInt32LE(payload.length,20); central.writeUInt32LE(size,24); central.writeUInt16LE(method,10);
  const index=Buffer.concat([central,Buffer.from(name)]),end=Buffer.alloc(22);end.writeUInt32LE(0x06054b50);end.writeUInt16LE(1,8);end.writeUInt16LE(1,10);end.writeUInt32LE(index.length,12);end.writeUInt32LE(contents.length,16);
  return Buffer.concat([contents,index,end]);
}
test("the updater refuses traversal, links, bombs and inconsistent ZIP headers before extraction",()=>{
  const name="Codewhale Computer Use.app/Contents/MacOS/node";
  assert.equal(validateReleaseZip(zip(name)),1);
  for(const unsafe of ["/tmp/escape","Codewhale Computer Use.app/../escape","Codewhale Computer Use.app/a/../../escape","Codewhale Computer Use.app/Contents/evil\\path","Codewhale Computer Use.app/Contents/a:b"]) assert.throws(()=>validateReleaseZip(zip(unsafe)));
  for(const options of [{kind:0xa000},{localName:"../escape"},{size:1024*1024*1024}]) assert.throws(()=>validateReleaseZip(zip(name,options)));
  assert.throws(()=>validateReleaseZip(Buffer.from("not a ZIP")));
  const payload=deflateRawSync(Buffer.alloc(1024*1024));
  assert.throws(()=>validateReleaseZip(zip(name,{method:8,payload,size:1})),/oversized/);
  assert.equal(validateReleaseZip(zip(name,{method:8,payload,size:1024*1024})),1);
});
test("failed update preparation or verification preserves the complete previous app",t=>{
  const directory=fs.mkdtempSync(path.join(os.tmpdir(),"cu-atomic-install-"));t.after(()=>fs.rmSync(directory,{recursive:true,force:true}));
  const source=path.join(directory,"source.app"),destination=path.join(directory,"installed.app"),relative="Contents/Resources/plugin/app/daemon.mjs";
  for(const [root,value] of [[source,"new"],[destination,"old"]]) {fs.mkdirSync(path.dirname(path.join(root,relative)),{recursive:true});fs.writeFileSync(path.join(root,relative),value);}
  assert.throws(()=>replaceMacBundle(source,destination,{verify:()=>{throw new Error("bad signature");}}),/bad signature/);
  assert.equal(fs.readFileSync(path.join(destination,relative),"utf8"),"old");
  assert.throws(()=>replaceMacBundle(source,destination,{prepare:()=>{throw new Error("disk error");},verify:()=>{}}),/disk error/);
  assert.equal(fs.readFileSync(path.join(destination,relative),"utf8"),"old");
  const result=replaceMacBundle(source,destination,{verify:()=>{}});
  assert.equal(fs.readFileSync(path.join(destination,relative),"utf8"),"new");
  assert.equal(fs.readFileSync(path.join(result.backup,relative),"utf8"),"old");
});

test("a rejected apply leaves a readable result for the next launch without changing control consent",t=>{
  const directory=fs.mkdtempSync(path.join(os.tmpdir(),"cu-update-result-"));
  const previous=process.env.CODEWHALE_CU_STATE_DIR;
  process.env.CODEWHALE_CU_STATE_DIR=directory;
  t.after(()=>{
    if(previous===undefined) delete process.env.CODEWHALE_CU_STATE_DIR; else process.env.CODEWHALE_CU_STATE_DIR=previous;
    fs.rmSync(directory,{recursive:true,force:true});
  });
  const controls=path.join(directory,"control.json");
  fs.writeFileSync(controls,JSON.stringify({mode:"stopped"}));
  assert.equal(readUpdateResult(),null);
  // Verification of a nonexistent bundle fails before either PID is used;
  // a nonexistent destination also prevents opening any real application.
  const result=spawnSync(process.execPath,[fileURLToPath(new URL("../app/updates.mjs",import.meta.url)),"--apply",path.join(directory,"missing-source.app"),path.join(directory,"missing-destination.app"),String(process.pid),String(process.pid)],{encoding:"utf8",env:process.env,timeout:10_000});
  assert.equal(result.status,1,result.stderr);
  const status=readUpdateResult();
  assert.equal(status.available,false);
  assert.match(status.message,/update could not be completed/i);
  assert.match(status.message,/sessions remain stopped/i);
  assert.deepEqual(JSON.parse(fs.readFileSync(controls)),{mode:"stopped"});
  const resultFile=path.join(directory,"update-result.json");
  assert.equal(JSON.parse(fs.readFileSync(resultFile)).ok,false);
  if(process.platform!=="win32") assert.equal(fs.statSync(resultFile).mode&0o777,0o600);
  for(const invalid of ["not JSON",JSON.stringify({ok:true,message:123}),JSON.stringify({ok:true,message:"x".repeat(5000)})]) {
    fs.writeFileSync(resultFile,invalid);
    assert.equal(readUpdateResult(),null);
  }
});
