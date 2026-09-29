import {it,expect} from "vitest";
import {primaryAccountStatus} from "./accountStatus";
it("keeps operator disable intent ahead of runtime problems",()=>{expect(primaryAccountStatus({enabled:false,auth:"expired",runtime:["unauthorized"],connections:1}).label).toBe("已停用");});
it("labels partial binding failures without declaring every protocol blocked",()=>{expect(primaryAccountStatus({enabled:true,auth:"active",runtime:["available","unauthorized","cooling"],connections:3}).label).toBe("调用被拒绝 · 1/3 个连接");});
it("never treats absent evidence as healthy or quota zero",()=>{expect(primaryAccountStatus({enabled:true,auth:"active",runtime:[],connections:1}).label).toBe("运行状态未观测");expect(primaryAccountStatus({enabled:true,auth:"active",runtime:["available"],connections:1,readFailed:true}).label).toBe("运行观测读取失败");});
it("separates a global serving block and leaves available credentials unverified",()=>{expect(primaryAccountStatus({enabled:true,auth:"active",runtime:["available"],connections:1,acceptingRequests:false}).label).toBe("服务运行配置待恢复");expect(primaryAccountStatus({enabled:true,auth:"active",runtime:["available"],connections:1}).label).toBe("已配置，调用未验证");});
