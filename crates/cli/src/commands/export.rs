// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use qin_aegis_core::storage::LocalStorage;
use std::path::PathBuf;

pub async fn export_project(name: &str, format: &str) -> anyhow::Result<PathBuf> {
    let projects = LocalStorage::list_projects()?;
    if !projects.contains(&name.to_string()) {
        anyhow::bail!("Project '{}' does not exist", name);
    }

    let project_dir = LocalStorage::project_dir(name);
    let output_path = match format {
        "html" => export_html(name, &project_dir)?,
        "md" => export_markdown(name, &project_dir)?,
        "json" => export_json(name, &project_dir)?,
        _ => anyhow::bail!("Unknown format '{}'. Use html, md, or json.", format),
    };

    println!("✓ Exported to: {}", output_path.display());
    Ok(output_path)
}

fn export_html(name: &str, _project_dir: &std::path::Path) -> anyhow::Result<PathBuf> {
    let spec_path = LocalStorage::project_spec_path(name);
    let spec = if spec_path.exists() {
        std::fs::read_to_string(&spec_path)?
    } else {
        "# No spec found".to_string()
    };

    let cases = LocalStorage::load_cases(name)?;

    let mut cases_html = String::new();
    for case in &cases {
        cases_html.push_str(&format!(
            "<li><strong>{}</strong> ({}) - {}</li>\n",
            case.name, case.test_type, case.priority
        ));
    }

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>QinAegis - {}</title></head>
<body>
<h1>Project: {}</h1>
<h2>Spec</h2>
<pre>{}</pre>
<h2>Test Cases ({})</h2>
<ul>{}</ul>
</body>
</html>"#,
        name, name, spec, cases.len(), cases_html
    );

    let output_path = LocalStorage::base_path().join(format!("{}-export.html", name));
    std::fs::write(&output_path, html)?;
    Ok(output_path)
}

fn export_markdown(name: &str, _project_dir: &std::path::Path) -> anyhow::Result<PathBuf> {
    let mut md = String::new();

    // Spec
    let spec_path = LocalStorage::project_spec_path(name);
    if spec_path.exists() {
        md.push_str("# Project Spec\n\n");
        md.push_str(&std::fs::read_to_string(&spec_path)?);
        md.push_str("\n\n");
    }

    // Cases
    let cases = LocalStorage::load_cases(name)?;
    md.push_str(&format!("# Test Cases ({})\n\n", cases.len()));
    for case in &cases {
        md.push_str(&format!("## {} ({})\n\n", case.name, case.test_type));
        md.push_str("```yaml\n");
        md.push_str(&case.yaml_script);
        md.push_str("\n```\n\n");
    }

    let output_path = LocalStorage::base_path().join(format!("{}-export.md", name));
    std::fs::write(&output_path, md)?;
    Ok(output_path)
}

fn export_json(name: &str, _project_dir: &std::path::Path) -> anyhow::Result<PathBuf> {
    let config = LocalStorage::load_project(name)?;
    let cases = LocalStorage::load_cases(name)?;

    let export = serde_json::json!({
        "project": config,
        "cases": cases,
    });

    let output_path = LocalStorage::base_path().join(format!("{}-export.json", name));
    std::fs::write(&output_path, serde_json::to_string_pretty(&export)?)?;
    Ok(output_path)
}
