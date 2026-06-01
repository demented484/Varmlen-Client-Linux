//! aegis-probe — a tiny setcap'd helper for the AegisVPN desktop client.
//!
//! Replaces the old root systemd daemon. It is NOT a daemon: the GUI invokes
//! it per-action and it exits. It carries file capabilities
//! `cap_net_admin,cap_net_raw+ep` (set once via pkexec at install/update), which
//! is exactly what the privileged operations need:
//!   - `tcp`/`icmp`     latency probes that bypass the active tunnel
//!                      (SO_MARK 0x2024 needs CAP_NET_ADMIN, SO_BINDTODEVICE
//!                      needs CAP_NET_RAW)
//!   - `killswitch-up`  apply the nftables drop table (CAP_NET_ADMIN)
//!   - `killswitch-down`remove it
//!   - `cleanup`        tear down a stranded TUN device + stray killswitch
//!
//! The GUI owns the sing-box/xray processes directly now, so process spawning
//! and the unix socket are gone.
//!
//! Usage:
//!   aegis-probe tcp  <host> <port> <timeout_ms>      -> prints RTT ms
//!   aegis-probe icmp <host> <timeout_ms>             -> prints RTT ms
//!   aegis-probe killswitch-up [--allow-lan] <ip>...  -> applies nft table
//!   aegis-probe killswitch-down
//!   aegis-probe cleanup

use std::process::{Command, Stdio};
use std::io::Write;

const KS_TABLE: &str = "aegis_ks";
const TUN_IFACE: &str = "aegis0";
const SING_BOX_BYPASS_MARK: u32 = 0x2024;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rc = run(&args);
    std::process::exit(rc);
}

fn run(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("tcp") => {
            // tcp <host> <port> <timeout_ms>
            let (Some(host), Some(port), Some(tmo)) = (args.get(1), args.get(2), args.get(3)) else {
                eprintln!("usage: aegis-probe tcp <host> <port> <timeout_ms>");
                return 2;
            };
            let port: u16 = match port.parse() { Ok(p) => p, Err(_) => { eprintln!("bad port"); return 2; } };
            let tmo: u32 = tmo.parse().unwrap_or(2500);
            match tcp_ping_bypass(host, port, tmo) {
                Ok(ms) => { println!("{ms}"); 0 }
                Err(e) => { eprintln!("{e}"); 1 }
            }
        }
        Some("icmp") => {
            let (Some(host), Some(tmo)) = (args.get(1), args.get(2)) else {
                eprintln!("usage: aegis-probe icmp <host> <timeout_ms>");
                return 2;
            };
            let tmo: u32 = tmo.parse().unwrap_or(2000);
            match icmp_ping(host, tmo) {
                Ok(ms) => { println!("{ms}"); 0 }
                Err(e) => { eprintln!("{e}"); 1 }
            }
        }
        Some("killswitch-up") => {
            let mut allow_lan = false;
            let mut ips = Vec::new();
            for a in &args[1..] {
                if a == "--allow-lan" { allow_lan = true; }
                else if let Ok(ip) = a.parse::<std::net::IpAddr>() { ips.push(ip); }
            }
            match apply_killswitch(&ips, allow_lan) {
                Ok(()) => 0,
                Err(e) => { eprintln!("{e}"); 1 }
            }
        }
        Some("killswitch-down") => { remove_killswitch(); 0 }
        Some("cleanup") => { remove_killswitch(); delete_tun(); 0 }
        _ => {
            eprintln!("usage: aegis-probe <tcp|icmp|killswitch-up|killswitch-down|cleanup> ...");
            2
        }
    }
}

// --- latency probes --------------------------------------------------------

/// First non-virtual interface with an IPv4 address. SO_BINDTODEVICE to this
/// makes probes bypass sing-box's auto_route/auto_redirect (which catches by
/// destination, not source — plain bind to a phys-iface IP isn't enough).
fn pick_physical_iface() -> Option<String> {
    unsafe {
        let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifap) != 0 {
            return None;
        }
        let mut found = None;
        let mut cur = ifap;
        while !cur.is_null() {
            let ifa = &*cur;
            if !ifa.ifa_name.is_null() && !ifa.ifa_addr.is_null() {
                let name = std::ffi::CStr::from_ptr(ifa.ifa_name).to_string_lossy().into_owned();
                let virt = name.starts_with("lo")
                    || name.starts_with("tun")
                    || name.starts_with("tap")
                    || name.starts_with("wg")
                    || name.starts_with("docker")
                    || name.starts_with("br-")
                    || name.starts_with("veth")
                    || name.starts_with("vmnet")
                    || name.starts_with("aegis");
                let sa = &*ifa.ifa_addr;
                if !virt && sa.sa_family as i32 == libc::AF_INET {
                    let sin = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                    let ip = std::net::Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
                    if !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified() {
                        found = Some(name);
                        break;
                    }
                }
            }
            cur = ifa.ifa_next;
        }
        libc::freeifaddrs(ifap);
        found
    }
}

