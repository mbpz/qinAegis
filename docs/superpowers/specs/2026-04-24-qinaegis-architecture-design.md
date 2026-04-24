# QinAegis Architecture Design

> Version: v0.1
> Date: 2026-04-24
> Status: Draft

---

## 1. Overview

QinAegis is an AI-powered E2E testing platform with a Rust TUI frontend, visual AI-driven execution engine, and Notion as the data backend.

**Product pillars:**
- **TUI client** (macOS) via `brew install qinAegis`
- **Fully local sandbox** — browser automation runs in Docker (`steel-browser`)
- **AI-driven** — visual LLMs understand UI, generate and execute tests without CSS selectors
- **Notion-centric** — all data (projects, requirements, test cases, results) lives in Notion

---

## 2. Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                      Rust TUI (ratatui)                         │
│  ┌──────────┐  ┌──────────┐  ┌────────┐  ┌──────────────────┐   │
│  │ OAuth2   │  │ Notion    │  │ Config │  │  TUI Screens    │   │
│  │ Server   │  │ Client    │  │ Store  │  │  (Dashboard,    │   │
│  │ (axum)   │  │ (reqwest) │  │(toml)  │  │   InitWizard,   │   │
│  └────┬─────┘  └────┬─────┘  └───┬────┘  │   ConfigForm)   │   │
│       │             │            │       └──────────────────┘   │
│       └─────────────┴────────────┴─────────────────────────────│
│                              │                                  │
│                   ┌──────────┴──────────┐                      │
│                   │  Core Task Runner   │                      │
│                   │   (tokio async)    │                      │
│                   └──────────┬──────────┘                      │
│                              │                                  │
│               ┌──────────────┼──────────────┐                   │
│               │              │              │                  │
│       ┌───────┴───────┐  ┌────┴────┐  ┌─────┴─────┐             │
│       │ Explorer Task │  │Executor │  │Lighthouse │             │
│       │               │  │  Task   │  │   Task    │             │
│       └───────┬───────┘  └────┬────┘  └─────┬─────┘             │
│               │               │            │                   │
│               └───────────────┼────────────┘                   │
│                               │                                │
│                    ┌──────────┴──────────┐                     │
│                    │  Midscene Runner    │                     │
│                    │ (mlua + Node.js)   │                     │
│                    └──────────┬──────────┘                     │
│                               │                                │
│                    ┌──────────┴──────────┐                     │
│                    │  CDP WebSocket      │                     │
│                    │  ws://localhost:9222│                     │
│                    └──────────┬──────────┘                     │
│                               │                                │
│                               ▼                                │
│              ┌────────────────────────────────┐               │
│              │      steel-browser             │               │
│              │      (Docker container)        │               │
│              └────────────────────────────────┘               │
│                                                                 │
│  ┌──────────────┐                                               │
│  │  MiniMax VL  │◄──────── HTTPS (reqwest)                      │
│  └──────────────┘                                               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ Notion API (HTTPS)
                              ▼
                      ┌──────────────────┐
                      │     Notion       │
                      │  (4 Databases)   │
                      └──────────────────┘
