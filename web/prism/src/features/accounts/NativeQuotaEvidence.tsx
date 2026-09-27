import {useQuery} from "@tanstack/react-query";
import {asAppError} from "../../api/errors";
import {call} from "../../api/client";
import {AccountQuotaEvidence,type AccountQuota} from "./AccountQuotaEvidence";
import {readAccountMetadata} from "./accountMetadataQueue";
import type {NativeAccount} from "./NativeAccounts";
const errors:Record<string,string>={not_connected:"服务尚未接入此渠道的额度读取。",busy:"额度读取繁忙，请稍后重试。",unauthorized:"渠道授权已失效，请重新授权或更新凭据。",forbidden:"渠道或其访问防护拒绝了额度读取，请检查会话与出口。",invalid_response:"渠道额度响应格式已变化，暂时无法可靠展示。",unavailable:"额度读取暂不可用，请稍后重试。"};
export function nativeQuotaQuery(account:NativeAccount) {
 return {queryKey:["native-account-usage",account.id,account.revision],retry:false as const,staleTime:300_000,refetchOnWindowFocus:false,
  queryFn:({signal}:{signal:AbortSignal})=>readAccountMetadata(signal,()=>call<{observation:AccountQuota|null;error:string|null}>("getNativeAccountUsage",{path:{account_id:account.id},query:{revision:account.revision},signal}))};
}
export function NativeQuotaEvidence({account}:{account:NativeAccount}) {
 const query=useQuery(nativeQuotaQuery(account));
 const failure=query.data?.error;
 return <>{failure?<p role="alert">{errors[failure]??errors.unavailable}</p>:null}<AccountQuotaEvidence observation={query.data?.observation} error={query.isError?(asAppError(query.error).kind==="conflict"?"configuration_changed":"unavailable"):undefined} loading={query.isFetching} onRefresh={()=>void query.refetch()}/></>;
}
