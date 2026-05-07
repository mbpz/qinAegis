// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::executor::TestResult;
use crate::performance::LighthouseResult;
use crate::stress::LocustResult;
use std::path::PathBuf;

pub struct Reporter;

impl Reporter {
    pub fn report_dir(run_id: &str) -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("qinAegis")
            .join("reports")
            .join(run_id)
    }

    pub fn save_summary(run_id: &str, results: &[TestResult]) -> anyhow::Result<PathBuf> {
        let dir = Self::report_dir(run_id);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("summary.json");

        let summary = serde_json::json!({
            "run_id": run_id,
            "total": results.len(),
            "passed": results.iter().filter(|r| r.passed).count(),
            "failed": results.iter().filter(|r| !r.passed).count(),
            "results": results,
        });

        std::fs::write(&path, serde_json::to_string_pretty(&summary)?)?;
        Ok(path)
    }

    /// Generate a self-contained HTML report.
    pub fn generate_run_report(
        project_name: &str,
        run_id: &str,
        results: &[TestResult],
        lighthouse: Option<&LighthouseResult>,
        locust: Option<&LocustResult>,
    ) -> anyhow::Result<PathBuf> {
        let dir = Self::report_dir(run_id);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("report.html");

        let passed = results.iter().filter(|r| r.passed).count();
        let failed = results.len() - passed;
        let total_duration: u64 = results.iter().map(|r| r.duration_ms).sum();
        let pass_rate = if results.is_empty() {
            100.0
        } else {
            (passed as f64 / results.len() as f64) * 100.0
        };

        let timestamp = chrono_lite_now();

        let mut html = String::new();
        html.push_str(&build_html_header(project_name, run_id, &timestamp));
        html.push_str(&build_css());
        html.push_str("</head><body>\n");
        html.push_str(&build_page_header(project_name, run_id, &timestamp));
        html.push_str(&build_summary_cards(passed, failed, total_duration, pass_rate));
        html.push_str(&build_result_table(results));
        html.push_str(&build_screenshot_gallery(results));

        if let Some(lh) = lighthouse {
            html.push_str(&build_lighthouse_section(lh));
        }
        if let Some(lc) = locust {
            html.push_str(&build_locust_section(lc));
        }

        html.push_str(&build_footer());
        html.push_str("</body></html>");

        std::fs::write(&path, &html)?;
        Ok(path)
    }
}

// ============================================================================
// HTML Builder Helpers
// ============================================================================

fn build_html_header(project: &str, run_id: &str, timestamp: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>qinAegis Report — {} — {}</title>
"#,
        project, run_id
    )
}

