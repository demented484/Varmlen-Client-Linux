//! Generate an xray-core client config from a parsed server.
//!
//! In the hybrid architecture xray is *only* the upstream transport engine: a
//! single local SOCKS inbound that sing-box's TUN forwards into, plus one
//! vless/reality outbound. All TUN/routing/split-tunnel/DNS logic stays in
//! sing-box (`singbox.rs`); xray exists solely because sing-box cannot speak
//! XHTTP. The produced config is validated with `xray run -test -c <file>`
//! before launch.
//!
//! Note the dialect differences from sing-box: xray uses `streamSettings`
//! (camelCase) with nested `realitySettings` / `xhttpSettings` / `tlsSettings`,
//! and `vnext[].users[]` instead of a flat outbound.

use serde_json::{json, Value};

use crate::subscription::VlessServer;

/// Local SOCKS port xray listens on; sing-box's TUN proxy outbound dials this.
/// Distinct from `singbox::PROXY_PORT` (2080) so the legacy local-proxy mode
/// and the hybrid upstream can't collide.
pub const XRAY_SOCKS_PORT: u16 = 2081;

/// Map our `transport` field to xray's `streamSettings.network`. xray speaks
/// every transport the subscription can carry, including xhttp (the whole
/// reason this core exists).
fn xray_network(transport: &str) -> &str {
    match transport {
        "xhttp" => "xhttp",
        "ws" => "ws",
        "grpc" => "grpc",
        "http" => "http",
        _ => "tcp",
    }
}

/// Build the `streamSettings` object: security (reality/tls/none) + the
/// transport-specific settings block.
fn build_stream_settings(s: &VlessServer) -> Value {
    let network = xray_network(&s.transport);
    let server_name = s
        .sni
        .clone()
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| s.host.clone());
    let fp = s
        .fingerprint
        .clone()
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| "chrome".to_string());

    let mut stream = serde_json::Map::new();
    stream.insert("network".into(), json!(network));

    // Security layer.
    match s.security.as_str() {
        "reality" => {
            stream.insert("security".into(), json!("reality"));
            stream.insert(
                "realitySettings".into(),
                json!({
                    "show": false,
                    "serverName": server_name,
                    "fingerprint": fp,
                    "publicKey": s.public_key.clone().unwrap_or_default(),
                    "shortId": s.short_id.clone().unwrap_or_default(),
                    // spiderX (spx) is carried in the subscription's raw params.
                    "spiderX": s.raw_params.get("spx").cloned().unwrap_or_else(|| "/".into()),
                }),
            );
        }
        "tls" => {
            stream.insert("security".into(), json!("tls"));
            stream.insert(
                "tlsSettings".into(),
                json!({
                    "serverName": server_name,
                    "fingerprint": fp,
                    "allowInsecure": false,
                }),
            );
        }
        _ => {
            stream.insert("security".into(), json!("none"));
        }
    }

    // Transport-specific settings.
    match network {
        "xhttp" => {
            let mut xs = serde_json::Map::new();
            xs.insert("path".into(), json!(s.path.clone().unwrap_or_else(|| "/".into())));
            xs.insert("mode".into(), json!(s.mode.clone().unwrap_or_else(|| "auto".into())));
            if let Some(host) = s.raw_params.get("host").filter(|h| !h.is_empty()) {
                xs.insert("host".into(), json!(host));
            }
            stream.insert("xhttpSettings".into(), Value::Object(xs));
        }
        "ws" => {
            let mut ws = serde_json::Map::new();
            ws.insert("path".into(), json!(s.path.clone().unwrap_or_else(|| "/".into())));
            if let Some(host) = s.raw_params.get("host").filter(|h| !h.is_empty()) {
                ws.insert("headers".into(), json!({ "Host": host }));
            }
            stream.insert("wsSettings".into(), Value::Object(ws));
        }
        "grpc" => {
            stream.insert(
                "grpcSettings".into(),
                json!({ "serviceName": s.raw_params.get("serviceName").cloned().unwrap_or_default() }),
            );
        }
        "http" => {
            stream.insert(
                "httpSettings".into(),
                json!({ "path": s.path.clone().unwrap_or_else(|| "/".into()) }),
            );
        }
        _ => {}
    }

    Value::Object(stream)
}

