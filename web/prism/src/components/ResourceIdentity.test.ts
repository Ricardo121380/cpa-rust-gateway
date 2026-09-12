import { createElement } from "react";
import { expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { IdentityDetails, ResourceIdentity, ResourceIdInput } from "./ResourceIdentity";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ResourcePicker } from "./ResourcePicker";

it("shows the observed name without legacy tooltip or generated code",()=>{
  const html=renderToStaticMarkup(createElement(ResourceIdentity,{id:"p12-06-codex-bridge-credential",kind:"account",name:"member@example.test"}));
  expect(html).toContain('>member@example.test</span>');
  expect(html).not.toContain('title=');expect(html).not.toContain('resource-code');
});
it("keeps an immutable form's exact target hidden while rendering its readable label",()=>{
  const html=renderToStaticMarkup(createElement(ResourceIdInput,{name:"id",value:"p12-06-codex-bridge-credential",kind:"account",displayName:"member@example.test",readOnly:true}));
  expect(html).toContain('type="hidden"');expect(html).toContain('name="id" value="p12-06-codex-bridge-credential"');
  expect(html).toContain('>member@example.test</span>');expect(html).not.toContain('type="text"');
});
it("renders references as resource names while preserving a deliberate internal-copy action",()=>{
  const html=renderToStaticMarkup(createElement(IdentityDetails,{entries:[["上游 ID","p12-12-production-grok-build-upstream","Grok Build"]]}));
  expect(html).toContain('关联信息');expect(html).toContain('>Grok Build</span>');
  expect(html).not.toContain('<code>p12');expect(html).toContain('复制内部引用');
});

it("keeps historical filtering available without requiring a current configuration",()=>{
  const client=new QueryClient();
  const html=renderToStaticMarkup(createElement(QueryClientProvider,{client},createElement(ResourcePicker,{kind:"upstream",name:"provider_id",defaultValue:"p12-old-upstream",allowCustom:true})));
  expect(html).toContain('value="p12-old-upstream"');expect(html).not.toContain('>p12-old-upstream<');
  expect(html).toContain('指定历史资源…');client.clear();
});
it("does not confuse a resource named like the history action with that action",()=>{
  const client=new QueryClient();
  const html=renderToStaticMarkup(createElement(QueryClientProvider,{client},createElement(ResourcePicker,{kind:"upstream",defaultValue:"__history_resource",allowCustom:true})));
  expect(html).toContain('value="__history_resource" selected=""');
  expect(html).toContain('value="__history_resource_" data-history-option="true"');client.clear();
});