fn build_css() -> String {
    r#"<style>
:root {
  --bg: #0f1117;
  --card-bg: #1a1d27;
  --border: #2a2d3a;
  --text: #e1e4e8;
  --text-dim: #8b949e;
  --green: #3fb950;
  --red: #f85149;
  --yellow: #d29922;
  --blue: #58a6ff;
  --purple: #a371f7;
}
* { margin:0; padding:0; box-sizing:border-box; }
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif; background: var(--bg); color: var(--text); line-height: 1.6; padding: 24px; max-width: 1200px; margin: 0 auto; }
.header { text-align: center; padding: 32px 0; border-bottom: 1px solid var(--border); margin-bottom: 24px; }
.header h1 { font-size: 28px; font-weight: 700; color: var(--text); }
.header .meta { color: var(--text-dim); font-size: 14px; margin-top: 8px; }
.cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px; margin-bottom: 32px; }
.card { background: var(--card-bg); border: 1px solid var(--border); border-radius: 12px; padding: 20px; text-align: center; }
.card .value { font-size: 36px; font-weight: 800; }
.card .label { font-size: 12px; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.5px; margin-top: 4px; }
.card.pass .value { color: var(--green); }
.card.fail .value { color: var(--red); }
.card.duration .value { color: var(--blue); }
.card.rate .value { color: var(--purple); }
.results { margin-bottom: 32px; }
.results h2 { font-size: 20px; margin-bottom: 16px; padding-bottom: 8px; border-bottom: 1px solid var(--border); }
.result-row { background: var(--card-bg); border: 1px solid var(--border); border-radius: 8px; padding: 16px; margin-bottom: 12px; display: flex; align-items: center; gap: 16px; }
.result-row.pass { border-left: 4px solid var(--green); }
.result-row.fail { border-left: 4px solid var(--red); }
.result-row .badge { font-size: 12px; font-weight: 700; padding: 4px 12px; border-radius: 20px; text-transform: uppercase; }
.badge.pass { background: rgba(63,185,80,0.15); color: var(--green); }
.badge.fail { background: rgba(248,81,73,0.15); color: var(--red); }
.result-row .info { flex: 1; }
.result-row .case-id { font-weight: 600; font-size: 14px; }
.result-row .duration { font-size: 13px; color: var(--text-dim); margin-top: 2px; }
.result-row .error { font-size: 13px; color: var(--red); margin-top: 6px; padding: 8px 12px; background: rgba(248,81,73,0.08); border-radius: 6px; font-family: 'SF Mono', 'Fira Code', monospace; white-space: pre-wrap; word-break: break-word; }
.screenshots h2 { font-size: 20px; margin-bottom: 16px; padding-bottom: 8px; border-bottom: 1px solid var(--border); }
.screenshot-card { background: var(--card-bg); border: 1px solid var(--border); border-radius: 8px; padding: 16px; margin-bottom: 16px; }
.screenshot-card .caption { font-size: 13px; color: var(--text-dim); margin-bottom: 10px; }
.screenshot-card img { max-width: 100%; border-radius: 6px; border: 1px solid var(--border); }
.perf-section, .stress-section { margin-bottom: 32px; }
.perf-section h2, .stress-section h2 { font-size: 20px; margin-bottom: 16px; padding-bottom: 8px; border-bottom: 1px solid var(--border); }
.metrics-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 12px; }
.metric-card { background: var(--card-bg); border: 1px solid var(--border); border-radius: 8px; padding: 12px; text-align: center; }
.metric-card .metric-value { font-size: 24px; font-weight: 700; color: var(--blue); }
.metric-card .metric-label { font-size: 11px; color: var(--text-dim); text-transform: uppercase; margin-top: 4px; }
.metric-card.regressed .metric-value { color: var(--red); }
.footer { text-align: center; padding: 24px; color: var(--text-dim); font-size: 12px; border-top: 1px solid var(--border); margin-top: 32px; }
</style>
"#
    .to_string()
}

fn build_page_header(project: &str, run_id: &str, timestamp: &str) -> String {
    format!(
        r#"<div class="header">
<h1>🧪 qinAegis Test Report</h1>
<div class="meta">Project: <strong>{}</strong> &nbsp;|&nbsp; Run: <strong>{}</strong> &nbsp;|&nbsp; {}</div>
</div>
"#,
        project, run_id, timestamp
    )
}

fn build_summary_cards(passed: usize, failed: usize, total_duration: u64, pass_rate: f64) -> String {
    format!(
        r#"<div class="cards">
<div class="card pass"><div class="value">{}</div><div class="label">Passed</div></div>
<div class="card fail"><div class="value">{}</div><div class="label">Failed</div></div>
<div class="card rate"><div class="value">{:.0}%</div><div class="label">Pass Rate</div></div>
<div class="card duration"><div class="value">{:.1}s</div><div class="label">Duration</div></div>
</div>
"#,
        passed,
        failed,
        pass_rate,
        total_duration as f64 / 1000.0
    )
}