```

---

## 3. Module Breakdown

### 3.1 `crates/cli` — TUI Entry Point

**Responsibilities:**
- User interaction (ratatui-based terminal UI)
- OAuth2 local HTTP server (receives Notion callback on `localhost:54321`)
- Command dispatch (`init`, `explore`, `generate`, `run`, `report`)
- TTY input handling and layout rendering

**Key components:**

| Component | Purpose |
|---|---|
| `main.rs` | Entry point, clap argument parsing |
| `tui/app.rs` | Root app state, event loop |
| `tui/screens/` | Dashboard, InitWizard, ProjectList, ConfigForm, TestResultView |
| `commands/init.rs` | OAuth2 flow trigger + Notion workspace init wizard |
| `commands/explore.rs` | Project exploration command |
| `commands/generate.rs` | Test case generation command |
| `commands/run.rs` | Test execution command |
| `config.rs` | `~/.config/qinAegis/config.toml` read/write |

### 3.2 `crates/notion` — Notion API Layer

**Responsibilities:**
- Notion OAuth2 token exchange + refresh
- macOS Keychain storage for tokens (`keyring` crate)
- Four-database creation wizard (interactive)
- Page and Database CRUD

**Key components:**

| Component | Purpose |
|---|---|
| `lib.rs` | Public API re-exports |
| `auth.rs` | OAuth2 flow, token storage/retrieval |
| `database.rs` | Database schema creation, query, update |
| `writer.rs` | Test result pages, file attachments |
| `models.rs` | Notion API request/response types |

**Notion Database Schema (4-dimension model):**

```
Projects ──1:N── Requirements ──1:N── TestCases ──1:N── TestResults
```

| Database | Key Properties |
|---|---|
| Projects | name (Title), url, tech_stack, status, spec_page (Relation) |
| Requirements | name (Title), project (Relation→Projects), description, priority, status |
| TestCases | name (Title), requirement (Relation), type, priority, status, yaml_script (Code), expected_result |
| TestResults | name (Title), test_case (Relation), status, duration_ms, run_at, report_url, error_message |

### 3.3 `crates/sandbox` — Browser Sandbox Manager

**Responsibilities:**
- steel-browser Docker container lifecycle (start, health-check, stop)
- CDP WebSocket connection pooling (multiple concurrent sessions)
- Session management (create, pause, resume, destroy)

**Key components:**

| Component | Purpose |
|---|---|
| `lib.rs` | Public API |
| `steel.rs` | steel-browser REST API client (`http::Client`) |
| `docker.rs` | Container lifecycle via Docker CLI |
| `health.rs` | Health check polling, readiness detection |

**Container policy:** Long-running — started on `qinAegis init` and kept alive. `docker start` wakes it if stopped.

### 3.4 `crates/core` — Business Logic

**Responsibilities:**
- LLM client (reqwest → MiniMax VL API)
- Midscene task execution via embedded Node.js (`mlua`)
- Report parsing (Midscene HTML → structured result)
- Test case generation (LLM prompt engineering)

**Key components:**

| Component | Purpose |
|---|---|
| `lib.rs` | Public API |
| `llm.rs` | MiniMax VL API client, prompt templates |
| `explorer.rs` | Project exploration (calls Midscene runner) |
| `generator.rs` | Test case YAML generation |
| `executor.rs` | Midscene YAML execution via mlua |
| `reporter.rs` | Midscene HTML report parsing + Notion upload |

### 3.5 `sandbox/` — Node.js Midscene Layer

**Note:** This is embedded into Rust via the `mlua` crate. The Node.js scripts are compiled to Lua-compatible scripts or called via `mlua` FFI.

**Scripts (loaded at runtime):**

| Script | Purpose |
|---|---|
| `explorer.ts` | `aiQuery` page structure extraction |
| `executor.ts` | Midscene YAML execution (`aiAct`, `aiQuery`, `aiAssert`) |
| `lighthouse.ts` | Lighthouse CI performance measurement |

---

## 4. Process Communication: mlua Embedding

Node.js is embedded into Rust via the `mlua` crate. No subprocess overhead.

```rust
// core/src/executor.rs
use mlua::Lua;

let lua = Lua::new();
lua.globals().set("cdp_ws_url", "ws://localhost:9222")?;
lua.globals().set("model_config", model_config)?;

let midscene_script = include_str!("../node_scripts/executor.lua");
lua.load(midscene_script).exec()?;

let result: Value = lua.globals().get("ai_act")?;
```

**Alternative:** If `mlua` embedding proves too restrictive for Midscene's Node.js dependencies, fall back to:
1. Node.js subprocess with stdin/stdout JSON
2. Unix Domain Socket for bidirectional communication

---

## 5. Configuration Storage

| Content | Storage |
|---|---|
| LLM API Key | macOS Keychain (`keyring` crate) |
| Notion OAuth Token | macOS Keychain |
| Notion Workspace ID | `~/.config/qinAegis/config.toml` |
| LLM Provider/Model | `~/.config/qinAegis/config.toml` |
| Sandbox compose path | `~/.config/qinAegis/config.toml` |
| Steel/CDP ports | `~/.config/qinAegis/config.toml` |

**`~/.config/qinAegis/config.toml` (example):**

```toml
[llm]
provider = "minimax"
base_url = "https://api.minimax.chat/v1"
model = "MiniMax-VL-01"

[sandbox]
compose_file = "~/.config/qinAegis/docker-compose.sandbox.yml"
steel_port = 3333
cdp_port = 9222

[notion]
workspace_id = "xxx"

