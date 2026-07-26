use std::process::Stdio;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::protocol::{DaemonError, DaemonErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DnsNftPlan {
    local_port: u16,
}

impl DnsNftPlan {
    pub fn new(local_port: u16) -> Self {
        Self { local_port }
    }
}

pub fn render_dns_rules(plan: &DnsNftPlan) -> String {
    format!(
        r#"table inet varmlen_dns {{
  chain dns_output {{
    type nat hook output priority dstnat - 10; policy accept;
    udp dport 53 redirect to :{port}
    tcp dport 53 redirect to :{port}
  }}
  chain guard_output {{
    type filter hook output priority filter - 10; policy accept;
    udp dport 53 reject
    tcp dport 53 reject
    tcp dport 853 reject
    oifname "lo" accept
    meta mark & 0x0000ffff == 0x2025 accept
    ct mark & 0x0000ffff == 0x2025 accept
    ip daddr 10.0.0.0/8 accept
    ip daddr 172.16.0.0/12 accept
    ip daddr 192.168.0.0/16 accept
    ip6 daddr fc00::/7 accept
  }}
}}
"#,
        port = plan.local_port
    )
}

pub async fn apply_ruleset(ruleset: &str) -> Result<(), DaemonError> {
    let mut child = Command::new("nft")
        .args(["-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            DaemonError::new(
                DaemonErrorCode::DnsInstallFailed,
                format!("could not start nft: {error}"),
            )
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(ruleset.as_bytes()).await.map_err(|error| {
            DaemonError::new(
                DaemonErrorCode::DnsInstallFailed,
                format!("could not write nft transaction: {error}"),
            )
        })?;
    }
    let output = child.wait_with_output().await.map_err(|error| {
        DaemonError::new(
            DaemonErrorCode::DnsInstallFailed,
            format!("could not wait for nft: {error}"),
        )
    })?;
    if !output.status.success() {
        return Err(DaemonError::new(
            DaemonErrorCode::DnsInstallFailed,
            format!(
                "nft rejected DNS transaction: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    Ok(())
}

pub async fn remove_dns_table() -> Result<(), DaemonError> {
    let output = Command::new("nft")
        .args(["delete", "table", "inet", "varmlen_dns"])
        .output()
        .await
        .map_err(|error| {
            DaemonError::new(
                DaemonErrorCode::DnsInstallFailed,
                format!("could not remove DNS table: {error}"),
            )
        })?;
    if output.status.success()
        || String::from_utf8_lossy(&output.stderr).contains("No such file or directory")
    {
        return Ok(());
    }
    Err(DaemonError::new(
        DaemonErrorCode::DnsInstallFailed,
        format!(
            "could not remove DNS table: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::{render_dns_rules, DnsNftPlan};

    #[test]
    fn dns_redirect_runs_before_lan_and_split_accepts() {
        let rules = render_dns_rules(&DnsNftPlan::new(5353));
        let redirect = rules.find("udp dport 53 redirect to :5353").unwrap();
        let split = rules.find("meta mark & 0x0000ffff == 0x2025 accept").unwrap();
        let lan = rules.find("ip daddr 10.0.0.0/8 accept").unwrap();
        assert!(redirect < split);
        assert!(redirect < lan);
        assert!(rules.contains("tcp dport 53 redirect to :5353"));
        assert!(rules.contains("tcp dport 853 reject"));
    }

    #[test]
    fn dns_policy_never_allows_direct_port_53() {
        let rules = render_dns_rules(&DnsNftPlan::new(5353));
        assert!(!rules.contains("dport 53 accept"));
        assert!(!rules.contains("1.1.1.1"));
        assert!(!rules.contains("192.168.1.1"));
    }
}
