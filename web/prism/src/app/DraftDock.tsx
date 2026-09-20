import {useNavigate} from "react-router-dom";
import {GlassSurface} from "../components/glass/GlassSurface";
import {useOperationBoundary} from "../components/OperationBoundary";
import {useConfigurationLifecycle} from "../features/config-versions/ConfigurationLifecycleHost";
import {useVersionStore} from "../features/config-versions/versionStore";

/** Both entry points lead to the same reviewed configuration, never directly to a POST. */
export function DraftDock(){
 const context=useVersionStore(state=>state.context);
 const pending=useVersionStore(state=>state.pending);
 const admission=useOperationBoundary();const lifecycle=useConfigurationLifecycle();const navigate=useNavigate();
 const target=context?.status==="draft"?context.configVersionId:pending?.id;
 if(!target)return null;
 const review=(apply:boolean)=>admission.request(()=>navigate(`/versions?${new URLSearchParams({review:target,...(apply?{intent:"apply"}:{})})}`));
 return <GlassSurface as="footer" className="dock" material="draft" pane="dock">
  <span>待应用草稿 <span className="muted">核对变更后统一生效</span></span>
  <span className="dock-actions"><button type="button" className="secondary" disabled={lifecycle.active} onClick={()=>review(false)}>查看变更</button><button type="button" disabled={lifecycle.active} onClick={()=>review(true)}>校验并应用</button></span>
 </GlassSurface>;
}
