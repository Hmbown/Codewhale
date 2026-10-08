/** Boundary evidence v1. Pure, deterministic, metadata-only; no network or trust-by-text. */
import { type Category, type Trace, type WhaleEvent, type Finding, stableHash } from './model.js';
export const SURFACES = ['model-api','http','mcp','git','filesystem','process','sandbox','dns','socket','message','database','artifact','browser','clipboard','other'] as const;
export type Surface = typeof SURFACES[number];
export const STAGES = ['intent','dispatch','runtime','effect','control'] as const;
export type Stage = typeof STAGES[number];
export const EFFECTS = ['read','write','delete','execute','publish','send','receive','connect','grant','revoke','none','unknown'] as const;
export type Effect = typeof EFFECTS[number];
export interface Observation {
  version: 1; id: string; runId: string; operationId?: string;
  sourceId: string; epoch: string; sequence: number;
  time: { wallMs: number; clockId?: string; monotonicMs?: number; taskMs?: number; uncertaintyMs?: number };
  receivedAt?: number;
  subject: { agentId: string; sandboxId?: string; isolationGroup?: string };
  stage: Stage; surface: Surface; action: string; effect: Effect;
  status: 'started' | 'completed' | 'error' | 'unknown';
  target?: { id: string; kind: string; version?: string; boundary?: 'local'|'external'|'unknown' };
  actionDigest?: string;
  authority?: { grantId?: string; claimed?: boolean };
  correlation?: { sessionId?: string; requestId?: string; parentOperationId?: string; messageId?: string };
  facts: Record<string, string | number | boolean | null>;
}
export interface EvidenceSource {
  id: string; stages: Stage[]; surfaces: Surface[];
  runIds: string[]; sandboxIds?: string[]; isolationGroups?: string[];
  heartbeatMs: number; description?: string;
}
export interface Grant {
  id: string; runIds: string[]; sandboxIds: string[]; targetIds: string[];
  actions: string[]; effects: Effect[]; notBefore: number; expiresAt: number;
  actionDigest?: string;
}
export interface BoundaryPolicy {
  version: 1; id: string; sources: EvidenceSource[]; grants: Grant[];
  expectedSurfaces: Surface[]; forbiddenCrossGroup: boolean;
}
export interface EvidenceBundle {
  format: 'whalesong.evidence/v1'; name: string; records: Observation[];
  policy: BoundaryPolicy; asOf: number;
}
export interface CoverageRow {
  sourceId: string; surface: Surface; status: 'observed'|'stale'|'missing'|'unconfigured'|'disabled'|'unverified-time';
  count: number; lastReceivedAt: number | null; sequenceGaps: number;
}
export interface BoundaryFinding {
  id: string; code: string; severity: 'info'|'warning'|'critical';
  title: string; detail: string; recordIds: string[]; time: number;
}
export interface Interaction {
  runId: string; sandboxId?: string; group?: string; target: string;
  kind: string; stage: Stage; effect: Effect; count: number; recordIds: string[];
}
export interface BoundaryAnalysis {
  version: 1; omittedFindings: number; asOf: number; inputRecords: number; visibleRecords: number; uniqueRecords: number;
  futureRecords: number; findings: BoundaryFinding[]; coverage: CoverageRow[];
  interactions: Interaction[]; runs: number; sandboxes: number; externalEffects: number;
  unknownEffects: number; authority: { permitted: number; denied: number; unknown: number };
  provenance: string;
}
const enumValue = <T extends string>(v:unknown, values: readonly T[], field:string): T => {
  if(typeof v!=='string'||!values.includes(v as T))throw new Error(`Invalid ${field}.`);return v as T;
};
const object = (v:unknown, field:string): Record<string,unknown> => {
  if(!v||typeof v!=='object'||Array.isArray(v))throw new Error(`Invalid ${field}: object required.`);return v as Record<string,unknown>;
};
const text = (v:unknown, field:string, max=256):string => {
  if(typeof v!=='string'||!v.length||v.length>max||/[\u0000-\u001f\u007f]/.test(v))throw new Error(`Invalid ${field}: nonempty bounded text required.`);return v;
};
const num = (v:unknown,field:string,min=0):number => {
  if(typeof v!=='number'||!Number.isFinite(v)||v<min||Math.abs(v)>Number.MAX_SAFE_INTEGER)throw new Error(`Invalid ${field}.`);return v;
};
const optionalText = (o:Record<string,unknown>,key:string):string|undefined => o[key]===undefined?undefined:text(o[key],key);
const strings = (v:unknown,field:string,allowEmpty=false):string[]=>{
  if(!Array.isArray(v)||(!allowEmpty&&!v.length)||v.length>256)throw new Error(`Invalid ${field}.`);
  const out=v.map(x=>text(x,field));if(new Set(out).size!==out.length)throw new Error(`Duplicate ${field}.`);return out;
};
const boolean = (v:unknown,field:string):boolean=>{if(typeof v!=='boolean')throw new Error(`Invalid ${field}.`);return v;};
const optionalNumber=(o:Record<string,unknown>,key:string)=>o[key]===undefined?undefined:num(o[key],key);
function onlyKeys(o:Record<string,unknown>,keys:string[],label:string):void{
  for(const k of Object.keys(o))if(!keys.includes(k))throw new Error(`Unknown ${label} field: ${k}.`);
}
/** All unrecognized fields are rejected, never secretly retained in metadata-only evidence. */
export function validateObservation(input:unknown):Observation {
  const o=object(input,'observation');
  onlyKeys(o,['version','id','runId','operationId','sourceId','epoch','sequence','time','receivedAt','subject','stage','surface','action','effect','status','target','actionDigest','authority','correlation','facts'],'observation');
  if(o.version!==1)throw new Error('Unsupported observation version.');
  const t=object(o.time,'time'),s=object(o.subject,'subject');
  onlyKeys(t,['wallMs','clockId','monotonicMs','taskMs','uncertaintyMs'],'time');onlyKeys(s,['agentId','sandboxId','isolationGroup'],'subject');
  const seq=num(o.sequence,'sequence',1);if(!Number.isSafeInteger(seq))throw new Error('sequence must be an integer.');
  const facts=object(o.facts??{},'facts'),safeFacts:Observation['facts']={};
  if(Object.keys(facts).length>48)throw new Error('Too many observation facts.');
  for(const [k,v] of Object.entries(facts)){
    text(k,'fact key',80);if(['__proto__','prototype','constructor'].includes(k))throw new Error('Unsafe fact key.');
    if(typeof v==='string')safeFacts[k]=text(v,'fact value',512);
    else if(typeof v==='number')safeFacts[k]=num(v,'fact value',-Number.MAX_SAFE_INTEGER);
    else if(v===null||typeof v==='boolean')safeFacts[k]=v;
    else throw new Error('Facts must be scalar metadata, not content objects.');
  }
  let target:Observation['target'];if(o.target!==undefined){const a=object(o.target,'target');onlyKeys(a,['id','kind','version','boundary'],'target');target={id:text(a.id,'target.id'),kind:text(a.kind,'target.kind',64),version:optionalText(a,'version'),boundary:a.boundary===undefined?undefined:enumValue(a.boundary,['local','external','unknown'] as const,'boundary')};}
  let authority:Observation['authority'];if(o.authority!==undefined){const a=object(o.authority,'authority');onlyKeys(a,['grantId','claimed'],'authority');authority={grantId:optionalText(a,'grantId'),claimed:a.claimed===undefined?undefined:boolean(a.claimed,'claimed')};}
  let correlation:Observation['correlation'];if(o.correlation!==undefined){const a=object(o.correlation,'correlation');onlyKeys(a,['sessionId','requestId','parentOperationId','messageId'],'correlation');correlation={sessionId:optionalText(a,'sessionId'),requestId:optionalText(a,'requestId'),parentOperationId:optionalText(a,'parentOperationId'),messageId:optionalText(a,'messageId')};}
  return {version:1,id:text(o.id,'id'),runId:text(o.runId,'runId'),operationId:optionalText(o,'operationId'),sourceId:text(o.sourceId,'sourceId'),epoch:text(o.epoch,'epoch'),sequence:seq,
    time:{wallMs:num(t.wallMs,'wallMs'),clockId:optionalText(t,'clockId'),monotonicMs:optionalNumber(t,'monotonicMs'),taskMs:optionalNumber(t,'taskMs'),uncertaintyMs:optionalNumber(t,'uncertaintyMs')},receivedAt:optionalNumber(o,'receivedAt'),
    subject:{agentId:text(s.agentId,'agentId'),sandboxId:optionalText(s,'sandboxId'),isolationGroup:optionalText(s,'isolationGroup')},stage:enumValue(o.stage,STAGES,'stage'),surface:enumValue(o.surface,SURFACES,'surface'),action:text(o.action,'action',128),effect:enumValue(o.effect,EFFECTS,'effect'),status:enumValue(o.status,['started','completed','error','unknown'],'status'),target,actionDigest:optionalText(o,'actionDigest'),authority,correlation,facts:safeFacts};
}
export function validatePolicy(input:unknown):BoundaryPolicy {
  const o=object(input,'policy');onlyKeys(o,['version','id','sources','grants','expectedSurfaces','forbiddenCrossGroup'],'policy');
  if(o.version!==1||!Array.isArray(o.sources)||o.sources.length>256||!Array.isArray(o.grants)||o.grants.length>2048)throw new Error('Invalid policy version or limits.');
  const sources=o.sources.map(x=>{const a=object(x,'source');onlyKeys(a,['id','stages','surfaces','runIds','sandboxIds','isolationGroups','heartbeatMs','description'],'source');return {id:text(a.id,'source.id'),stages:strings(a.stages,'stages').map(v=>enumValue(v,STAGES,'stage')),surfaces:strings(a.surfaces,'surfaces').map(v=>enumValue(v,SURFACES,'surface')),runIds:strings(a.runIds,'runIds'),sandboxIds:a.sandboxIds===undefined?undefined:strings(a.sandboxIds,'sandboxIds'),isolationGroups:a.isolationGroups===undefined?undefined:strings(a.isolationGroups,'isolationGroups'),heartbeatMs:num(a.heartbeatMs,'heartbeatMs',1),description:optionalText(a,'description')};});
  const grants=o.grants.map(x=>{const a=object(x,'grant');onlyKeys(a,['id','runIds','sandboxIds','targetIds','actions','effects','notBefore','expiresAt','actionDigest'],'grant');const g={id:text(a.id,'grant.id'),runIds:strings(a.runIds,'runIds'),sandboxIds:strings(a.sandboxIds,'sandboxIds'),targetIds:strings(a.targetIds,'targetIds'),actions:strings(a.actions,'actions'),effects:strings(a.effects,'effects').map(v=>enumValue(v,EFFECTS,'effect')),notBefore:num(a.notBefore,'notBefore'),expiresAt:num(a.expiresAt,'expiresAt'),actionDigest:optionalText(a,'actionDigest')};if(g.expiresAt<=g.notBefore)throw new Error('Grant expiry must follow its start.');return g;});
  if(new Set(sources.map(s=>s.id)).size!==sources.length||new Set(grants.map(g=>g.id)).size!==grants.length)throw new Error('Duplicate policy identity.');
  return {version:1,id:text(o.id,'policy.id'),sources,grants,expectedSurfaces:strings(o.expectedSurfaces,'expectedSurfaces',true).map(v=>enumValue(v,SURFACES,'surface')),forbiddenCrossGroup:boolean(o.forbiddenCrossGroup,'forbiddenCrossGroup')};
}
export function validateBundle(input:unknown,maxRecords=100_000):EvidenceBundle {
  const o=object(input,'bundle');onlyKeys(o,['format','name','records','policy','asOf'],'bundle');
  if(o.format!=='whalesong.evidence/v1'||!Array.isArray(o.records)||o.records.length>maxRecords)throw new Error('Invalid evidence bundle or record limit exceeded.');
  const records=o.records.map(validateObservation),seen=new Set<string>();
  for(const record of records){const key=observationKey(record);if(seen.has(key))throw new Error('Duplicate producer incarnation/sequence in evidence bundle. Import cancelled; resolve identity before import.');seen.add(key);}
  return {format:o.format,name:text(o.name,'name'),records,policy:validatePolicy(o.policy),asOf:num(o.asOf,'asOf')};
}
export function sourceAccepts(source:EvidenceSource,o:Observation):boolean {
  return source.id===o.sourceId&&source.stages.includes(o.stage)&&source.surfaces.includes(o.surface)&&source.runIds.includes(o.runId)&&(!source.sandboxIds||source.sandboxIds.includes(o.subject.sandboxId??''))&&(!source.isolationGroups||source.isolationGroups.includes(o.subject.isolationGroup??''));
}
export type Authorization = { decision:'permitted'|'denied'|'unknown'; reason:string };
/** Exact allowlists; no prefix matching, text approval, or ambient default allow. Not enforcement. */
export function authorize(o:Observation,policy:BoundaryPolicy):Authorization {
  const source=policy.sources.find(s=>s.id===o.sourceId);
  if(!source||!sourceAccepts(source,o))return {decision:'unknown',reason:'Source is not bound to this scope.'};
  if(o.effect==='unknown')return {decision:'unknown',reason:'Actual effect not established.'};
  const grant=policy.grants.find(g=>g.id===o.authority?.grantId);
  if(!grant)return {decision:'denied',reason:'No matching operator-configured grant.'};
  const u=o.time.uncertaintyMs;
  if(u===undefined)return {decision:'unknown',reason:'Clock uncertainty absent; grant validity cannot be established.'};
  if(o.time.wallMs-u<grant.notBefore||o.time.wallMs+u>=grant.expiresAt)return {decision:'denied',reason:'Outside grant validity interval (including clock uncertainty).'};
  if(!grant.runIds.includes(o.runId)||!grant.sandboxIds.includes(o.subject.sandboxId??'')||!grant.targetIds.includes(o.target?.id??'')||!grant.actions.includes(o.action)||!grant.effects.includes(o.effect))return {decision:'denied',reason:'Action, effect, target, run, or sandbox is outside the grant.'};
  if(grant.actionDigest&&o.actionDigest!==grant.actionDigest)return {decision:'denied',reason:'Approved action digest does not match.'};
  return {decision:'permitted',reason:'Matches the supplied policy; source claims still require independent verification.'};
}
export const observationKey=(o:Observation):string=>JSON.stringify([o.sourceId,o.epoch,o.sequence]);
export const observationEventId=(o:Observation):string=>`obs:${encodeURIComponent(o.sourceId)}:${encodeURIComponent(o.epoch)}:${o.sequence}`;
export function canonicalJSON(v:unknown):string {
  if(v===undefined)return 'null';if(v===null||typeof v!=='object')return JSON.stringify(v);
  if(Array.isArray(v))return '['+v.map(canonicalJSON).join(',')+']';
  return '{'+Object.entries(v).filter(([,x])=>x!==undefined).sort(([a],[b])=>a<b?-1:a>b?1:0).map(([k,x])=>JSON.stringify(k)+':'+canonicalJSON(x)).join(',')+'}';
}
const writeEffects=new Set<Effect>(['write','delete','publish','send','grant','revoke']);
const consequential=(o:Observation)=>o.stage==='effect'&&o.status==='completed'&&writeEffects.has(o.effect);
function definitelyAfter(a:Observation,b:Observation):boolean {
  if(a.sourceId===b.sourceId&&a.epoch===b.epoch&&a.time.clockId&&a.time.clockId===b.time.clockId&&a.time.monotonicMs!==undefined&&b.time.monotonicMs!==undefined)return a.time.monotonicMs>b.time.monotonicMs;
  return a.time.uncertaintyMs!==undefined&&b.time.uncertaintyMs!==undefined&&a.time.wallMs-a.time.uncertaintyMs>b.time.wallMs+b.time.uncertaintyMs;
}
/** asOf is receipt time for operator replay. Missing receipt stamps are excluded in replay. */
export function analyzeEvidence(bundle:EvidenceBundle,asOf=bundle.asOf,operatorReplay=false):BoundaryAnalysis {
  let omittedFindings=0;
  const policy=bundle.policy,findings:BoundaryFinding[]=[],rows:CoverageRow[]=[],unique=new Map<string,Observation>();
  const visible=bundle.records.filter(o=>operatorReplay?o.receivedAt!==undefined&&o.receivedAt<=asOf:(o.receivedAt??o.time.wallMs)<=asOf);
  const add=(code:string,title:string,detail:string,records:Observation[],severity:BoundaryFinding['severity']='warning')=>{
    if(findings.length>=2000){omittedFindings++;return;}
    const recordIds=records.slice(0,24).map(observationEventId),time=records.reduce((n,o)=>Math.min(n,o.time.wallMs),asOf);
    findings.push({id:`boundary-${stableHash(code+canonicalJSON(recordIds))}`,code,title,detail,recordIds,time,severity});
  };
  for(const o of visible){const key=observationKey(o),old=unique.get(key);if(old){if(canonicalJSON(old)!==canonicalJSON(o))add('sequence-conflict','Conflicting producer sequence','Two different records claim the same producer incarnation and sequence. Neither is silently treated as corroboration.',[old,o],'critical');}else unique.set(key,o);}
  const records=[...unique.values()].sort((a,b)=>a.time.wallMs-b.time.wallMs||observationKey(a).localeCompare(observationKey(b)));
  const sourceMap=new Map(policy.sources.map(s=>[s.id,s])),bySource=new Map<string,Observation[]>(),operations=new Map<string,Observation[]>(),resource=new Map<string,Observation[]>(),bySandbox=new Map<string,Observation[]>();
  const interactions=new Map<string,Interaction>(),authority={permitted:0,denied:0,unknown:0};
  for(const o of records){
    const src=sourceMap.get(o.sourceId);if(!src||!sourceAccepts(src,o))add('unbound-source','Source outside configured scope','This record is retained as an unverified assertion, not attributed to an allowed source for this run, stage, surface, and sandbox.',[o]);
    const arr=bySource.get(o.sourceId)??[];arr.push(o);bySource.set(o.sourceId,arr);
    if(o.operationId){const key=JSON.stringify([o.runId,o.subject.sandboxId,o.operationId]),a=operations.get(key)??[];a.push(o);operations.set(key,a);}
    if(o.subject.sandboxId){const key=JSON.stringify([o.runId,o.subject.sandboxId]),a=bySandbox.get(key)??[];a.push(o);bySandbox.set(key,a);}
    if(o.target){const key=JSON.stringify([o.runId,o.subject.sandboxId,o.subject.isolationGroup,o.target.id,o.target.version,o.stage,o.effect]),edge=interactions.get(key)??{runId:o.runId,sandboxId:o.subject.sandboxId,group:o.subject.isolationGroup,target:o.target.id,kind:o.target.kind,stage:o.stage,effect:o.effect,count:0,recordIds:[]};edge.count++;if(edge.recordIds.length<24)edge.recordIds.push(observationEventId(o));interactions.set(key,edge);
      if(o.stage==='effect'&&o.status==='completed'){const a=resource.get(o.target.id)??[];a.push(o);resource.set(o.target.id,a);}}
    if(o.stage==='dispatch'||o.stage==='effect'){
      const result=authorize(o,policy);authority[result.decision]++;
      if(o.authority?.claimed&&!policy.grants.some(g=>g.id===o.authority?.grantId))add('claimed-approval','Approval claim is not a grant','Peer, tool, and transcript assertions cannot create operator policy.',[o]);
      if(consequential(o)&&result.decision==='denied')add('out-of-scope-effect','Effect outside supplied authorization',result.reason+' This is a policy comparison, not proof of malicious intent.',[o],'critical');
    }
    if(consequential(o)&&o.target?.boundary==='external')add('external-effect','External state-changing effect recorded','A discrete external effect is preserved even at whole-run zoom. Inspect its evidence and authorization; volume is not an impact measure.',[o],'info');
    if(o.surface==='sandbox'&&(o.facts.privileged===true||o.facts.hostNetwork===true||o.facts.engineSocket===true))add('sandbox-exposure','Broad sandbox capability reported','The recorded profile reports privilege, host networking, or engine-socket access. A sandbox label is not evidence of confinement.',[o]);
    if(consequential(o)&&o.facts.evaluatorArtifact===true)add('evaluator-change','Evaluation artifact changed','A recorded write changed an object identified as part of evaluation. Passing results before and after may not use the same acceptance criteria.',[o]);
    if(o.facts.monitorState==='disabled'||o.facts.monitorState==='failed')add('monitor-offline','Monitor not evaluating','This source explicitly reports disabled or failed monitoring; no clean verdict is inferred.',[o],'critical');
    if(typeof o.facts.dropped==='number'&&o.facts.dropped>0)add('telemetry-loss','Telemetry loss reported',`${o.facts.dropped} records were reported dropped. Recorded totals are incomplete.`,[o]);
  }
  for(const [key,ops] of operations){
    const bound=ops.filter(o=>{const src=sourceMap.get(o.sourceId);return src&&sourceAccepts(src,o);});
    const proposed=bound.filter(o=>o.stage==='intent'||o.stage==='dispatch'),actual=bound.filter(o=>o.stage==='runtime'||o.stage==='effect');
    // Keep at most two DISTINCT digests per configured source, enough to show disagreement.
    const examples:Observation[]=[],seenDigests=new Map<string,Set<string>>();
    for(const o of proposed)if(o.actionDigest){const seen=seenDigests.get(o.sourceId)??new Set<string>();
      if(seen.size<2&&!seen.has(o.actionDigest)){seen.add(o.actionDigest);examples.push(o);seenDigests.set(o.sourceId,seen);}}
    const mismatch=actual.some(b=>b.actionDigest&&examples.some(a=>a.sourceId!==b.sourceId&&a.actionDigest!==b.actionDigest));
    if(mismatch)add('action-mismatch','Dispatch and execution digests differ','Distinct configured sources disagree on the action identity for the same run/sandbox/operation. Instrumentation errors remain an alternative explanation.',ops,'critical');
    if(proposed.some(o=>o.effect==='read'||o.facts.readOnlyHint===true)&&actual.some(consequential))add('effect-mismatch','Read claim followed by a write effect','The same correlated operation includes a read/read-only claim and a completed state-changing effect. An HTTP method or MCP annotation was not treated as a guarantee.',ops,'critical');
    const stop=ops.filter(o=>o.action==='cancel.request'),after=actual.filter(o=>stop.length&&definitelyAfter(o,stop[0]));
    if(stop.length&&after.length)add('cancel-not-stop','Execution continues after cancellation request','Cancellation is a request, not evidence of termination. The recorded clocks establish later activity for this operation.',[...stop,...after]);
    void key;
  }
  if(policy.forbiddenCrossGroup)for(const [target,ops] of resource){
    const readers=new Map<string,Set<string>>();let unknownVersionReaderGroups=new Set<string>();
    for(const o of ops)if(o.effect==='read'&&o.subject.isolationGroup){const version=o.target?.version??'',groups=readers.get(version)??new Set<string>();groups.add(o.subject.isolationGroup);readers.set(version,groups);if(!version)unknownVersionReaderGroups.add(o.subject.isolationGroup);}
    const allReaderGroups=new Set([...readers.values()].flatMap(g=>[...g]));
    const other=(groups:Set<string>|undefined,group:string)=>!!groups&&(groups.size>1||groups.size===1&&!groups.has(group));
    const bridge=ops.some(w=>consequential(w)&&w.subject.isolationGroup&&(other(unknownVersionReaderGroups,w.subject.isolationGroup)||other(w.target?.version?readers.get(w.target.version):allReaderGroups,w.subject.isolationGroup)));
    if(bridge)add('shared-write-bridge','Shared writable resource crosses isolation groups',`Multiple groups read/write ${target}. This establishes a shared-state path under the supplied isolation policy, not intent or causal influence. Missing versions weaken the association.`,ops,'critical');
  }
  for(const ops of bySandbox.values()){
    let pendingStop:Observation|undefined,stopped:Observation|undefined,waiting:Observation|undefined;
    const afterStop:Observation[]=[],duringWait:Observation[]=[],requests:Observation[]=[];
    for(const o of ops){
      if(o.action==='sandbox.stop.request'){pendingStop=o;requests.push(o);}
      if(o.action==='sandbox.stopped'&&o.stage==='control'){stopped=o;if(pendingStop&&definitelyAfter(o,pendingStop))pendingStop=undefined;}
      if(o.action==='sandbox.started'){stopped=undefined;waiting=undefined;}
      if(o.action==='sandbox.waiting')waiting=o;
      if(o.action==='sandbox.resumed'&&waiting&&definitelyAfter(o,waiting))waiting=undefined;
      if(o.stage==='runtime'||o.stage==='effect'){
        if(stopped&&definitelyAfter(o,stopped)){if(!afterStop.length)afterStop.push(stopped);afterStop.push(o);}
        if(waiting&&definitelyAfter(o,waiting)){if(!duringWait.length)duringWait.push(waiting);duringWait.push(o);}
      }
    }
    if(pendingStop)add('stop-unconfirmed','Sandbox stop is unconfirmed','A stop request was recorded without a later stopped observation. Remote jobs and credentials are outside this lifecycle unless separately instrumented.',[pendingStop]);
    if(afterStop.length)add('activity-after-stop','Activity recorded after sandbox stopped','Recorded execution/effects occur after a stopped assertion for this run and sandbox. Inspect identity, clocks, observer correctness, and descendants.',afterStop,'critical');
    if(duringWait.length)add('waiting-active','Reported waiting overlaps later activity','Waiting describes one control state, not the absence of work across all observation surfaces.',duringWait);
  }
  for(const src of policy.sources){
    const all=(bySource.get(src.id)??[]).filter(o=>sourceAccepts(src,o));let gaps=0;const gapEvidence:Observation[]=[];
    const epochs=new Map<string,Map<number,Observation>>();for(const o of all){const seq=epochs.get(o.epoch)??new Map<number,Observation>();seq.set(o.sequence,o);epochs.set(o.epoch,seq);}
    for(const bySequence of epochs.values()){const a=[...bySequence.keys()].sort((a,b)=>a-b);for(let i=1;i<a.length;i++){const gap=Math.max(0,a[i]-a[i-1]-1);gaps+=gap;if(gap)gapEvidence.push(bySequence.get(a[i-1])!,bySequence.get(a[i])!);}}
    const timeOrdered=[...all].sort((a,b)=>a.sequence-b.sequence),clockEvidence:Observation[]=[];
    for(let i=1;i<timeOrdered.length;i++){const a=timeOrdered[i-1],b=timeOrdered[i];if(a.epoch!==b.epoch||a.time.clockId!==b.time.clockId)continue;
      if(a.time.monotonicMs!==undefined&&b.time.monotonicMs!==undefined&&a.time.taskMs!==undefined&&b.time.taskMs!==undefined){const real=b.time.monotonicMs-a.time.monotonicMs,task=b.time.taskMs-a.time.taskMs;if(real>0&&Math.abs(task-real)>Math.max(1000,real*.25))clockEvidence.push(a,b);}}
    if(clockEvidence.length)add('clock-divergence','Task clock differs from runtime clock','Same-source monotonic and task-time deltas differ by over 25% and 1 second. Simulation or clock configuration can explain this; it is not proof of evasion.',clockEvidence);
    if(gaps)add('sequence-gap','Producer sequence gaps',`${src.id} is missing ${gaps} sequence positions between observed records. Prefix and suffix completeness are unknown.`,gapEvidence);
    for(const surface of src.surfaces){const matching=all.filter(o=>o.surface===surface),last=matching.reduce<Observation|undefined>((a,b)=>(a&&(a.receivedAt??a.time.wallMs)>(b.receivedAt??b.time.wallMs))?a:b,undefined),at=last?.receivedAt??null;
      rows.push({sourceId:src.id,surface,status:!last?'missing':at===null?'unverified-time':last.facts.monitorState==='disabled'||last.facts.monitorState==='failed'?'disabled':asOf-at!>src.heartbeatMs?'stale':'observed',count:matching.length,lastReceivedAt:at,sequenceGaps:gaps});}
  }
  for(const surface of policy.expectedSurfaces)if(!policy.sources.some(s=>s.surfaces.includes(surface)))rows.push({sourceId:'—',surface,status:'unconfigured',count:0,lastReceivedAt:null,sequenceGaps:0});
  const absent=rows.filter(r=>r.status!=='observed');if(absent.length)add('coverage-gap','Observation coverage is incomplete',`${absent.length} configured/expected source-surface pairs are missing, stale, disabled, unconfigured, or lack recorder receipt stamps. No clean result is inferred for them.`,[]);
  return {version:1,omittedFindings,asOf,inputRecords:bundle.records.length,visibleRecords:visible.length,uniqueRecords:records.length,futureRecords:bundle.records.length-visible.length,findings:findings.sort((a,b)=>a.time-b.time||a.id.localeCompare(b.id)),coverage:rows,interactions:[...interactions.values()],runs:new Set(records.map(o=>o.runId)).size,sandboxes:new Set(records.map(o=>o.subject.sandboxId).filter(Boolean)).size,externalEffects:records.filter(o=>consequential(o)&&o.target?.boundary==='external').length,unknownEffects:records.filter(o=>o.effect==='unknown').length,authority,provenance:'Offline source and policy declarations; not authenticated by the viewer. Observed means reported by an attributed source, not independently certified.'};
}
const category:Record<Surface,Category>={'model-api':'reasoning',http:'network',mcp:'tool',git:'code',filesystem:'filesystem',process:'code',sandbox:'orchestration',dns:'network',socket:'network',message:'communication',database:'memory',artifact:'filesystem',browser:'browser',clipboard:'communication',other:'other'};
export function evidenceToTrace(bundle:EvidenceBundle):Trace {
  const base=bundle.records.reduce((n,o)=>Math.min(n,o.time.wallMs),bundle.asOf),id=`evidence-${stableHash(bundle.name)}`;
  const events:WhaleEvent[]=bundle.records.map(o=>({schemaVersion:1,id:observationEventId(o),traceId:id,startTime:Math.max(0,o.time.wallMs-base),endTime:Math.max(0,o.time.wallMs-base),agentId:`${o.runId}/${o.subject.agentId}`,category:category[o.surface],name:o.action,subtype:o.stage,status:o.status==='error'?'error':o.status==='completed'?'success':o.status==='started'?'running':'unknown',targetId:o.target?.id,targetType:o.target?.kind,attributes:{'evidence.surface':o.surface,'evidence.stage':o.stage,'evidence.effect':o.effect,'evidence.run':o.runId,'evidence.source':o.sourceId,'evidence.sandbox':o.subject.sandboxId??'unknown','evidence.isolationGroup':o.subject.isolationGroup??'unknown',...o.facts},observation:o}));
  events.sort((a,b)=>a.startTime-b.startTime||a.id.localeCompare(b.id));
  return {id,name:bundle.name,source:'jsonl',privacy:'metadata',events,duration:Math.max(1,bundle.asOf-base,events.reduce((n,e)=>Math.max(n,e.endTime),0)),originTime:String(base),warnings:['Boundary evidence is metadata-only but identifiers and behavioral timing remain sensitive.','Imported policy/provenance are declarations, not authenticated by this viewer.','No universal interception: only instrumented sources can be observed.'],metadata:{evidenceBundle:{...bundle,records:undefined},evidenceBase:base}};
}
export function bundleFromTrace(trace:Trace):EvidenceBundle|undefined {
  const records=trace.events.flatMap(e=>e.observation?[e.observation]:[]);if(!records.length)return undefined;
  const header=trace.metadata.evidenceBundle as Omit<EvidenceBundle,'records'>|undefined;
  if(!header)return undefined;return {...header,records};
}
export function boundaryFindings(trace:Trace):Finding[]{
  const bundle=bundleFromTrace(trace);if(!bundle)return [];const base=Number(trace.metadata.evidenceBase??0);
  return analyzeEvidence(bundle).findings.map(f=>({id:f.id,kind:'boundary',severity:f.severity,title:f.title,detail:f.detail,startTime:Math.max(0,f.time-base),endTime:Math.max(0,f.time-base)+1,eventIds:f.recordIds,evidence:{code:f.code,provenance:'declared',scope:'cross-run evidence case'}}));
}
