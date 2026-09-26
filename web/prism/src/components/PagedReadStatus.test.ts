import {describe,it,expect} from "vitest";
import {InfiniteQueryObserver,QueryClient} from "@tanstack/react-query";
import {pagedRecovery} from "./PagedReadStatus";

describe("paged read recovery",()=>{
  it("retains loaded pages and retries only the failed cursor; refresh failure also retains the snapshot",async()=>{
    const client=new QueryClient({defaultOptions:{queries:{retry:false}}});
    let failure=false;
    const calls:number[]=[];
    const observer=new InfiniteQueryObserver(client,{queryKey:["m3-pages"],initialPageParam:0,queryFn:async({pageParam})=>{calls.push(pageParam);if(failure)throw {kind:"network"};return {rows:[pageParam],next:pageParam<2?pageParam+1:undefined};},getNextPageParam:page=>page.next});
    await observer.refetch();failure=true;
    const failed=await observer.fetchNextPage();
    expect(failed.data?.pages.map(page=>page.rows)).toEqual([[0]]);
    expect(pagedRecovery(failed.error,true,failed.isFetchNextPageError)).toMatchObject({title:"下一页读取失败",nextPage:true});
    failure=false;await observer.fetchNextPage();expect(calls).toEqual([0,1,1]);
    failure=true;const refresh=await observer.refetch();
    expect(refresh.data?.pages.map(page=>page.rows)).toEqual([[0],[1]]);
    expect(pagedRecovery(refresh.error,true,observer.getCurrentResult().isFetchNextPageError).title).toBe("刷新失败");
    client.clear();
  });
  it("restarts a changed snapshot and distinguishes initial failure",()=>{
    expect(pagedRecovery({kind:"conflict"},true,true)).toMatchObject({action:"从头重新读取",nextPage:false});
    expect(pagedRecovery({kind:"network"},false,false).title).toBe("读取失败");
  });
});
