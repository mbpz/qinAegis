# Storage Trait — Abstracting Persistence

`LocalStorage` 是具体类型，所有调用方直接依赖，无法在不修改源码的情况下切换存储后端。决定引入 `Storage` trait 作为持久化的抽象接口。

支持 async 操作、Stream 批量返回、事务、认证凭证。云存储（S3/GCS）和本地文件系统都作为实现。
