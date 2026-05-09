// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

//! CLI integration tests using trycmd
//!
//! Run with: cargo test --test trycmd_test

use std::path::Path;

#[test]
fn test_cli_help() {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap();

    let shim = project_root.join("target/release/qinAegis.shim");
    assert!(shim.exists(), "Shim not found at {}", shim.display());

    // Set PATH to use our shim
    let shim_dir = shim.parent().unwrap();
    std::env::set_var("PATH", format!("{}:{}", shim_dir.display(), std::env::var("PATH").unwrap_or_default()));

    trycmd::TestCases::new()
        .case("tests/cmd/qinAegis.trycmd")
        .run();
}