import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { RuntimeApplyNotice } from "../accounts/RuntimeApplyNotice";

type SystemInfo=Readonly<{build:{version:string;build_revision:string;build_target:string;rust_version:string;schema_version:number};uptime_seconds:number;configuration_application:"live";accepting_requests:boolean|null}>;
export function SystemInformation() {
  const info=useQuery({queryKey:["system-information"],queryFn:()=>call<SystemInfo>("getSystemInformation"),staleTime:60000,refetchOnMount:"always",retry:false});
  const build=info.data?.build;
  return <div className="card system-information" data-gap="top"><div className="page-head"><h3>网关服务</h3><button className="secondary" disabled={info.isFetching} onClick={()=>void info.refetch()}>刷新</button></div>
    {info.isError?<p role="alert">{asAppError(info.error).message}</p>:!build?<p>读取服务信息…</p>:<>
      <dl className="system-facts"><div><dt>版本</dt><dd>{build.version}</dd></div><div><dt>构建</dt><dd className="mono">{build.build_revision==="development"?"本地开发构建":build.build_revision.slice(0,12)}</dd></div><div><dt>本次运行</dt><dd>{Math.floor((info.data?.uptime_seconds??0)/3600)} 小时 {Math.floor(((info.data?.uptime_seconds??0)%3600)/60)} 分钟</dd></div><div><dt>请求处理</dt><dd>{info.data?.accepting_requests===true?"可接收新请求":info.data?.accepting_requests===false?"新请求已暂停":"未观测"}</dd></div></dl>
      {info.data?.accepting_requests===false?<RuntimeApplyNotice onApplied={()=>void info.refetch()}/>:null}
      <details><summary>构建详情</summary><dl className="settings-facts"><dt>完整修订</dt><dd className="mono">{build.build_revision}</dd><dt>平台</dt><dd>{build.build_target}</dd><dt>Rust</dt><dd>{build.rust_version}</dd><dt>数据结构</dt><dd>{build.schema_version}</dd><dt>配置应用</dt><dd>保存后生效</dd></dl></details>
    </>}
    <div className="system-links"><Link to="/egress">网络与出口</Link><Link to="/runtime">运行诊断</Link><Link to="/versions">高级配置</Link></div>
  </div>;
}
