# Storage Trait — Abstracting Persistence

<!-- product-status-2.0 -->
> **决策沿用与演进说明 · 2026-11-16**
>
> 决策状态：沿用存储抽象原则，后端能力按实现核验。2.0 聚焦单用户本地资产，契约治理、私有配置和运行证据各有持久化边界；原文 Stream、事务、认证及 S3/GCS 是抽象愿景，不表示当前已提供云存储、通用事务或远程协作。
>
> 正文的状态、任务勾选、代码和验收结论保留为原始记录，不作为 2.0 开发指令或交付证明。当前以[产品记忆](../../CONTEXT.md)、[2.0 批准方案](../plans/local-web.md)及[实现进度与验收缺口](../progress/local-web-2.0.md)为准。
<!-- /product-status-2.0 -->

`LocalStorage` 是具体类型，所有调用方直接依赖，无法在不修改源码的情况下切换存储后端。决定引入 `Storage` trait 作为持久化的抽象接口。

支持 async 操作、Stream 批量返回、事务、认证凭证。云存储（S3/GCS）和本地文件系统都作为实现。
