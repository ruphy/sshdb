use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

use crate::model::{Config, Host};

pub fn build_command(
    host: &Host,
    config: &Config,
    default_key: Option<&str>,
    extra_command: Option<&str>,
) -> Result<Command> {
    let mut cmd = Command::new("ssh");

    if let Some(bastion_name) = &host.bastion {
        let bastion_str = build_bastion_string(config, bastion_name, default_key, &mut Vec::new())?;
        cmd.arg("-J").arg(bastion_str);
    }

    if let Some(port) = host.port {
        cmd.arg("-p").arg(port.to_string());
    }

    if let Some(key) = select_key(host.key_path.as_deref(), default_key) {
        cmd.arg("-i").arg(key);
    }

    for opt in &host.options {
        cmd.arg(opt);
    }

    let target = if let Some(user) = &host.user {
        format!("{user}@{}", host.address)
    } else {
        host.address.clone()
    };
    cmd.arg(target);

    if let Some(extra) = extra_command {
        cmd.arg(extra);
    } else if let Some(remote) = &host.remote_command {
        cmd.arg(remote);
    }

    Ok(cmd)
}

pub fn run_command(mut cmd: Command) -> Result<()> {
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("ssh exited with status {status}");
    }
    Ok(())
}

pub fn command_preview(
    host: &Host,
    config: &Config,
    default_key: Option<&str>,
    extra: Option<&str>,
) -> String {
    let mut parts: Vec<String> = vec!["ssh".to_string()];

    if let Some(bastion_name) = &host.bastion {
        match build_bastion_string(config, bastion_name, default_key, &mut Vec::new()) {
            Ok(b_str) => {
                parts.push("-J".into());
                parts.push(b_str);
            }
            Err(_) => {
                parts.push(format!("-J <error: bastion '{}' not found>", bastion_name));
            }
        }
    }

    if let Some(port) = host.port {
        parts.push("-p".into());
        parts.push(port.to_string());
    }

    if let Some(key) = select_key(host.key_path.as_deref(), default_key) {
        parts.push("-i".into());
        parts.push(key);
    }

    for opt in &host.options {
        parts.push(opt.clone());
    }

    if let Some(user) = &host.user {
        parts.push(format!("{user}@{}", host.address));
    } else {
        parts.push(host.address.clone());
    }

    if let Some(extra_cmd) = extra {
        parts.push(extra_cmd.to_string());
    } else if let Some(remote) = &host.remote_command {
        parts.push(remote.clone());
    }

    parts.join(" ")
}

fn build_bastion_string(
    config: &Config,
    bastion_name: &str,
    default_key: Option<&str>,
    visited: &mut Vec<String>,
) -> Result<String> {
    if visited.contains(&bastion_name.to_string()) {
        anyhow::bail!("circular bastion reference detected: {}", bastion_name);
    }
    visited.push(bastion_name.to_string());

    let bastion = config
        .find_host(bastion_name)
        .with_context(|| format!("bastion host '{}' not found", bastion_name))?;

    let mut chains = Vec::new();
    if let Some(nested) = &bastion.bastion {
        let nested_str = build_bastion_string(config, nested, default_key, visited)?;
        chains.push(nested_str);
    }

    let mut bastion_str = if let Some(user) = &bastion.user {
        format!("{user}@{}", bastion.address)
    } else {
        bastion.address.clone()
    };
    if let Some(port) = bastion.port {
        bastion_str.push_str(&format!(":{}", port));
    }

    if !chains.is_empty() {
        chains.push(bastion_str);
        Ok(chains.join(","))
    } else {
        Ok(bastion_str)
    }
}

fn select_key(host_key: Option<&str>, default_key: Option<&str>) -> Option<String> {
    const FALLBACKS: [&str; 2] = ["~/.ssh/id_ed25519", "~/.ssh/id_rsa"];
    if let Some(k) = host_key {
        return Some(expand_tilde(k));
    }
    if let Some(k) = default_key {
        if k == "agent" {
            return None;
        }
        return Some(expand_tilde(k));
    }

    let agent_available = std::env::var("SSH_AUTH_SOCK")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    if agent_available {
        return None;
    }

    // fall back to common keys when no agent is present
    for cand in FALLBACKS {
        return Some(expand_tilde(cand));
    }
    None
}

fn expand_tilde(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(stripped)
                .to_string_lossy()
                .into_owned();
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn builds_preview_with_defaults() {
        let config = Config::default();
        let host = Host {
            name: "prod".into(),
            address: "10.0.0.1".into(),
            user: Some("deploy".into()),
            port: Some(2222),
            key_path: None,
            tags: vec![],
            options: vec!["-L".into(), "8080:localhost:80".into()],
            remote_command: None,
            description: None,
            bastion: None,
        };
        let preview = command_preview(&host, &config, Some("~/.ssh/id_ed25519"), Some("uptime"));
        assert!(preview.contains("-p 2222"));
        assert!(preview.contains("-i"));
        assert!(preview.contains("deploy@10.0.0.1"));
        assert!(preview.ends_with("uptime"));
        assert!(preview.contains("-L 8080:localhost:80"));
    }

    #[test]
    fn expands_tilde() {
        let out = expand_tilde("~/abc");
        if let Ok(home) = std::env::var("HOME") {
            assert!(out.contains(&home));
        } else {
            assert_eq!(out, "~/abc".to_string());
        }
    }

    #[test]
    fn uses_fallback_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let config = Config::default();
        let host = Host {
            name: "fallback".into(),
            address: "example.com".into(),
            user: None,
            port: None,
            key_path: None,
            tags: vec![],
            options: Vec::new(),
            remote_command: None,
            description: None,
            bastion: None,
        };
        let old = std::env::var("SSH_AUTH_SOCK").ok();
        unsafe { std::env::remove_var("SSH_AUTH_SOCK") };
        let preview = command_preview(&host, &config, None, None);
        if let Some(prev) = old {
            unsafe { std::env::set_var("SSH_AUTH_SOCK", prev) };
        }
        assert!(preview.contains("-i"));
    }

    #[test]
    fn respects_agent_when_available() {
        let _guard = ENV_LOCK.lock().unwrap();
        let config = Config::default();
        let host = Host {
            name: "agent".into(),
            address: "example.com".into(),
            user: None,
            port: None,
            key_path: None,
            tags: vec![],
            options: Vec::new(),
            remote_command: None,
            description: None,
            bastion: None,
        };
        let old = std::env::var("SSH_AUTH_SOCK").ok();
        unsafe {
            std::env::set_var("SSH_AUTH_SOCK", "/tmp/agent.sock");
        }
        let preview = command_preview(&host, &config, None, None);
        if let Some(prev) = old {
            unsafe { std::env::set_var("SSH_AUTH_SOCK", prev) };
        } else {
            unsafe { std::env::remove_var("SSH_AUTH_SOCK") };
        }
        assert!(!preview.contains("-i"), "agent mode should not add -i");
    }
}
