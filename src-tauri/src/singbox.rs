//! Generate a sing-box client config from a parsed server + split-tunnel rules.
//!
//! Targets the sing-box 1.12+ schema. The produced config is validated against
//! the installed core (`sing-box check`) before it is used to connect.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::subscription::VlessServer;

/// Split-tunnel selection passed from the UI (only enabled entries).
///
/// One `mode` applies to BOTH the apps and sites lists. selective = whitelist
/// (only listed entries get the proxy outbound; default direct). general =
/// blacklist (all traffic uses the proxy outbound; listed entries are
/// exceptions that stay direct).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SplitInput {
    /// "selective" | "general". Empty string is treated as "general" so an
    /// uninitialised input doesn't accidentally cut the user's network.
    #[serde(default)]
    pub mode: String,
    /// Process / binary names of enabled apps.
    #[serde(default)]
    pub apps: Vec<String>,
    /// Enabled site patterns (e.g. "example.com" or "*.example.com").
    #[serde(default)]
    pub sites: Vec<String>,
}

fn is_selective(mode: &str) -> bool {
    mode == "selective"
}

/// The `proxy` outbound for the hybrid: sing-box forwards ALL tunneled traffic
/// to the local xray SOCKS, which does the actual vless/reality/XHTTP transport
/// (sing-box can't speak XHTTP). xray must already be listening before sing-box
/// dials it — `vpn_connect` spawns xray first.
fn socks_to_xray_outbound() -> Value {
    json!({
        "type": "socks",
        "tag": "proxy",
        "server": "127.0.0.1",
        "server_port": crate::xray::XRAY_SOCKS_PORT,
        "version": "5",
        "udp_over_tcp": false
    })
}

/// Route rules derived from the split-tunnel selection.
///
/// The mode controls TWO things at once: where listed entries go, and where
/// everything else goes (the route `final`):
///   - selective: listed -> proxy, default direct (whitelist).
///   - general:   listed -> direct, default proxy (blacklist).
///
/// Apps and sites both follow the same mode (the UI exposes one toggle), so
/// "selective apps + general sites" can't accidentally degrade selective
/// behaviour by silently flipping the default — the old per-list modes had
/// that bug.
fn build_route_rules(split: &SplitInput) -> (Vec<Value>, &'static str) {
    let mut rules = Vec::new();

    let selective = is_selective(&split.mode);
    let listed_outbound = if selective { "proxy" } else { "direct" };
    let default = if selective { "direct" } else { "proxy" };

    for app in &split.apps {
        if !app.is_empty() {
            rules.push(json!({ "process_name": [app], "outbound": listed_outbound }));
        }
    }

    for site in &split.sites {
        let site = site.trim();
        if site.is_empty() {
            continue;
        }
        if let Some(suffix) = site.strip_prefix("*.") {
            rules.push(json!({ "domain_suffix": [suffix], "outbound": listed_outbound }));
        } else {
            rules.push(json!({ "domain": [site], "outbound": listed_outbound }));
        }
    }

    (rules, default)
}

/// Local mixed (SOCKS5 + HTTP) inbound port used by "proxy" mode.
pub const PROXY_PORT: u16 = 2080;

/// Whole-system TUN inbound (canonical sing-box 1.12+ client setup).
fn tun_inbound() -> Value {
    json!({
        "type": "tun",
        "tag": "tun-in",
        "interface_name": "aegis0",
        "address": ["172.19.0.1/30"],
        "mtu": 1500,
        "auto_route": true,
        "strict_route": true,
        // Linux nftables fast-path; required for reliable capture with auto_route.
        "auto_redirect": true,
        // system TCP + gvisor UDP — the recommended, most reliable stack.
        "stack": "mixed"
    })
}

/// Local SOCKS5/HTTP inbound for "proxy" mode — no TUN, no root needed; apps
/// point at 127.0.0.1:PROXY_PORT.
fn proxy_inbound() -> Value {
    json!({
        "type": "mixed",
        "tag": "mixed-in",
        "listen": "127.0.0.1",
        "listen_port": PROXY_PORT
    })
}

