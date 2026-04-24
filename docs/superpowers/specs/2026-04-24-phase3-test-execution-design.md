# QinAegis Phase 3 Design: Test Execution + Notion Result Storage

> Version: v0.1
> Date: 2026-04-24
> Status: Approved

---

## 1. Overview

Phase 3 implements parallel test execution and batch result storage in Notion. It consumes the Phase 2 outputs (approved test cases in YAML) and produces: executed results, Midscene HTML reports, and updated Notion TestResults pages.

**Key decisions:**
- Parallel execution: up to 4 concurrent test cases
- Batch write: all results written to Notion only after all executions complete
- Dual report: Midscene HTML saved locally AND uploaded to Notion as attachment

---

## 2. Parallel Execution Architecture

```
qinAegis run --type smoke
       │
       ▼
NotionClient::query_test_cases(type = "smoke", status = "Approved")
       │
       ▼
Concurrent scheduler (tokio::spawn, MAX_CONCURRENCY = 4)
  ┌──────┬──────┬──────┬──────┐
  │ TC1  │ TC2  │ TC3  │ TC4  │
  └──────┴──────┴──────┴──────┘
       │
       │ each: executor.ts::run_yaml(yaml_script)
       │
       ▼
Vec<TestResult> { passed, duration_ms, screenshot_base64, report_path }
       │
       ▼
Batch write to Notion TestResults DB (all at once)
       │
       ▼
Midscene HTML Report: local archive + Notion file upload
```

### 2.1 Concurrency Control

- `MAX_CONCURRENCY = 4` (configurable via `config.toml`)
- steel-browser supports multiple CDP sessions on same port
- Each test case gets a dedicated `page` object
- Semaphore used to limit concurrent executions

### 2.2 Result Collection

All `TestResult` structs collected before any Notion write:
```rust
struct TestResult {
    case_id: String,
    status: TestStatus, // Passed | Failed | Skipped | Error
    duration_ms: u64,
    screenshot_base64: Option<String>,
    error_message: Option<String>,
    report_local_path: Option<String>,
}
```

---

## 3. Notion Batch Write

After all executions complete:

```rust
pub async fn write_results_batch(
    client: &NotionClient,
    results: Vec<TestResult>,
    run_id: &str,
) -> anyhow::Result<()> {
    for result in results {
        create_test_result_page(client, result, run_id).await?;
    }
    Ok(())
}
```

### 3.1 TestResult Page Fields

| Notion Field | Value |
|---|---|
| name | `TC-{id}-{run_id}` |
| test_case | Relation → TestCases |
| status | Passed / Failed / Skipped / Error |
| duration_ms | milliseconds |
| run_at | current timestamp |
| report_url | Notion file URL (after upload) |
| error_message | error text if failed |
| retry_count | 0 (first run) |

### 3.2 Notion File Upload

Notion supports file attachments via `POST /v1/pages/{page_id}/attachments`.
Upload flow:
1. Save Midscene HTML report locally first
2. Use Notion API to attach file to TestResult page
3. Store returned `file_url` in `report_url` property

---

## 4. Midscene Report Dual Write

```
Midscene HTML Report (auto-generated at ./midscene_run/report/)
       │
       ├─→ Copy to: ~/.local/share/qinAegis/reports/{run_id}/{tc_id}.html
       │
       └─→ Upload to: Notion TestResult page as attachment
```

### 4.1 Report Path Convention

```
~/.local/share/qinAegis/reports/
└── {run_id}/
    ├── {tc_id_1}.html
    ├── {tc_id_2}.html
    └── summary.json   ← run summary with pass/fail counts
```

---

## 5. Module Breakdown

### `crates/core/src/executor.rs` (new file)
- `TestExecutor::new()` — spawns midscene process
- `TestExecutor::run_parallel(cases, maxConcurrency)` — parallel execution
- `run_yaml(yaml)` — send `run_yaml` JSON-RPC to executor.ts, return `TestResult`
- Semaphore-based concurrency limiting

### `crates/core/src/reporter.rs` (new file)
- `save_report_local(path, html)` — write to local reports dir
- `upload_report_to_notion(client, page_id, local_path)` — Notion file upload
- `generate_summary(results, run_id)` — write `summary.json`

### `crates/notion/src/writer.rs` (new file)
- `write_test_result(client, result, run_id)` — create TestResult page
- `upload_file_attachment(client, page_id, file_path)` — Notion file attachment API
- `batch_write_results(client, results, run_id)` — sequential batch

### `sandbox/src/executor.ts` (modify)
- Add `run_yaml` method:
  ```typescript
  case 'run_yaml': {
    const [yamlScript] = req.args as [string];
    const result = await runMidsceneYaml(yamlScript);
    return { id: req.id, ok: true, data: result };
  }
  ```
  Where `runMidsceneYaml` parses the YAML and executes step by step.

---

## 6. CLI Command

```bash
qinAegis run [smoke|functional|performance|stress]
  --project PROJECT_ID    # Notion project page ID
  --type TYPE             # smoke | functional | performance | stress
  --concurrency N         # max parallel (default 4)
```

---

## 7. Error Handling

| Scenario | Strategy |
|---|---|
| Any test case fails | Continue others, mark failed in results, don't abort batch |
| All fail | Still write all results to Notion, show summary |
| Notion API error on write | Retry individual page 3x, log failures, continue |
| Midscene YAML parse error | Mark as Error status, save error message |
| CDP connection lost | Restart browser session for that task |

---

## 8. Configuration

`~/.config/qinAegis/config.toml` additions:

```toml
[execution]
max_concurrency = 4
report_local_dir = "~/.local/share/qinAegis/reports"

[notion]
test_results_db_id = "xxx"   # from Phase 1 init
```

---

## 9. Data Flow Summary

```
Phase 2 output: TestCases (Approved, YAML stored in Notion)
       │
       ▼
executor.rs: run_parallel()
  → N × executor.ts::run_yaml(yaml)
  → Vec<TestResult>
       │
       ▼
reporter.rs: save_reports_local()
  → ~/.local/share/qinAegis/reports/{run_id}/*.html
       │
       ▼
notion/writer.rs: batch_write_results()
  → Notion TestResults DB pages
  → Notion file attachments (HTML reports)
```

---

*Last updated: 2026-04-24*
