import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import crypto from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { inflateRawSync } from "node:zlib";
import { replaceMacBundle, verifyReleaseBundle } from "./install-macos.mjs";
import { APP_VERSION, APP_NAME } from "../src/app-socket.mjs";

const repository="https://github.com/Hmbown/codewhale-cu-plugin";
const limit=256*1024*1024;
async function responseBytes(response, maximum) {
  const chunks=[]; let size=0;
  for await(const chunk of response.body) { size+=chunk.length; if(size>maximum) throw new Error("The update service exceeded its response size limit."); chunks.push(chunk); }
  return Buffer.concat(chunks);
}
export function newerVersion(candidate,current) {
  const parse=value=>/^\d+\.\d+\.\d+$/.test(value)?value.split(".").map(Number):null;
  const a=parse(candidate),b=parse(current); if(!a||!b) return false;
  for(let i=0;i<3;i++) { if(a[i]!==b[i]) return a[i]>b[i]; } return false;
}
export function releaseUpdate(release,current=APP_VERSION) {
  const version=release?.tag_name?.replace(/^v/,"");
  if(!version||release.draft||release.prerelease||!newerVersion(version,current)) return {available:false,message:`You have Computer Use ${current}. No newer stable installer is available.`};
  const name=`Codewhale-Computer-Use-${version}-macos-universal.zip`;
  const asset=release.assets?.find(asset=>asset.name===name);
  const url=`${repository}/releases/download/v${version}/${name}`;
  if(!asset||asset.browser_download_url!==url||!/^sha256:[a-f0-9]{64}$/.test(asset.digest)||!Number.isSafeInteger(asset.size)||asset.size<=0||asset.size>limit) return {available:false,message:`Version ${version} has no verified macOS installer yet.`};
  return {available:true,version,url,sha256:asset.digest.slice(7),size:asset.size,message:`Computer Use ${version} is available. Install it to restart the helper; existing computer sessions will stop.`};
}
export async function checkForUpdate() {
  const response=await fetch("https://api.github.com/repos/Hmbown/codewhale-cu-plugin/releases/latest",{redirect:"error",headers:{Accept:"application/vnd.github+json","X-GitHub-Api-Version":"2022-11-28"},signal:AbortSignal.timeout(10_000)});
  if(response.status===404) return {available:false,message:"No stable installer has been published yet. Your current app is unchanged."};
  if(!response.ok) throw new Error(`The update service is unavailable (${response.status}). Try again later.`);
  return releaseUpdate(JSON.parse((await responseBytes(response,1024*1024)).toString("utf8")));
}

/** Inspect both ZIP headers before extraction: no links, traversal or bombs. */
export function validateReleaseZip(bytes) {
  const minimum=Math.max(0,bytes.length-65557); let end=-1;
  for(let i=bytes.length-22;i>=minimum;i--) if(bytes.readUInt32LE(i)===0x06054b50&&i+22+bytes.readUInt16LE(i+20)===bytes.length) { end=i; break; }
  if(end<0||bytes.readUInt16LE(end+4)||bytes.readUInt16LE(end+6)) throw new Error("Invalid update archive.");
  const count=bytes.readUInt16LE(end+10); let position=bytes.readUInt32LE(end+16),total=0;
  if(!count||count>2000||bytes.readUInt16LE(end+8)!==count||position+bytes.readUInt32LE(end+12)!==end) throw new Error("Invalid update archive index.");
  const seen=new Set();
  for(let i=0;i<count;i++) {
    if(position+46>end||bytes.readUInt32LE(position)!==0x02014b50) throw new Error("Invalid update entry.");
    const flags=bytes.readUInt16LE(position+8),method=bytes.readUInt16LE(position+10),length=bytes.readUInt16LE(position+28),extra=bytes.readUInt16LE(position+30),comment=bytes.readUInt16LE(position+32);
    const name=bytes.subarray(position+46,position+46+length).toString("utf8");
    const kind=(bytes.readUInt32LE(position+38)>>>16)&0xf000,offset=bytes.readUInt32LE(position+42),compressed=bytes.readUInt32LE(position+20);
    const size=bytes.readUInt32LE(position+24); total+=size;
    if(flags&1||![0,8].includes(method)||![0,0x4000,0x8000].includes(kind)||total>512*1024*1024||position+46+length+extra+comment>end) throw new Error("Unsupported update entry.");
    if(!name.startsWith(`${APP_NAME}.app/`)||name.includes("\\")||name.includes(":")||name.includes("\0")||name.split("/").some(part=>part===".."||part===".")||seen.has(name)) throw new Error("Unsafe update path.");
    seen.add(name);
    if(offset+30>position||bytes.readUInt32LE(offset)!==0x04034b50) throw new Error("Invalid update file header.");
    const localLength=bytes.readUInt16LE(offset+26),localExtra=bytes.readUInt16LE(offset+28);
    if(offset+30+localLength+localExtra+compressed>bytes.readUInt32LE(end+16)||bytes.subarray(offset+30,offset+30+localLength).toString("utf8")!==name) throw new Error("Inconsistent update file header.");
    if(bytes.readUInt16LE(offset+8)!==method||bytes.readUInt16LE(offset+6)!==flags||(!(flags&8)&&(bytes.readUInt32LE(offset+18)!==compressed||bytes.readUInt32LE(offset+22)!==size))) throw new Error("Inconsistent update sizes or compression.");
    const start=offset+30+localLength+localExtra;
    // Header sizes are untrusted. Bound actual expansion before ditto writes
    // anything, including a compressed payload whose headers understate size.
    const payload=bytes.subarray(start,start+compressed);
    let expanded;
    try { expanded=method===0?payload.length:inflateRawSync(payload,{maxOutputLength:Math.max(size,1)}).length; }
    catch { throw new Error("Invalid or oversized compressed update entry."); }
    if(expanded!==size) throw new Error("The update entry size did not match its contents.");
    position+=46+length+extra+comment;
  }
  if(position!==end) throw new Error("Invalid update archive length.");
  return count;
}