/// Assemble the full sing-box config for the given mode ("tun" | "proxy").
/// The client tunnels everything (subject to split rules); it deliberately
/// does no geo-based bypass — geo routing is the server's concern.
///
/// In the hybrid, sing-box does TUN + routing + DNS and forwards the tunneled
/// traffic to the local xray SOCKS (`socks_to_xray_outbound`). `server` is
/// unused here — xray dials the actual server — but kept in the signature for
/// the command/tests.
pub fn build_config(_server: &VlessServer, split: &SplitInput, mode: &str, allow_lan: bool) -> Value {
    let proxy_mode = mode == "proxy";
    let (split_rules, final_out) = build_route_rules(split);

    // Canonical route rule order: sniff → DNS hijack (TUN only) → optionally
    // keep private/LAN traffic direct (per the "Allow LAN" toggle) → split.
    let mut rules = vec![json!({ "action": "sniff" })];
    if !proxy_mode {
        rules.push(json!({ "protocol": "dns", "action": "hijack-dns" }));
    }
    if allow_lan {
        rules.push(json!({ "ip_is_private": true, "outbound": "direct" }));
    }
    rules.extend(split_rules);

    json!({
        "log": { "level": "warn" },
        // Anti-leak DNS: every app query is hijacked into sing-box's DNS module
        // and resolved via `remote` (DoH 1.1.1.1, detoured through the proxy →
        // xray → tunnel), so it egresses from the VPN node, never the ISP/RU
        // resolver. `local` exists ONLY as a bootstrap and is never the `final`
        // for app traffic. No `default_domain_resolver` — servers are IP
        // literals (xray dials them), so nothing needs off-tunnel resolution.
        "dns": {
            "servers": [
                { "type": "https", "tag": "remote", "server": "1.1.1.1", "detour": "proxy" },
                { "type": "udp", "tag": "local", "server": "1.1.1.1" }
            ],
            "final": "remote",
            "strategy": "prefer_ipv4"
        },
        "inbounds": [if proxy_mode { proxy_inbound() } else { tun_inbound() }],
        "outbounds": [
            socks_to_xray_outbound(),
            { "type": "direct", "tag": "direct" }
        ],
        "route": {
            "rules": rules,
            "final": final_out,
            "auto_detect_interface": true
        }
    })
}

// --- Tauri command ----------------------------------------------------------

/// Build and return the sing-box config JSON (pretty-printed) for a server +
/// split selection. Used both for inspection and (later) to connect.
#[tauri::command]
pub fn generate_singbox_config(
    server: VlessServer,
    split: SplitInput,
    mode: String,
    allow_lan: bool,
) -> Result<String, String> {
    let cfg = build_config(&server, &split, &mode, allow_lan);
    serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subscription::parse_proxy_uri;

    fn split() -> SplitInput {
        SplitInput::default()
    }

    #[test]
    fn tun_proxy_outbound_is_socks_to_xray() {
        // In the hybrid, sing-box's proxy outbound is always a SOCKS hop to the
        // local xray (which does the real vless/reality/xhttp transport).
        let s = parse_proxy_uri(
            "vless://uuid-1@1.2.3.4:443?type=xhttp&security=reality&pbk=KEY&sid=ab&fp=firefox#X",
        )
        .unwrap();
        let cfg = build_config(&s, &split(), "tun", true);
        let ob = &cfg["outbounds"][0];
        assert_eq!(ob["type"], "socks");
        assert_eq!(ob["tag"], "proxy");
        assert_eq!(ob["server"], "127.0.0.1");
        assert_eq!(ob["server_port"], crate::xray::XRAY_SOCKS_PORT);
    }

    #[test]
    fn dns_has_no_offtunnel_default_resolver() {
        // DNS-leak guard: `final` is the proxied DoH resolver and there's no
        // default_domain_resolver pointing at plaintext `local`.
        let s = parse_proxy_uri("vless://u@1.2.3.4:443?type=xhttp&security=reality&pbk=K#X").unwrap();
        let cfg = build_config(&s, &split(), "tun", true);
        assert_eq!(cfg["dns"]["final"], "remote");
        assert!(cfg["route"].get("default_domain_resolver").is_none());
    }

    #[test]
    fn selective_apps_route_to_proxy() {
        let s = parse_proxy_uri("vless://u@1.2.3.4:443?security=reality&pbk=K#X").unwrap();
        let sp = SplitInput {
            mode: "selective".into(),
            apps: vec!["firefox".into()],
            ..Default::default()
        };
        let cfg = build_config(&s, &sp, "tun", true);
        assert_eq!(cfg["route"]["final"], "direct");
        let rules = cfg["route"]["rules"].as_array().unwrap();
        let app_rule = rules.iter().find(|r| r.get("process_name").is_some()).unwrap();
        assert_eq!(app_rule["process_name"][0], "firefox");
        assert_eq!(app_rule["outbound"], "proxy");
    }

    #[test]
    fn selective_with_empty_lists_routes_everything_direct() {
        // Regression: previously "apps selective + sites general" (the default
        // for sites_mode after toggling apps to selective) defaulted to proxy
        // and silently made selective meaningless.
        let s = parse_proxy_uri("vless://u@1.2.3.4:443?security=reality&pbk=K#X").unwrap();
        let sp = SplitInput { mode: "selective".into(), ..Default::default() };
        let cfg = build_config(&s, &sp, "tun", true);
        assert_eq!(cfg["route"]["final"], "direct");
    }

    #[test]
    fn general_sites_route_to_direct() {
        let s = parse_proxy_uri("vless://u@1.2.3.4:443?security=reality&pbk=K#X").unwrap();
        let sp = SplitInput {
            mode: "general".into(),
            sites: vec!["*.ru".into()],
            ..Default::default()
        };
        let cfg = build_config(&s, &sp, "tun", true);
        assert_eq!(cfg["route"]["final"], "proxy");
        let rules = cfg["route"]["rules"].as_array().unwrap();
        let site_rule = rules.iter().find(|r| r.get("domain_suffix").is_some()).unwrap();
        assert_eq!(site_rule["domain_suffix"][0], "ru");
        assert_eq!(site_rule["outbound"], "direct");
    }
}
