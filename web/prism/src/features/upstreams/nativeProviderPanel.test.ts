import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { SubresourcePanel } from "./SubresourcePanel";

vi.mock("../accounts/inventory", () => ({ useManagedInventory: (kind: string) => ({
  data: {pages:[{items:kind === "endpoints" ? [{id:"build-endpoint",upstream_id:"build",adapter_id:"grok.build.responses",api_format:"openai/responses",base_url:"https://example.test",enabled:true}] : []}]},
}) }));
vi.mock("../accounts/AccountRuntimeSummary", () => ({ useAccountRuntimeSummary: () => ({
  data:{rows:[{provider_id:"build",channel_id:"build-endpoint",account_id:"native"}]},
}) }));
vi.mock("../accounts/NativeAccounts", () => ({ useNativeAccounts: () => ({
  data:{pages:[{items:[{id:"native",provider:"grok_build",enabled:true,auth_status:"active",revision:1,import_batch_id:"autoreg",identity:{email:"native@example.test",phone:null,username:null}}]}]},
}) }));

describe("native provider details", () => {
  it("renders native identities and real runtime count without ordinary binding controls", () => {
    const client = new QueryClient({defaultOptions:{queries:{retry:false}}});
    const html = renderToStaticMarkup(createElement(QueryClientProvider,{client},createElement(MemoryRouter,{},
      createElement(SubresourcePanel,{upstreamId:"build",onAddAccount:()=>{},onActionActiveChange:()=>{}}))));
    expect(html).toContain("native@example.test");
    expect(html).toContain("运行调度中观测到 1 个账号");
    expect(html).toContain("渠道账号");
    expect(html).toContain("重新授权");
    expect(html).toContain("添加账号");
    expect(html).not.toContain("运行调度中未观测到账号");
    expect(html).not.toContain("连接账号");
    expect(html).not.toContain("核对绑定");
    client.clear();
  });
});
