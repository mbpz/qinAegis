# Local Storage Only - Remove Notion Dependency

**Date:** 2026-04-27
**Status:** Approved

## Overview

Remove Notion as the default storage backend. Replace with local file-based storage under `~/.qinAegis/projects/`. Add `qinAegis export` command for HTML/MD/JSON export.

## Storage Structure

```
~/.qinAegis/
└── projects/
    └── <project-name>/
        ├── config.yaml          # 项目配置（URL、技术栈）
        ├── spec.md              # explore 结果（Markdown）
        ├── requirements/        # generate 生成的需求文档
        │   └── <req-id>.md
        ├── cases/               # 测试用例（JSON）
        │   └── <case-id>.json
        └── reports/             # 测试执行结果
            └── <run-id>/
                ├── summary.json  # 本次运行汇总
                └── <case-id>.html # 每个用例的详细报告
```

## Commands

### `qinAegis init`
- 跳过 Notion OAuth
- 创建 `~/.qinAegis/config.yaml`
- 提示用户添加第一个项目

### `qinAegis project add <name> [--url <url>]`
- 创建 `~/.qinAegis/projects/<name>/`
- 初始化 `config.yaml`

### `qinAegis export --project <name> [--format html|md|json]`
- 导出项目的所有数据
- `--format html`: 生成单页 HTML 报告（包含 spec + cases + latest report）
- `--format md`: 生成 Markdown 压缩包
- `--format json`: 导出原始 JSON 数据

### Modified Commands

#### `qinAegis explore <project> [--url <url>] [--depth <n>]`
- 结果写入 `<project>/spec.md`
- 不再写入 Notion

#### `qinAegis generate <project> --requirements <file>`
- 测试用例写入 `<project>/cases/*.json`
- 不再写入 Notion

#### `qinAegis run <project> [--type smoke|full|perf|stress]`
- 结果写入 `<project>/reports/<run-id>/`
- Reporter 已有的 HTML 导出逻辑保持不变

## Code Changes

### Delete
- `crates/notion/` 整个目录

### Modify
- `crates/cli/src/commands/mod.rs` - 移除 notion 命令
- `crates/cli/src/commands/init.rs` - 移除 Notion OAuth
- `crates/cli/src/commands/explore.rs` - 写入本地文件
- `crates/cli/src/commands/generate.rs` - 写入本地文件
- `crates/core/src/reporter.rs` - 已有本地逻辑，扩展导出功能
- `Cargo.toml` - 移除 notion workspace member

### Add
- `crates/cli/src/commands/export.rs` - 新增 export 命令
- `crates/cli/src/commands/project.rs` - 项目管理命令
- `crates/core/src/storage.rs` - 本地存储抽象

## Data Models

### ProjectConfig (config.yaml)
```yaml
name: <project-name>
url: <target-url>
tech_stack: []
created_at: <timestamp>
```

### TestCase (cases/<id>.json)
```json
{
  "id": "<case-id>",
  "name": "<test-name>",
  "requirement_id": "<req-id>",
  "type": "smoke|full|perf|stress",
  "steps": [...],
  "expected": "...",
  "created_at": "<timestamp>"
}
```

### TestResult (reports/<run-id>/summary.json)
```json
{
  "run_id": "<run-id>",
  "project": "<project-name>",
  "type": "<type>",
  "total": 10,
  "passed": 9,
  "failed": 1,
  "duration_ms": 12345,
  "cases": [...]
}
```

## Implementation Order

1. 删除 `crates/notion`
2. 创建 `crates/core/src/storage.rs` 本地存储抽象
3. 修改 `init` 命令（移除 Notion OAuth）
4. 创建 `project` 命令（add/list/remove）
5. 修改 `explore` 命令（写入本地 spec.md）
6. 修改 `generate` 命令（写入本地 cases/）
7. 创建 `export` 命令
8. 清理 Cargo.toml 和其他引用