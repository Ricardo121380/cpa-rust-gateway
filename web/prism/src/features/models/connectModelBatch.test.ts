import {describe,expect,it,vi} from "vitest";
import {connectModelBatch,type ModelBatchRow} from "./connectModelBatch";
import type {ConfigurationTask} from "./connectModel";

describe("directory model batch receipts",()=>{
  it("stops after a partially saved model and leaves later models untouched",async()=>{
    let routes=0;
    const mutate=vi.fn(async(op:string)=>{if(op==="createRoute"&&++routes===2)throw new Error("write conflict");return {};});
    const task={assertOwner:vi.fn(),mutate,read:vi.fn(async(op:string)=>op==="listPublicModels"?[]:{items:[],revision:"rev-0",next_cursor:null})} as unknown as ConfigurationTask;
    const progress:ModelBatchRow[][]=[];
    await expect(connectModelBatch(task,["Exact/A","Exact/B","Exact/C"],"endpoint",rows=>progress.push([...rows]))).rejects.toThrow("write conflict");
    expect(progress.at(-1)).toEqual([{model:"Exact/A",state:"saved"},{model:"Exact/B",state:"uncertain"},{model:"Exact/C",state:"waiting"}]);
    expect(mutate.mock.calls.filter(([op])=>op==="createPublicModel")).toHaveLength(2);
    expect(mutate.mock.calls.filter(([op])=>op==="createRouteCandidate")).toHaveLength(1);
  });
  it("refuses a duplicate selection before any configuration write",async()=>{
    const mutate=vi.fn();const task={mutate} as unknown as ConfigurationTask;
    await expect(connectModelBatch(task,["Exact/A","Exact/A"],"endpoint",()=>{})).rejects.toThrow("不同模型");
    expect(mutate).not.toHaveBeenCalled();
  });
});
