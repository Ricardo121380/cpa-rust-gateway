// Explicit live acceptance: four inference requests per model; no external tool execution.
// Uses the selected existing provider key in memory; never prints credentials or model content.
import fs from 'node:fs';
import assert from 'node:assert/strict';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const [execute, modulePath, configPath, providerName, ...modelIds] = process.argv.slice(2);
if(execute !== '--execute' || !modulePath || !configPath || !providerName || modelIds.length === 0) {
 console.error('Usage: node scripts/verify-pi-responses-tools.mjs --execute <pi-ai openai-responses.js> <models.json> <provider name> <model id> [...]');
 process.exit(2);
}
const {stream} = await import(pathToFileURL(path.resolve(modulePath)).href);
const p=JSON.parse(fs.readFileSync(configPath,'utf8')).providers[providerName];
const tool={name:'diagnostic_echo',description:'Synthetic diagnostic echo; no external action.',parameters:{type:'object',properties:{value:{type:'string'}},required:['value'],additionalProperties:false}};
for(const id of modelIds) {
 const model={...p.models[0],cost:{input:0,output:0,cacheRead:0,cacheWrite:0},id,api:'openai-responses',provider:providerName,baseUrl:p.baseUrl};
 const user={role:'user',content:'Call diagnostic_echo with value CPAR_TOOL_OK. After the tool returns, reply exactly CPAR_TOOL_OK.',timestamp:Date.now()};
 const options={apiKey:p.apiKey,maxTokens:2048,maxRetries:0,timeoutMs:90000,cacheRetention:'none'};
 async function run(messages,required){
  const events={};let result;
  for await(const e of stream(model,{messages,tools:[tool]},{...options,...(required?{toolChoice:'required'}:{})})) {
   events[e.type]=(events[e.type]||0)+1;
   if(e.type==='done')result=e.message;
   if(e.type==='error') {console.log(JSON.stringify({id,error:true,events,failure:'client_stream_error'}));throw Error('Pi stream error');}
  }
  assert(result);console.log(JSON.stringify({id,phase:required?'tool_call':'tool_result',stopReason:result.stopReason,events}));return result;
 }
 const first=await run([user],true);const calls=first.content.filter(x=>x.type==='toolCall');assert.equal(calls.length,1);assert.equal(calls[0].name,'diagnostic_echo');assert.deepEqual(calls[0].arguments,{value:'CPAR_TOOL_OK'});
 const result={role:'toolResult',toolCallId:calls[0].id,toolName:'diagnostic_echo',content:[{type:'text',text:'CPAR_TOOL_OK'}],isError:false,timestamp:Date.now()};
 const final=await run([user,first,result],false);assert(final.content.some(x=>x.type==='text'&&x.text.includes('CPAR_TOOL_OK')));assert(!final.content.some(x=>x.type==='toolCall'));
 const nextUser={role:'user',content:'Call diagnostic_echo again with value CPAR_TOOL_OK. After its result, reply exactly CPAR_TOOL_OK.',timestamp:Date.now()};
 const history=[user,first,result,final,nextUser];
 const next=await run(history,true);
 const nextCalls=next.content.filter(x=>x.type==='toolCall');
 assert.equal(nextCalls.length,1);assert.equal(nextCalls[0].name,'diagnostic_echo');assert.deepEqual(nextCalls[0].arguments,{value:'CPAR_TOOL_OK'});
 const nextResult={...result,toolCallId:nextCalls[0].id,timestamp:Date.now()};
 const last=await run([...history,next,nextResult],false);
 assert(last.content.some(x=>x.type==='text'&&x.text.includes('CPAR_TOOL_OK')));
 assert(!last.content.some(x=>x.type==='toolCall'));
 console.log(JSON.stringify({id,four_turn_text_and_tool_replay:'PASS'}));
}
