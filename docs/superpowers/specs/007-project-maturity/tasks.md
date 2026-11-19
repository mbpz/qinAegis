# Tasks - qinAegis 项目成熟度提升

<!-- product-status-2.0 -->
> **历史方案说明 · 2026-11-16**
>
> 历史成熟度阶段记录。许可证、锁文件、可测试性与项目记忆方向沿用；原文 CLI/TUI、发布完成度和任务勾选仅记录当时范围，不代表 2.0 验收或发布状态。
>
> 正文的状态、任务勾选、代码和验收结论保留为原始记录，不作为 2.0 开发指令或交付证明。当前以[产品记忆](../../../../CONTEXT.md)、[2.0 批准方案](../../../plans/local-web.md)及[实现进度与验收缺口](../../../progress/local-web-2.0.md)为准。
<!-- /product-status-2.0 -->

**Spec:** 007-project-maturity
**Status:** In Progress

---

## T-001: 添加 MIT LICENSE
**Status:** COMPLETED
**Priority:** P0
**Module:** project-root

**Task:**
1. 创建 `LICENSE` 文件，使用 MIT License 文本
2. 版权年份：2026
3. 版权所有者：QinAegis Team

**验收标准：**
- ✅ LICENSE 文件存在于项目根目录
- ✅ 内容为标准 MIT License
- ✅ 包含 2026 年份和 QinAegis Team

---

## T-002: 添加源文件版权头
**Status:** COMPLETED
**Priority:** P0
**Module:** all-rust-files

**Task:**
1. 为所有 `.rs` 文件添加版权头
2. 格式：
```rust
// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

// [原有代码...]
```

**涉及文件：**
- `crates/cli/src/**/*.rs`
- `crates/core/src/**/*.rs`
- `crates/sandbox/src/**/*.rs`

**验收标准：**
- ✅ 所有 .rs 文件包含版权头
- ✅ 编译无错误

---

## T-003: 生成 Cargo.lock
**Status:** COMPLETED
**Priority:** P1
**Module:** project-root

**Task:**
1. 运行 `cargo generate-lockfile`
2. 确保 Cargo.lock 存在
3. 验证 lockfile 内容完整

**验收标准：**
- ✅ `Cargo.lock` 存在于项目根目录
- ✅ 包含所有 workspace 依赖

---

## T-004: 创建 CONTEXT.md
**Status:** COMPLETED
**Priority:** P1
**Module:** project-root

**Task:**
1. 创建 `CONTEXT.md` 文件
2. 内容包含：
   - 项目概述
   - 技术栈
   - 目录结构
   - 关键模块说明
   - 命令参考

**验收标准：**
- ✅ `CONTEXT.md` 存在于项目根目录
- ✅ 内容完整、结构清晰

---

## T-005: 添加 storage 模块测试
**Status:** SELECTED
**Priority:** P2
**Module:** crates/core/src/storage

**Task:**
1. 为 `storage/local.rs` 添加单元测试
2. 测试内容：
   - 配置读写
   - 项目 CRUD 操作
   - 路径处理

**验收标准：**
- `cargo test` 通过 storage 相关测试
- 测试覆盖率 > 70%

---

## T-006: 添加 generator 模块测试
**Status:** SELECTED
**Priority:** P2
**Module:** crates/core/src/generator

**Task:**
1. 为 `generator.rs` 添加单元测试
2. 测试内容：
   - Prompt 生成逻辑
   - 测试用例格式验证

**验收标准：**
- `cargo test` 通过 generator 相关测试

---

## T-007: 添加 executor 模块测试
**Status:** SELECTED
**Priority:** P2
**Module:** crates/core/src/executor

**Task:**
1. 为 `executor.rs` 添加单元测试
2. 测试内容：
   - 测试用例加载
   - 结果序列化

**验收标准：**
- `cargo test` 通过 executor 相关测试

---

## T-008: 创建 CHANGELOG.md
**Status:** SELECTED
**Priority:** P2
**Module:** project-root

**Task:**
1. 创建 `CHANGELOG.md`
2. 使用 Keep a Changelog 格式
3. 初始版本：v0.1.0 (2026-04)

**验收标准：**
- `CHANGELOG.md` 符合 Keep a Changelog 格式
- 包含 v0.1.0 条目

---

## 任务统计

| 状态 | 数量 |
|------|------|
| SELECTED | 8 |
| COMPLETED | 0 |
| DEFERRED | 0 |

---

## 实施顺序

1. T-001: LICENSE（先做）
2. T-002: 版权头（并行）
3. T-003: Cargo.lock（紧随）
4. T-004: CONTEXT.md
5. T-005~T-007: 单元测试
6. T-008: CHANGELOG（最后）

---

_Last Updated: 2026-05-06_