export async function prepareUpdate(update) {
  if(!update?.available) throw new Error("Check for an available update first.");
  if(!newerVersion(update.version,APP_VERSION)||update.url!==`${repository}/releases/download/v${update.version}/Codewhale-Computer-Use-${update.version}-macos-universal.zip`||!Number.isSafeInteger(update.size)||update.size<=0||update.size>limit) throw new Error("The update identity is invalid.");
  // Only GitHub's fixed release URL and its asset CDN can serve the bytes.
  let url=update.url, response;
  for(let redirects=0;redirects<4;redirects++) {
    response=await fetch(url,{redirect:"manual",signal:AbortSignal.timeout(60_000)});
    if(![301,302,303,307,308].includes(response.status)) break;
    const next=new URL(response.headers.get("location"),url);
    if(next.protocol!=="https:"||!["github.com","release-assets.githubusercontent.com","objects.githubusercontent.com"].includes(next.hostname)) throw new Error("The update download redirected to an unexpected host.");
    url=next.href;
  }
  if(!response?.ok) throw new Error("The update could not be downloaded. Your current app is unchanged.");
  const bytes=await responseBytes(response,update.size);
  if(bytes.length!==update.size||crypto.createHash("sha256").update(bytes).digest("hex")!==update.sha256) throw new Error("The update checksum did not match. Your current app is unchanged.");
  validateReleaseZip(bytes);
  const stage=fs.mkdtempSync(path.join(os.tmpdir(),"codewhale-cu-release-"));
  try {
    const archive=path.join(stage,"release.zip"); fs.writeFileSync(archive,bytes,{mode:0o600});
    const result=spawnSync("ditto",["-x","-k",archive,stage],{encoding:"utf8"});
    if(result.status!==0) throw new Error("The update could not be unpacked.");
    const bundle=path.join(stage,`${APP_NAME}.app`); verifyReleaseBundle(bundle);
    const version=spawnSync("/usr/libexec/PlistBuddy",["-c","Print :CFBundleShortVersionString",path.join(bundle,"Contents","Info.plist")],{encoding:"utf8"});
    if(version.status!==0||version.stdout.trim()!==update.version) throw new Error("The downloaded app has a different version.");
    return {stage,bundle};
  } catch(error) { fs.rmSync(stage,{recursive:true,force:true}); throw error; }
}

export async function restartWithUpdate(prepared,destination) {
  const logDir=path.join(os.homedir(),"Library","Logs",APP_NAME); fs.mkdirSync(logDir,{recursive:true});
  const log=fs.openSync(path.join(logDir,"update.log"),"a",0o600);
  const child=spawn(process.execPath,[fileURLToPath(import.meta.url),"--apply",prepared.bundle,destination,String(process.pid),String(process.ppid)],{detached:true,stdio:["ignore",log,log]});
  try { await new Promise((resolve,reject)=>{child.once("spawn",resolve);child.once("error",reject);}); child.unref(); }
  finally { fs.closeSync(log); }
}

if(process.argv[1]===fileURLToPath(import.meta.url)&&process.argv[2]==="--apply") {
  const [source,destination,owner,launcher]=process.argv.slice(3);
  try {
    verifyReleaseBundle(source);
    process.kill(Number(owner),"SIGTERM");
    for(let i=0;i<100;i++) { try { process.kill(Number(owner),0); } catch { break; } await new Promise(resolve=>setTimeout(resolve,100)); }
    try { process.kill(Number(owner),0); throw new Error("The running helper did not stop; update cancelled."); } catch(error) { if(error.code!=="ESRCH") throw error; }
    // LaunchServices must not send Reopen to the retiring menu-bar process.
    for(let i=0;i<100;i++) { try { process.kill(Number(launcher),0); } catch { break; } await new Promise(resolve=>setTimeout(resolve,100)); }
    try { process.kill(Number(launcher),0); throw new Error("The menu-bar app did not exit; update cancelled."); } catch(error) { if(error.code!=="ESRCH") throw error; }
    const receipt=replaceMacBundle(source,destination,{verify:verifyReleaseBundle});
    const installed=spawnSync("/usr/libexec/PlistBuddy",["-c","Print :CFBundleShortVersionString",path.join(destination,"Contents","Info.plist")],{encoding:"utf8"});
    console.log(JSON.stringify({version:installed.stdout.trim(),...receipt,installedAt:new Date().toISOString()}));
  } catch(error) { console.error(error.message); process.exitCode=1; }
  finally { spawnSync("open",["-g","-a",destination]); }
}