[oauth]
redirect_port = 54321
```

---

## 6. Error Handling

| Scenario | Strategy |
|---|---|
| Sandbox connection failure | Auto-retry 3×, 2s interval, then surface user-friendly error |
| MiniMax API failure | Pass through error code + user-friendly message |
| Notion API failure (429/5xx) | Retry 3× with exponential backoff |
| Lua script error | Catch `mlua::Error` → unified `TaskError` enum |
| Docker not running | Prompt user to install Docker / start Docker Desktop |

---

## 7. Development Phases

### Phase 1A (Parallel with 1B)
- OAuth2 local HTTP server (axum)
- Notion client + token storage (Keychain)
- Notion 4-database creation wizard (interactive)

### Phase 1B (Parallel with 1A)
- steel-browser Docker compose setup
- CDP WebSocket connection test
- MiniMax VL API connectivity verification
- Midscene integration test

### Phase 2
- mlua embedding of Midscene runner
- UDS communication fallback if needed
- End-to-end Midscene YAML execution

### Phase 3
- Project exploration pipeline
- Test case generation + Notion write
- AI Critic review logic

### Phase 4
- k6 stress testing scripts
- Lighthouse CI integration
- Report parsing + Notion upload

### Phase 5
- TUI polish (Dashboard, error states, loading states)
- Homebrew tap + GitHub Actions CI/CD
- README + documentation

---

## 8. Key Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Browser sandbox | steel-browser (Docker) | Open-source, production-ready, replaces 2 weeks of mitmproxy+CDP work |
| AI execution engine | Midscene.js | Visual-only, no CSS selectors, MIT licensed, ByteDance production use |
| LLM model | MiniMax VL (vision-capable) | Must be vision model; text-only models cannot interpret screenshots |
| Process embedding | mlua → Node.js | Zero IPC overhead, shares tokio runtime |
| Sandbox lifecycle | Long-running | Faster test execution, no cold-start delay |
| Notion DB init | Interactive wizard | User confirms database names before creation |
| OAuth callback server | Rust axum (native) | No external process dependency |
| Test case format | Midscene YAML | Natural language, no selectors, AI-generatable |
| Distribution | Homebrew tap | Standard macOS distribution, matches CLI tool conventions |

---

## 9. Directory Structure

```
qinAegis/
├── Cargo.toml
├── Cargo.lock
│
├── crates/
│   ├── cli/
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── tui/
│   │   │   │   ├── app.rs
│   │   │   │   ├── dashboard.rs
│   │   │   │   ├── init_wizard.rs
│   │   │   │   ├── config_form.rs
│   │   │   │   └── project_list.rs
│   │   │   ├── commands/
│   │   │   │   ├── init.rs
│   │   │   │   ├── explore.rs
│   │   │   │   ├── generate.rs
│   │   │   │   └── run.rs
│   │   │   └── config.rs
│   │   └── Cargo.toml
│   │
│   ├── notion/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── auth.rs
│   │   │   ├── database.rs
│   │   │   ├── writer.rs
│   │   │   └── models.rs
│   │   └── Cargo.toml
│   │
│   ├── sandbox/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── steel.rs
│   │   │   ├── docker.rs
│   │   │   └── health.rs
│   │   └── Cargo.toml
│   │
│   └── core/
│       ├── src/
│       │   ├── lib.rs
│       │   ├── llm.rs
│       │   ├── explorer.rs
│       │   ├── generator.rs
│       │   ├── executor.rs
│       │   └── reporter.rs
│       └── Cargo.toml
│
├── sandbox/                    # Node.js Midscene scripts
│   ├── package.json
│   ├── tsconfig.json
│   └── src/
│       ├── explorer.ts
│       ├── executor.ts
│       ├── lighthouse.ts
│       └── k6-generator.ts
│
├── docker/
│   ├── docker-compose.sandbox.yml
│   └── Dockerfile.sandbox
│
├── Formula/
│   └── qinAegis.rb
│
├── .github/
│   └── workflows/
│       └── release.yml
│
└── docs/
    └── superpowers/
        └── specs/
            └── 2026-04-24-qinaegis-architecture-design.md
```

---

## 10. Dependencies Summary

### Rust Crates

| Crate | Version | Purpose |
|---|---|---|
| ratatui | 0.27+ | TUI framework |
| tokio | 1.x | Async runtime |
| reqwest | 0.12+ | HTTP client |
| serde / serde_json | 1.x | Serialization |
| keyring | 3.x | macOS Keychain |
| axum | 0.7+ | Local OAuth2 HTTP server |
| clap | 5.x | CLI argument parsing |
| mlua | 0.9+ | Lua/Node.js embedding |
| tokio-postgres | 0.7+ | (reserved) |
| thiserror | 2.x | Error enum derivation |

### Node.js Packages (sandbox layer)

| Package | Purpose |
|---|---|
| @midscene/web | AI execution engine |
| playwright | Browser automation |
| lighthouse | Performance testing |
| k6 | Stress testing |

---

*Last updated: 2026-04-24*
