# qinAegis 项目成熟度提升规范

<!-- product-status-2.0 -->
> **历史方案说明 · 2026-11-16**
>
> 历史成熟度阶段记录。许可证、锁文件、可测试性与项目记忆方向沿用；原文 CLI/TUI、发布完成度和任务勾选仅记录当时范围，不代表 2.0 验收或发布状态。
>
> 正文的状态、任务勾选、代码和验收结论保留为原始记录，不作为 2.0 开发指令或交付证明。当前以[产品记忆](../../../../CONTEXT.md)、[2.0 批准方案](../../../plans/local-web.md)及[实现进度与验收缺口](../../../progress/local-web-2.0.md)为准。
<!-- /product-status-2.0 -->

**Spec ID:** 007-project-maturity
**Status:** Completed
**Created:** 2026-05-06

---

## Summary

将 qinAegis 项目提升到可开源状态，解决以下关键问题：
1. 添加 LICENSE（MIT/Apache 2.0）
2. 添加单元测试覆盖
3. 添加 Cargo.lock 锁定依赖
4. 创建 CONTEXT.md 为 agent 提供项目上下文
5. 添加 CHANGELOG.md

---

## 背景分析

根据项目完成度评估：
- ✅ 核心架构完整（Rust Workspace）
- ✅ CLI + TUI 双模式
- ✅ CI/CD + Homebrew 发布
- ❌ 缺少 LICENSE
- ❌ 无单元测试
- ❌ 依赖版本未锁定
- ❌ 缺少 CONTEXT.md
- ❌ 缺少 CHANGELOG

---

## Requirements

### R-001: LICENSE 文件
- 选择开源许可证（MIT 或 Apache 2.0）
- 添加到项目根目录
- 所有源文件头部添加版权声明

### R-002: 单元测试覆盖
- 为核心模块添加测试
- 至少覆盖：storage, executor, generator, reporter
- 目标：关键业务逻辑有测试保障

### R-003: 依赖版本锁定
- 生成 Cargo.lock
- 提交到版本控制

### R-004: CONTEXT.md
- 为 AI agent 提供项目概述
- 包含项目结构、技术栈、关键模块说明
- 放在项目根目录

### R-005: CHANGELOG.md
- 记录版本变更历史
- 使用 Keep a Changelog 格式
- 从 v0.1.0 开始

---

## Out of Scope

- 不包含 Notion 集成相关代码
- 不改变现有架构
- 不添加集成测试（仅单元测试）

---

_Requirements will be refined in Phase 1._