/// Build the `proxy` outbound for the selected server. Only VLESS is wired for
/// the hybrid (every active server is vless+reality+xhttp); other protocols
/// would be handled by sing-box natively, not routed through xray.
fn build_proxy_outbound(s: &VlessServer) -> Value {
    json!({
        "tag": "proxy",
        "protocol": "vless",
        "settings": {
            "vnext": [{
                "address": s.host,
                "port": s.port,
                "users": [{
                    "id": s.uuid,
                    "encryption": "none",
                    // xhttp uses no flow; vision (tcp) would set it. Empty when absent.
                    "flow": s.flow.clone().unwrap_or_default(),
                }]
            }]
        },
        "streamSettings": build_stream_settings(s),
    })
}

/// Full xray config: one SOCKS inbound (for sing-box to forward into) + the
/// vless outbound + a freedom/direct outbound.
pub fn build_xray_config(server: &VlessServer) -> Value {
    json!({
        "log": { "loglevel": "warning" },
        "inbounds": [{
            "tag": "socks-in",
            "listen": "127.0.0.1",
            "port": XRAY_SOCKS_PORT,
            "protocol": "socks",
            "settings": { "udp": true, "auth": "noauth" },
            "sniffing": { "enabled": true, "destOverride": ["http", "tls", "quic"], "routeOnly": false }
        }],
        "outbounds": [
            build_proxy_outbound(server),
            { "tag": "direct", "protocol": "freedom" }
        ],
        "routing": { "rules": [] }
    })
}

/// Build and pretty-print the xray config JSON for inspection / launch.
#[tauri::command]
pub fn generate_xray_config(server: VlessServer) -> Result<String, String> {
    serde_json::to_string_pretty(&build_xray_config(&server)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subscription::parse_proxy_uri;

    #[test]
    fn vless_reality_xhttp_outbound() {
        let s = parse_proxy_uri(
            "vless://16ddb21e-5342-4a82-a870-1038b01b8dbc@46.29.238.157:443?type=xhttp&security=reality&encryption=none&sni=gateway.icloud.com&fp=firefox&pbk=PUBKEY&sid=SID&spx=%2F&path=%2F&mode=packet-up#NO",
        )
        .expect("parse");
        let cfg = build_xray_config(&s);

        let out = &cfg["outbounds"][0];
        assert_eq!(out["protocol"], "vless");
        let user = &out["settings"]["vnext"][0]["users"][0];
        assert_eq!(user["id"], "16ddb21e-5342-4a82-a870-1038b01b8dbc");
        assert_eq!(user["encryption"], "none");
        // xhttp carries no flow.
        assert_eq!(user["flow"], "");

        let ss = &out["streamSettings"];
        assert_eq!(ss["network"], "xhttp");
        assert_eq!(ss["security"], "reality");
        assert_eq!(ss["realitySettings"]["fingerprint"], "firefox");
        assert_eq!(ss["realitySettings"]["publicKey"], "PUBKEY");
        assert_eq!(ss["realitySettings"]["shortId"], "SID");
        assert_eq!(ss["realitySettings"]["serverName"], "gateway.icloud.com");
        assert_eq!(ss["xhttpSettings"]["path"], "/");
        assert_eq!(ss["xhttpSettings"]["mode"], "packet-up");

        // SOCKS inbound for sing-box to forward into.
        let inb = &cfg["inbounds"][0];
        assert_eq!(inb["protocol"], "socks");
        assert_eq!(inb["port"], XRAY_SOCKS_PORT);
        assert_eq!(inb["settings"]["udp"], true);
    }

    #[test]
    fn tcp_reality_vision_keeps_flow() {
        let s = parse_proxy_uri(
            "vless://uuid-1@1.2.3.4:443?type=tcp&security=reality&flow=xtls-rprx-vision&sni=icloud.com&pbk=K&sid=ab&fp=chrome#X",
        )
        .expect("parse");
        let cfg = build_xray_config(&s);
        let out = &cfg["outbounds"][0];
        assert_eq!(out["settings"]["vnext"][0]["users"][0]["flow"], "xtls-rprx-vision");
        assert_eq!(out["streamSettings"]["network"], "tcp");
        // tcp transport adds no transport-settings block
        assert!(out["streamSettings"].get("xhttpSettings").is_none());
    }
}
