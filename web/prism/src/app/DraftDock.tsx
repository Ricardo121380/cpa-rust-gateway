import {useLocation,useNavigate} from "react-router-dom";
import {GlassSurface} from "../components/glass/GlassSurface";
import {useOperationBoundary} from "../components/OperationBoundary";
import {useConfigurationLifecycle} from "../features/config-versions/ConfigurationLifecycleHost";
import {useVersionStore} from "../features/config-versions/versionStore";

/** The dock only opens review; applying remains inside the reviewed workspace. */
export function DraftDock(){
 const context=useVersionStore(state=>state.context);
 const pending=useVersionStore(state=>state.pending);
 const admission=useOperationBoundary();const lifecycle=useConfigurationLifecycle();const navigate=useNavigate();const location=useLocation();
 const target=context?.status==="draft"?context.configVersionId:pending?.id;
 if(!target)return null;
 const reviewing=location.pathname==="/versions"&&new URLSearchParams(location.search).get("review")===target;
 const review=()=>admission.request(()=>navigate(`/versions?${new URLSearchParams({review:target})}`));
 return <GlassSurface as="footer" className="dock" material="draft" pane="dock">
  <span className="dock-mark" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M9 4H5v16h14v-4 M9 8h7 M9 12h4 M15 3v6 M12 6h6"/></svg></span>
  <div className="dock-copy"><strong>待应用变更</strong><p><span className="dock-saved">草稿已保存 · </span>当前服务未改变</p></div>
  <button type="button" className={reviewing?"secondary":"primary"} disabled={lifecycle.active||reviewing} onClick={review}>{reviewing?"正在核对":"查看变更"}</button>
 </GlassSurface>;
}