/// TCP-connect RTT in ms, source-bound + marked so it bypasses the tunnel.
fn tcp_ping_bypass(host: &str, port: u16, timeout_ms: u32) -> Result<u32, String> {
    use socket2::{Domain, Protocol, SockAddr, Socket, Type};
    use std::net::{SocketAddr, ToSocketAddrs};
    use std::time::{Duration, Instant};

    if host.trim().is_empty() {
        return Err("empty host".into());
    }
    if !host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-' | '_')) {
        return Err("invalid host".into());
    }
    let dst: SocketAddr = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("resolve: {e}"))?
        .find(|a| a.is_ipv4())
        .ok_or_else(|| "no ipv4".to_string())?;

    let sock = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
        .map_err(|e| format!("socket: {e}"))?;
    let _ = sock.set_mark(SING_BOX_BYPASS_MARK);
    if let Some(iface) = pick_physical_iface() {
        let _ = sock.bind_device(Some(iface.as_bytes()));
    }
    let timeout = Duration::from_millis(timeout_ms as u64);
    let started = Instant::now();
    sock.connect_timeout(&SockAddr::from(dst), timeout)
        .map_err(|e| format!("connect: {e}"))?;
    Ok(started.elapsed().as_millis().min(u32::MAX as u128) as u32)
}

/// ICMP RTT via the system `ping` tool.
fn icmp_ping(host: &str, timeout_ms: u32) -> Result<u32, String> {
    if host.trim().is_empty() {
        return Err("empty host".into());
    }
    if !host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-' | '_')) {
        return Err("invalid host".into());
    }
    let timeout_s = ((timeout_ms + 999) / 1000).clamp(1, 10);
    let out = Command::new("ping")
        .arg("-n").arg("-c").arg("1").arg("-W").arg(timeout_s.to_string()).arg(host)
        .stdout(Stdio::piped()).stderr(Stdio::null())
        .output().map_err(|e| format!("spawn ping: {e}"))?;
    if !out.status.success() {
        return Err("ping failed".into());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some(idx) = line.find("time=") {
            let rest = &line[idx + 5..];
            let num: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            if let Ok(ms) = num.parse::<f64>() {
                return Ok(ms.round().max(1.0).min(u32::MAX as f64) as u32);
            }
        }
    }
    Err("no RTT in ping output".into())
}

// --- killswitch ------------------------------------------------------------

/// Build + apply the killswitch ruleset: drop all output except loopback, the
/// tunnel, sing-box's own marked traffic, the proxy server, DNS bootstrap, and
/// (optionally) LAN. Atomic via a single `nft -f` transaction.
fn apply_killswitch(server_ips: &[std::net::IpAddr], allow_lan: bool) -> Result<(), String> {
    let mut r = String::new();
    r.push_str(&format!("add table inet {KS_TABLE}\n"));
    r.push_str(&format!("delete table inet {KS_TABLE}\n"));
    r.push_str(&format!("table inet {KS_TABLE} {{\n"));
    r.push_str("  chain out {\n");
    r.push_str("    type filter hook output priority 0; policy drop;\n");
    r.push_str("    oifname \"lo\" counter accept\n");
    r.push_str(&format!("    oifname \"{TUN_IFACE}\" counter accept\n"));
    r.push_str("    ct state established,related counter accept\n");
    r.push_str("    fib daddr type local counter accept\n");
    r.push_str("    meta mark & 0x0000ffff == 0x2023 counter accept\n");
    r.push_str("    meta mark & 0x0000ffff == 0x2024 counter accept\n");
    r.push_str("    ct mark & 0x0000ffff == 0x2023 counter accept\n");
    r.push_str("    ct mark & 0x0000ffff == 0x2024 counter accept\n");
    r.push_str("    ip daddr 1.1.1.1 udp dport 53 counter accept\n");
    r.push_str("    ip daddr 1.1.1.1 tcp dport 53 counter accept\n");
    r.push_str("    ip daddr 1.1.1.1 tcp dport 443 counter accept\n");
    for ip in server_ips {
        match ip {
            std::net::IpAddr::V4(v4) => r.push_str(&format!("    ip daddr {v4} counter accept\n")),
            std::net::IpAddr::V6(v6) => r.push_str(&format!("    ip6 daddr {v6} counter accept\n")),
        }
    }
    if allow_lan {
        r.push_str("    ip daddr { 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16, 100.64.0.0/10 } counter accept\n");
        r.push_str("    ip6 daddr { fe80::/10, fc00::/7 } counter accept\n");
    }
    r.push_str("    limit rate 30/second log prefix \"aegis_ks_drop \" level info\n");
    r.push_str("    counter drop\n");
    r.push_str("  }\n}\n");

    let mut child = Command::new("nft").arg("-f").arg("-")
        .stdin(Stdio::piped()).spawn().map_err(|e| format!("nft spawn: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(r.as_bytes()).map_err(|e| format!("nft write: {e}"))?;
    }
    let status = child.wait().map_err(|e| format!("nft wait: {e}"))?;
    if status.success() { Ok(()) } else { Err("nft apply failed".to_string()) }
}

fn remove_killswitch() {
    let _ = Command::new("nft").arg("delete").arg("table").arg("inet").arg(KS_TABLE)
        .stderr(Stdio::null()).status();
}

fn delete_tun() {
    let _ = Command::new("ip").arg("link").arg("delete").arg("dev").arg(TUN_IFACE)
        .stderr(Stdio::null()).status();
}
