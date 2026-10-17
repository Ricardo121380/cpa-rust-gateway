import assert from 'node:assert/strict';
import fs from 'node:fs';
import OpenAI from 'openai';
import Anthropic from '@anthropic-ai/sdk';

// All SDK HTTP calls are replaced in memory. No socket or Provider is used.
let mockFetchCalls = 0;
const extension = { version: 1, usage: { presence: 'object', raw_json: '{"zero":0,"nil":null,"empty":[],"large":9007199254740993}' } };
const jsonResponse = value => new Response(JSON.stringify(value), { headers: { 'content-type': 'application/json', 'x-request-id': 'synthetic-request' } });
const sseResponse = (events, chat = false) => new Response(events.map(e => `${chat ? '' : `event: ${e.type}\n`}data: ${JSON.stringify(e)}\n\n`).join('') + (chat ? 'data: [DONE]\n\n' : ''), { headers: { 'content-type': 'text/event-stream' } });
const fakeFetch = producer => async () => { mockFetchCalls++; return producer(); };
const open = producer => new OpenAI({ apiKey: 'synthetic', baseURL: 'https://synthetic.invalid/v1', maxRetries: 0, fetch: fakeFetch(producer) });
const anth = producer => new Anthropic({ apiKey: 'synthetic', baseURL: 'https://synthetic.invalid', maxRetries: 0, fetch: fakeFetch(producer) });
const chatParams = { model: 'synthetic-model', messages: [{ role: 'user', content: 'synthetic' }] };
const msgParams = { ...chatParams, max_tokens: 8 };
const responseParams = { model: 'synthetic-model', input: 'synthetic' };
const usage = { input_tokens: 1, output_tokens: 1, total_tokens: 2, cpar_native_usage: extension.usage };
const messageItem = { id: 'msg-synthetic', type: 'message', status: 'completed', role: 'assistant', content: [{ type: 'output_text', text: 'ab', annotations: [] }] };
const response = { id: 'resp-synthetic', object: 'response', created_at: 1, status: 'completed', model: 'synthetic-model', output: [messageItem], usage, cpar_fidelity: extension };
const message = { id: 'msg-synthetic', type: 'message', role: 'assistant', model: 'synthetic-model', content: [{ type: 'text', text: 'ab' }], stop_reason: 'end_turn', stop_sequence: null, usage: { input_tokens: 1, output_tokens: 1, cpar_native_usage: extension.usage }, cpar_fidelity: extension };
const chunk = (delta, finish_reason = null, root = {}) => ({ id: 'chat-synthetic', object: 'chat.completion.chunk', created: 1, model: 'synthetic-model', choices: [{ index: 0, delta, finish_reason }], ...root });
const chatEvents = [chunk({ role: 'assistant', content: 'a', cpar_native_output: { fragment: 'a' } }), chunk({ content: 'b', cpar_native_output: { fragment: 'b' } }), chunk({}, 'stop', { usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2, cpar_native_usage: extension.usage }, cpar_fidelity: extension })];
const messageEvents = [
  { type: 'message_start', message: { ...message, content: [], stop_reason: null, usage: { input_tokens: 1, output_tokens: 0 } } },
  { type: 'content_block_start', index: 0, content_block: { type: 'text', text: '' } },
  { type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: 'ab', cpar_native_output: { fragment: 'ab' } } },
  { type: 'cpar.fidelity.delta', fragment: 'synthetic-extra' },
  { type: 'content_block_stop', index: 0 },
  { type: 'message_delta', delta: { stop_reason: 'end_turn', stop_sequence: null }, usage: { output_tokens: 1, cpar_native_usage: extension.usage }, cpar_fidelity: extension },
  { type: 'message_stop' },
];
const responseEvents = [
  { type: 'response.created', sequence_number: 0, response: { ...response, status: 'in_progress', output: [], usage: null } },
  { type: 'response.output_item.added', sequence_number: 1, output_index: 0, item: { ...messageItem, status: 'in_progress', content: [] } },
  { type: 'response.content_part.added', sequence_number: 2, item_id: messageItem.id, output_index: 0, content_index: 0, part: { type: 'output_text', text: '', annotations: [] } },
  { type: 'response.output_text.delta', sequence_number: 3, item_id: messageItem.id, output_index: 0, content_index: 0, delta: 'ab' },
  { type: 'response.output_text.done', sequence_number: 4, item_id: messageItem.id, output_index: 0, content_index: 0, text: 'ab' },
  { type: 'response.content_part.done', sequence_number: 5, item_id: messageItem.id, output_index: 0, content_index: 0, part: messageItem.content[0] },
  { type: 'response.output_item.done', sequence_number: 6, output_index: 0, item: messageItem },
  { type: 'cpar.fidelity.delta', sequence_number: 7, fragment: 'synthetic-extra' },
  { type: 'response.completed', sequence_number: 8, response },
];
const results = { versions: { openai: '7.36.0', anthropic: '0.135.0', node: process.version }, fixtures: 'synthetic only', external_network_calls: 0, records: [] };
async function run(name, fn) {
  try { results.records.push({ name, status: 'PASS_EXECUTION', result: await fn() }); }
  catch (e) { results.records.push({ name, status: 'OBSERVED_REJECTION', error: String(e.message) }); }
}
await run('chat-json', async () => {
  const value = await open(() => jsonResponse({ ...chatEvents[2], object: 'chat.completion', choices: [{ index: 0, finish_reason: 'stop', message: { role: 'assistant', content: 'ab', cpar_native_output: { fragment: 'ab' } } }] })).chat.completions.create(chatParams);
  assert.deepEqual(value.cpar_fidelity, extension);
  return { root_extension: true, message_extension: !!value.choices[0].message.cpar_native_output, usage_extension: !!value.usage.cpar_native_usage };
});
await run('responses-json', async () => {
  const value = await open(() => jsonResponse(response)).responses.create(responseParams);
  assert.deepEqual(value.cpar_fidelity, extension);
  return { root_extension: true, usage_extension: !!value.usage.cpar_native_usage, output_text: value.output_text };
});
await run('messages-json', async () => {
  const value = await anth(() => jsonResponse(message)).messages.create(msgParams);
  assert.deepEqual(value.cpar_fidelity, extension);
  return { root_extension: true, usage_extension: !!value.usage.cpar_native_usage };
});
await run('chat-sse-helper', async () => {
  const stream = open(() => sseResponse(chatEvents, true)).chat.completions.stream(chatParams);
  const raw = []; for await (const e of stream) raw.push(e);
  const value = await stream.finalChatCompletion();
  assert.equal(value.choices[0].message.content, 'ab');
  return { raw_chunks: raw.length, raw_delta_extensions: raw.filter(x => x.choices?.[0]?.delta?.cpar_native_output).length, final_delta_extension: value.choices[0].message.cpar_native_output ?? null, root_extension: !!value.cpar_fidelity, usage_extension: !!value.usage?.cpar_native_usage };
});
await run('responses-sse-helper', async () => {
  const stream = open(() => sseResponse(responseEvents)).responses.stream(responseParams);
  const raw = []; for await (const e of stream) raw.push(e);
  const value = await stream.finalResponse();
  assert.equal(value.output_text, 'ab');
  return { events: raw.length, custom_events_delivered: raw.filter(x => x.type === 'cpar.fidelity.delta').length, final_root_extension: !!value.cpar_fidelity, final_usage_extension: !!value.usage?.cpar_native_usage, custom_fragment_accumulated: value.fragment ?? null };
});
await run('responses-sse-helper-without-custom-event-control', async () => {
  const events = responseEvents.filter(x => x.type !== 'cpar.fidelity.delta');
  const stream = open(() => sseResponse(events)).responses.stream(responseParams);
  const value = await stream.finalResponse();
  assert.equal(value.output_text, 'ab');
  return { final_root_extension: !!value.cpar_fidelity, final_usage_extension: !!value.usage?.cpar_native_usage, output_text: value.output_text };
});
await run('messages-sse-helper', async () => {
  const stream = anth(() => sseResponse(messageEvents)).messages.stream(msgParams);
  const raw = []; for await (const e of stream) raw.push(e);
  const value = await stream.finalMessage();
  assert.equal(value.content[0].text, 'ab');
  return { events: raw.length, custom_events_delivered: raw.filter(x => x.type === 'cpar.fidelity.delta').length, raw_delta_extensions: raw.filter(x => x.delta?.cpar_native_output).length, final_content_extension: value.content[0].cpar_native_output ?? null, final_usage_extension: value.usage.cpar_native_usage ?? null, root_from_start: !!value.cpar_fidelity };
});
const parsed = JSON.parse(extension.usage.raw_json);
results.numeric_precision = { raw_json_preserved: extension.usage.raw_json.includes('9007199254740993'), ordinary_JSON_parse_value: String(parsed.large), exact_integer_preserved: String(parsed.large) === '9007199254740993' };
results.mock_fetch_calls = mockFetchCalls;
fs.writeFileSync(new URL('./results.json', import.meta.url), JSON.stringify(results, null, 2) + '\n');
console.log(JSON.stringify(results, null, 2));
