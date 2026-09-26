import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
const script=new URL('./reliability-pi-live.mjs',import.meta.url);
const revision='1'.repeat(40);
for(const scenario of ['missing','malformed','wrong_revision','stale','future','retry_route','absent_route']) {
  test(`preflight ${scenario} rejects before SDK invocation or quota reservation`,()=>{
    const dir=fs.mkdtempSync(path.join(os.tmpdir(),'cpar-preflight-test-'));
    try {
      const config=path.join(dir,'config.json'), ledger=path.join(dir,'ledger.json'), receipt=path.join(dir,'receipt.json'), preflight=path.join(dir,'preflight.json'), module=path.join(dir,'client.mjs');
      fs.writeFileSync(config,JSON.stringify({baseUrl:'https://cpar.142857142.xyz/v1',model:'grok-4.5',apiKey:'synthetic-test-only'}),{mode:0o600});
      fs.writeFileSync(ledger,JSON.stringify({plan:'cpar-reliability-20260926',allowance:12,maxOutputTokens:512,attempts:[]}));
      const ledgerBefore=fs.readFileSync(ledger,'utf8');
      fs.writeFileSync(module,`import fs from 'node:fs';export async function* stream(){fs.writeFileSync(${JSON.stringify(path.join(dir,'sdk-invoked'))},'unexpected');throw Error('SDK must not be invoked');}`);
      const pre={candidate_revision:revision,observed_at_ms:Date.now(),routes:{'grok-4.5':{max_attempts:1}}};
      if(scenario==='wrong_revision')pre.candidate_revision='2'.repeat(40);
      if(scenario==='stale')pre.observed_at_ms-=600001;
      if(scenario==='future')pre.observed_at_ms+=600001;
      if(scenario==='retry_route')pre.routes['grok-4.5'].max_attempts=2;
      if(scenario==='absent_route')pre.routes={};
      if(scenario!=='missing')fs.writeFileSync(preflight,scenario==='malformed'?'[]':JSON.stringify(pre));
      const run=spawnSync(process.execPath,[script.pathname,'--execute',module,config,ledger,receipt,preflight,revision],{encoding:'utf8',timeout:5000});
      assert.notEqual(run.status,0);assert.equal(fs.existsSync(path.join(dir,'sdk-invoked')),false);
      assert.equal(fs.readFileSync(ledger,'utf8'),ledgerBefore);assert.equal(fs.existsSync(ledger+'.lock'),false);
    } finally {fs.rmSync(dir,{recursive:true,force:true});}
  });
}
