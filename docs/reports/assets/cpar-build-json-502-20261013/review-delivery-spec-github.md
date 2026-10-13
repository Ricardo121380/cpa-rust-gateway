# 收口 Spec 独立 GitHub 补核

来源 `/root/github_spec_closure` FINAL_ANSWER；聚焦初次PARTIAL唯一未核缺口，未声称重审43文件。

聚焦Spec补核PASS，0项契约不符合，覆盖3/3；补齐既有收口Spec的底层GitHub比较缺口。target=4104b5a373d50e53c2f57bfab67617d8debf43f7；指定10个公开文件逐字节等于该对象。A为本批assets，P为output/cpar-build-json-502-20261013/private。

| 票／既有评论ID | 实际历史字符 | SHA256 |
|---|---:|---|
| #47／6081215242 | 24298 | b30c0364784121924fe2b3b1cb151fbd089ab1c8c6db817c74efa6e6635334db |
| #57／6081234278 | 27999 | b518be523d0e5f6e6bc1de58bdeddbd435a1281d82ce704e418191b7877487bf |
| #9／6081235490 | 26738 | 53a13ebed15dbbbb465554fd7415d00cc03d9768d12c6f5fd6d49c37a8b97819 |

三票初始GH评论body＝preflight REST body＝before PATCH body，UTF-8字节精确相同。payload、PATCH、after GET、final GET、展开后的评论列表，body均精确等于旧历史＋对应MD全部字节；MD实含前置换行、分割线和真实换行。追加字符2357/2369/2376，结果字符26655/30368/29114；历史、追加、结果SHA及marker计数与全部公开回执吻合。普通marker旧历史0次、追加及最终各1次（A/github-append-plan.json:3,9,16,23；三个MD:5；A/github-readback.json:39,71,118）。

三票均ISSUE、OPEN、评论各1条；原正文/AC逐字节不变。preflight/readback的24个protected_fields完全一致；initial→preflight→final投影、字段存在性及投影SHA复算均吻合。preflight→final评论除body/updated_at外全部字段保持，包括完整user、created_at、html_url、node_id。

分页按“页列表→逐页记录”展开，三阶段关系、原序、受保护属性完全一致：#47八项16,27,29,30,31,32,33,34；#57二十三项12,17,21,25,26,37,38,40,41,42,43,44,45,46,47,48,49,50,52,53,54,55,56；#9零项。#57中#47仅updated_at改变，该字段不在保护投影中（P/github-final-readback/blocked-by-57.stdout:1）。

额外观察，不能宣称完整跨端点响应对象相等：三票PATCH/列表的user.avatar_url为https://avatars.githubusercontent.com/u/129812278?v=4；单GET含?u=b23b5046d4a65829ced048af8a79fe039670505a&v=4。这是唯一跨端点字段差异；同端点发送前→最终user完整保持。出处示例P/github-write/patch-47.stdout:1、P/github-final-readback/comments-47.stdout:1、comment-47.stdout:1；差异原因未查证。

授权行为为追加既有评论、保留历史并读回，三票继续OPEN（A/batch-contract.md:55-57）。未重审43文件、运行项目测试或真实请求；未联网、写文件。
