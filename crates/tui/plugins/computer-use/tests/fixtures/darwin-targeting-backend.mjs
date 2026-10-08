// The real backend receives app-handler arguments. Only its native executable
// is substituted; no installed helper, app, or desktop is involved.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { run } from '../../src/exec.mjs';
import { create as createDarwin } from '../../src/backends/darwin.mjs';

export function create() {
  return createDarwin({exec:{async run(cmd,args,opts) {
    assert.equal(cmd,path.join(process.env.CODEWHALE_CU_APP_BUNDLE,'Contents','MacOS','accessibility'));
    const request=JSON.parse(args[0]);
    assert.ok(['list_windows','get_app_state','resolve_element','window_info'].includes(request.tool));
    fs.appendFileSync(process.env.CU_TARGETING_CALLS,JSON.stringify(request)+'\n');
    if(process.env.CU_TARGETING_NATIVE==='1') return run(cmd,args,opts);
    return {code:0,stderr:'',stdout:JSON.stringify({found:true,pid:123,name:'Fixture',bundle_id:'test.fixture',windows:[],elements:[]})};
  }}});
}