fn build_result_table(results: &[TestResult]) -> String {
    let mut html = String::from(r#"<div class="results"><h2>📋 Test Results</h2>"#);

    for r in results {
        let status_class = if r.passed { "pass" } else { "fail" };
        let badge_text = if r.passed { "PASS" } else { "FAIL" };

        html.push_str(&format!(
            r#"<div class="result-row {}">
<div class="badge {}">{}</div>
<div class="info">
<div class="case-id">{}</div>
<div class="duration">{}ms</div>
"#,
            status_class,
            status_class,
            badge_text,
            r.case_id,
            r.duration_ms,
        ));

        if !r.passed {
            if let Some(ref err) = r.error_message {
                html.push_str(&format!(
                    r#"<div class="error">{}</div>"#,
                    html_escape(err)
                ));
            }
        }

        html.push_str("</div></div>\n");
    }

    html.push_str("</div>\n");
    html
}

fn build_screenshot_gallery(results: &[TestResult]) -> String {
    let screenshots: Vec<&TestResult> = results
        .iter()
        .filter(|r| r.screenshot_base64.is_some())
        .collect();

    if screenshots.is_empty() {
        return String::new();
    }

    let mut html = String::from(r#"<div class="screenshots"><h2>📸 Screenshots</h2>"#);

    for r in screenshots {
        let b64 = r.screenshot_base64.as_ref().unwrap();
        let status_label = if r.passed { "✅ Pass" } else { "❌ Fail" };
        html.push_str(&format!(
            r#"<div class="screenshot-card">
<div class="caption">{} — {} ({}ms)</div>
<img src="data:image/png;base64,{}" alt="screenshot for {}" loading="lazy" />
</div>
"#,
            r.case_id,
            status_label,
            r.duration_ms,
            b64,
            html_escape(&r.case_id),
        ));
    }

    html.push_str("</div>\n");
    html
}

fn build_lighthouse_section(lh: &LighthouseResult) -> String {
    let mut html = String::from(r#"<div class="perf-section"><h2>⚡ Lighthouse Performance</h2><div class="metrics-grid">"#);

    let metrics = [
        ("Performance", lh.metrics.performance, ""),
        ("Accessibility", lh.metrics.accessibility, ""),
        ("Best Practices", lh.metrics.best_practices, ""),
        ("SEO", lh.metrics.seo, ""),
        ("FCP (ms)", lh.metrics.first_contentful_paint, "ms"),
        ("LCP (ms)", lh.metrics.largest_contentful_paint, "ms"),
        ("CLS", lh.metrics.cumulative_layout_shift, ""),
        ("TBT (ms)", lh.metrics.total_blocking_time, "ms"),
        ("Speed Index", lh.metrics.speed_index, "s"),
        ("TTFB (ms)", lh.metrics.ttfb, "ms"),
    ];

    for (label, value, unit) in &metrics {
        let display = if *unit == "ms" {
            format!("{:.0}{}", value, unit)
        } else if *unit == "s" {
            format!("{:.2}{}", value, unit)
        } else {
            format!("{:.0}", value * 100.0)
        };
        let regressed = if *label == "Performance" && *value < 0.8 { " regressed" } else { "" };
        html.push_str(&format!(
            r#"<div class="metric-card{}"><div class="metric-value">{}</div><div class="metric-label">{}</div></div>"#,
            regressed, display, label
        ));
    }

    html.push_str("</div></div>\n");
    html
}

fn build_locust_section(lc: &LocustResult) -> String {
    let error_rate = if lc.stats.total_requests > 0 {
        (lc.stats.total_failures as f64 / lc.stats.total_requests as f64) * 100.0
    } else {
        0.0
    };

    format!(
        r#"<div class="stress-section"><h2>🔥 Stress Test (Locust)</h2>
<div class="metrics-grid">
<div class="metric-card"><div class="metric-value">{}</div><div class="metric-label">Total Requests</div></div>
<div class="metric-card"><div class="metric-value">{}</div><div class="metric-label">Failures</div></div>
<div class="metric-card{}"><div class="metric-value">{:.1}%</div><div class="metric-label">Error Rate</div></div>
<div class="metric-card"><div class="metric-value">{:.0} ms</div><div class="metric-label">Avg Response</div></div>
<div class="metric-card"><div class="metric-value">{:.0} ms</div><div class="metric-label">P95 Response</div></div>
<div class="metric-card"><div class="metric-value">{:.1}</div><div class="metric-label">RPS</div></div>
</div></div>
"#,
        lc.stats.total_requests,
        lc.stats.total_failures,
        if error_rate > 5.0 { " regressed" } else { "" },
        error_rate,
        lc.stats.avg_response_time,
        lc.stats.p95_response_time,
        lc.stats.rps,
    )
}

fn build_footer() -> String {
    r#"<div class="footer">Generated by <strong>qinAegis</strong> — AI-powered quality engineering platform</div>
"#
    .to_string()
}

/// Simple HTML entity escape.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Simple ISO-ish format
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    format!("{:02}:{:02}:{:02} UTC", hours, minutes, seconds)
}
