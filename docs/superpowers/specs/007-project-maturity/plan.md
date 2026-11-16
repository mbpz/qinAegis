# qinAegis 项目成熟度提升实施计划

<!-- product-status-2.0 -->
> **历史方案说明 · 2026-11-16**
>
> 历史成熟度阶段记录。许可证、锁文件、可测试性与项目记忆方向沿用；原文 CLI/TUI、发布完成度和任务勾选仅记录当时范围，不代表 2.0 验收或发布状态。
>
> 正文的状态、任务勾选、代码和验收结论保留为原始记录，不作为 2.0 开发指令或交付证明。当前以[产品记忆](../../../../CONTEXT.md)、[2.0 批准方案](../../../plans/local-web.md)及[实现进度与验收缺口](../../../progress/local-web-2.0.md)为准。
<!-- /product-status-2.0 -->

**Spec:** 007-project-maturity
**Status:** Completed
**Created:** 2026-05-06

---

## 实施策略

### 许可证选择：MIT License

选择原因：
- 简洁宽松，易于被社区接受
- 与 Homebrew 生态兼容良好
- 字节/开源社区常用

### 测试策略

- 使用 Rust 内置 `#[cfg(test)]` 模块
- 为核心模块添加单元测试
- Mock 外部依赖（storage trait, LLM client）
- 测试覆盖重点：storage/local.rs, executor.rs, generator.rs

### 实施顺序

1. 添加 LICENSE + 版权头
2. 生成 Cargo.lock
3. 创建 CONTEXT.md
4. 添加单元测试
5. 添加 CHANGELOG.md

---

## 关键文件

### 新增文件
- `LICENSE` - MIT License
- `CONTEXT.md` - 项目上下文
- `CHANGELOG.md` - 变更日志
- `**/copyright.rs` - 各源文件版权头（需修改）

### 修改文件
- `crates/*/src/*.rs` - 添加版权头
- `Cargo.lock` - 生成

---

## 技术决策

### D-001: 使用 MIT License
- 决策：采用 MIT 开源许可证
- 理由：简洁、宽松、与 Homebrew 生态兼容

### D-002: 测试框架选择
- 决策：使用 Rust 内置测试框架 + mockall
- 理由：轻量、无需额外依赖、内置支持好

---

_Next: Phase 3 - Task Breakdown_
