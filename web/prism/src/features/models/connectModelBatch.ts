import { isCancelledError } from "@tanstack/react-query";
import { connectModel, type ConfigurationTask } from "./connectModel";

export type ModelBatchRow = Readonly<{model:string;state:"waiting"|"saving"|"saved"|"existing"|"uncertain"}>;

/** Stop after any uncertain write; keep an honest per-model draft receipt without replay. */
export async function connectModelBatch(task:ConfigurationTask, models:readonly string[], endpointId:string,
  onProgress:(rows:readonly ModelBatchRow[])=>void) {
  if(!models.length||models.length>20||new Set(models).size!==models.length)throw new Error("请选择 1–20 个不同模型。");
  const rows:ModelBatchRow[]=models.map(model=>({model,state:"waiting"}));
  onProgress([...rows]);
  for(const [index,model] of models.entries()) {
    task.assertOwner();
    rows[index]={model,state:"saving"};onProgress([...rows]);
    try {
      const result=await connectModel(task,{upstreamModel:model,endpointId,allowUnlisted:false});
      rows[index]={model,state:result.added?"saved":"existing"};
    } catch(error) {
      if(!isCancelledError(error)){rows[index]={model,state:"uncertain"};onProgress([...rows]);}
      throw error;
    }
    onProgress([...rows]);
  }
  return rows;
}
