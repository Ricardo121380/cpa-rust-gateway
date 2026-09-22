import {beforeEach,describe,expect,it,vi} from "vitest";
vi.mock("../../api/client",()=>({call:vi.fn()}));
import {call} from "../../api/client";
import {readAccountRuntimeSummary} from "./AccountRuntimeSummary";
const read=vi.mocked(call);
beforeEach(()=>read.mockReset());
describe("account list runtime snapshot",()=>{
 it("consumes all pages before reporting absence",async()=>{
  read.mockResolvedValueOnce({snapshot_id:"s",observed_at_ms:1,items:[],next_cursor:"next"}).mockResolvedValueOnce({snapshot_id:"s",observed_at_ms:1,items:[{account_id:"later"}],next_cursor:null});
  expect((await readAccountRuntimeSummary()).rows).toEqual([{account_id:"later"}]);
  expect(read.mock.calls[1]?.[1]?.query).toEqual({limit:100,cursor:"next"});
 });
 it("rejects mixed snapshots",async()=>{
  read.mockResolvedValueOnce({snapshot_id:"s",items:[],next_cursor:"next"}).mockResolvedValueOnce({snapshot_id:"changed",items:[],next_cursor:null});
  await expect(readAccountRuntimeSummary()).rejects.toThrow("快照已变化");
 });
 it("bounds enumeration without treating truncation as absence",async()=>{
  read.mockResolvedValue({snapshot_id:"s",items:[],next_cursor:"more"});
  await expect(readAccountRuntimeSummary()).rejects.toThrow("超出本次范围");expect(read).toHaveBeenCalledTimes(100);
 });
});
