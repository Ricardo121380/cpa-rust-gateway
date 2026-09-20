import {expect,it} from "vitest";
import {assertChangePage,groupChanges,independentCatalogEvents,type ConfigurationChange} from "./pendingChanges";

it("keeps composite bindings distinct and preserves unknown resource kinds",()=>{
 const rows:ConfigurationChange[]=[{resource_kind:"credential_binding",resource_key:'["endpoint","account-a"]',change:"added",changed_fields:[]},{resource_kind:"credential_binding",resource_key:'["endpoint","account-b"]',change:"added",changed_fields:[]},{resource_kind:"future_kind",resource_key:"unknown",change:"changed",changed_fields:["future_field"]}];
 const grouped=groupChanges(rows);expect(grouped.get("提供商与账号")).toHaveLength(2);expect(grouped.get("其他资源")).toEqual([rows[2]]);
});
it("rejects any page whose identity or revision changed",()=>{
 const base={id:"active",revision:"rev-4"},target={id:"draft",revision:"rev-8"};
 const page={base,target,items:[],next_cursor:null};expect(()=>assertChangePage(page,base,target)).not.toThrow();
 expect(()=>assertChangePage({...page,target:{...target,revision:"rev-9"}},base,target)).toThrow("配置已变化");
 expect(()=>assertChangePage({...page,base:{...base,id:"other"}},base,target)).toThrow("配置已变化");
});
it("independent catalog evidence is exact-version filtered and retains string audit IDs",()=>{
 const row={id:"9007199254740993",action:"billing_catalog_imported",actor:"synthetic",config_version_id:"draft",resource_kind:"billing_catalog",resource_id:"catalog",occurred_at_ms:1};
 const events=independentCatalogEvents({items:[row,{...row,id:"9007199254740994",config_version_id:"other"},{...row,id:"9007199254740995",action:"access_group_created"}],next_before_id:null},"draft");
 expect(events).toEqual([row]);expect(events[0]?.id).toBe("9007199254740993");
});
