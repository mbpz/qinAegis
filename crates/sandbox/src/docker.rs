// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use std::process::Command;

pub fn docker_command(args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("docker")
        .args(args)
        .output()
        .map_err(|e| anyhow::anyhow!("docker command failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("docker {} failed: {}", args.join(" "), stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn is_container_running(name: &str) -> anyhow::Result<bool> {
    let output = docker_command(&["ps", "--filter", &format!("name={}", name), "--format", "{{.Names}}"])?;
    Ok(output.trim().split('\n').any(|n| n == name))
}

pub fn start_container(compose_file: &str) -> anyhow::Result<()> {
    let output = docker_command(&["compose", "-f", compose_file, "up", "-d"])?;
    if !output.is_empty() {
        println!("{}", output);
    }
    Ok(())
}

pub fn stop_container(compose_file: &str) -> anyhow::Result<()> {
    docker_command(&["compose", "-f", compose_file, "down"])?;
    Ok(())
}

pub fn get_container_ip(name: &str) -> anyhow::Result<String> {
    let output = docker_command(&[
        "inspect",
        "-f",
        "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}",
        name,
    ])?;
    Ok(output.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_container_running_returns_bool() {
        // If Docker is not running, this returns Err
        // If running but no container, returns Ok(false)
        let result = is_container_running("nonexistent-qinaegis-sandbox");
        assert!(result.is_ok());
    }
}