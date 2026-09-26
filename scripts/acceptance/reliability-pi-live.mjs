// Explicit M4 Pi four-turn acceptance. Synthetic content; no model-selected tool execution.
// --check-client verifies SDK wire bounds locally without network or inference allowance.
import fs from 'node:fs';
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import path from 'node:path';
const [mode, modulePath, configPath, ledgerPath, receiptPath, preflightPath, expectedRevision] = process.argv.slice(2);
assert(['--check-client', '--execute'].includes(mode) && modulePath, 'Explicit mode and installed Pi module required');
const {stream} = await import(pathToFileURL(modulePath).href);
const offline = mode === '--check-client';
function requirePreflight() {
  assert(preflightPath && /^[a-f0-9]{40}$/.test(expectedRevision ?? ''), 'Explicit preflight and exact release revision required');
  const pre = JSON.parse(fs.readFileSync(preflightPath, 'utf8'));
  assert.equal(pre.candidate_revision, expectedRevision, 'Running release differs from expected revision');
  const age = Date.now() - pre.observed_at_ms;
  assert(Number.isFinite(age) && age >= 0 && age < 600_000, 'Fresh route observation required');
  assert.equal(pre.routes?.[config.model]?.max_attempts, 1, 'Route must permit exactly one upstream attempt');
}
const config = offline ? {baseUrl:'http://127.0.0.1:1/v1',model:'synthetic',apiKey:'synthetic'} : JSON.parse(fs.readFileSync(configPath,'utf8'));
const target = new URL(config.baseUrl + '/responses');
assert(offline || (target.origin === 'https://cpar.142857142.xyz' && target.pathname === '/v1/responses'));
assert(typeof config.apiKey === 'string' && config.apiKey.length > 0 && !config.apiKey.startsWith('!'));
const model = {id:config.model,name:config.model,api:'openai-responses',provider:'cpar',baseUrl:config.baseUrl,reasoning:true,input:['text'],contextWindow:131072,maxTokens:512,cost:{input:0,output:0,cacheRead:0,cacheWrite:0}};
const tool={name:'diagnostic_echo',description:'Synthetic echo; no external action.',parameters:{type:'object',properties:{value:{type:'string'}},required:['value'],additionalProperties:false}};
const receipt={client:'Pi',clientModule:modulePath,model:config.model,maxOutputTokens:512,maxRetries:0,offline,turns:[],passed:false};
let lock;
if (!offline) {
  assert((fs.statSync(configPath).mode & 0o777) === 0o600,'Private configuration must be 0600');
  requirePreflight();
  lock = fs.openSync(ledgerPath+'.lock','wx',0o600);
}
function persist() { if (!offline) fs.writeFileSync(receiptPath,JSON.stringify(receipt,null,2),{mode:0o600}); }
async function run(messages,required,turn) {
  let calls=0,result;
  const row={turn,required,wireRequests:0,requestId:null,result:'not_sent',events:{}};
  receipt.turns.push(row);persist();
  const checkedFetch = async (url,options) => {
    assert.equal(new URL(String(url)).href,target.href);
    assert.equal(options.method,'POST');
    const payload=JSON.parse(options.body);
    assert.equal(payload.model,model.id);assert.equal(payload.max_output_tokens,512);assert.equal(payload.stream,true);
    assert.equal(payload.reasoning.effort,'low');
    assert(++calls===1,'Refuse SDK retry');row.wireRequests=calls;
    if (!offline) {
      requirePreflight();
      const ledger=JSON.parse(fs.readFileSync(ledgerPath,'utf8'));
      assert.equal(ledger.allowance,12);assert.equal(ledger.plan,'cpar-reliability-20260926');
      assert(ledger.attempts.length<12);assert.equal(ledger.maxOutputTokens,512);
      ledger.attempts.push({ordinal:ledger.attempts.length+1,channel:'grok-build',model:model.id,protocol:'responses-sse',client:'Pi',turn,maxOutputTokens:512,time:new Date().toISOString(),status:'reserved_before_send'});
      const temp=ledgerPath+'.pending';
      const fd=fs.openSync(temp,'wx',0o600);try {fs.writeFileSync(fd,JSON.stringify(ledger,null,2));fs.fsyncSync(fd);}finally{fs.closeSync(fd);}
      fs.renameSync(temp,ledgerPath);
      const directory=fs.openSync(path.dirname(ledgerPath),'r');try {fs.fsyncSync(directory);}finally{fs.closeSync(directory);}
      row.result='send_started';persist();
      const response=await fetch(url,{...options,redirect:'error'});
      const id=response.headers.get('x-request-id');
      row.requestId=id && /^[a-zA-Z0-9:._-]{1,200}$/.test(id) ? id : null;row.httpStatus=response.status;persist();
      return response;
    }
    // Force an HTTP failure to prove the installed SDK does not retry.
    return new Response('{"error":{"message":"synthetic refusal"}}',{status:503,headers:{'content-type':'application/json'}});
  };
  for await (const event of stream(model,{messages,tools:[tool]}, {
    apiKey:config.apiKey,maxTokens:512,maxRetries:0,timeoutMs:90000,
    signal:AbortSignal.timeout(90000),cacheRetention:'none',reasoningEffort:'low',
    fetch:checkedFetch,...(required?{toolChoice:'required'}:{}),
  })) {
    if(['start','text_start','text_delta','text_end','thinking_start','thinking_delta','thinking_end','toolcall_start','toolcall_delta','toolcall_end','done','error'].includes(event.type)) row.events[event.type]=(row.events[event.type]??0)+1;
    if(event.type==='done')result=event.message;
    if(event.type==='error'){
      row.result='client_stream_error';
      const message=String(event.error?.errorMessage ?? '');
      row.errorClass=['UpstreamProtocolError','StreamTruncated','CredentialUnavailable','ProviderPermanent','ProviderTransient'].find(code=>message.includes(code)) ?? (message.includes('without')&&message.includes('terminal')?'missing_terminal':'unknown');
      persist();
    }
  }
  assert.equal(calls,1,'Expected exactly one wire submission');
  if(offline){assert.equal(row.result,'client_stream_error');return;}
  assert(result,'No completed Pi message');
  row.stopReason=result.stopReason;row.outputTokens=result.usage.output;
  assert(result.usage.output<=512,'Reported output exceeds authorized cap');
  row.result='completed';persist();return result;
}
try {
  const prompt={role:'user',content:'Call diagnostic_echo with value CPAR_TOOL_OK. After the tool returns, reply exactly CPAR_TOOL_OK.',timestamp:Date.now()};
  if(offline){await run([prompt],true,0);console.log(JSON.stringify({passed:true,networkRequests:0,sdkSubmissions:1,maxOutputTokens:512,reasoning:'low',retryOn503:false}));}
  else {
    const history=[prompt];
    for(let pair=0;pair<2;pair++){
      if(pair)history.push({...prompt,timestamp:Date.now()});
      const response=await run(history,true,pair*2+1);
      const calls=response.content.filter(item=>item.type==='toolCall');
      assert.equal(calls.length,1);assert.equal(calls[0].name,tool.name);assert.deepEqual(calls[0].arguments,{value:'CPAR_TOOL_OK'});
      history.push(response,{role:'toolResult',toolCallId:calls[0].id,toolName:tool.name,content:[{type:'text',text:'CPAR_TOOL_OK'}],isError:false,timestamp:Date.now()});
      const final=await run(history,false,pair*2+2);
      assert(final.content.some(item=>item.type==='text' && item.text.includes('CPAR_TOOL_OK')));
      assert(!final.content.some(item=>item.type==='toolCall'));history.push(final);
    }
    receipt.passed=true;persist();console.log(JSON.stringify({passed:true,turns:4,receipt:receiptPath}));
  }
} catch {
  persist();console.error(JSON.stringify({passed:false,failure:'bounded_client_acceptance_failed',turns:receipt.turns.map(({turn,result,httpStatus,requestId})=>({turn,result,httpStatus,requestId}))}));process.exitCode=1;
} finally {
  if(lock!==undefined){fs.closeSync(lock);fs.unlinkSync(ledgerPath+'.lock');}
}
