//! leakcheck — диагностика split-tunneling и утечек для Varmlen.
//!
//! Каждый цикл делает ДВЕ независимые проверки и печатает публичный IP + RTT:
//!   - TCP: HTTP-запрос к api.ipify.org (путь браузера / обычных приложений).
//!     Каждый цикл — НОВОЕ соединение, так что строка отражает текущую
//!     маршрутизацию, а не старый установленный поток.
//!   - UDP: STUN Binding к Cloudflare на :3478. Это настоящий игровой UDP-путь,
//!     не порт 53 (Varmlen намеренно перехватывает DNS даже у исключений).
//!
//! Как пользоваться:
//!   1. Запустить БЕЗ VPN — первый замер запоминается как «реальный IP»
//!      (или задайте явно: `leakcheck --real 1.2.3.4`).
//!   2. Добавить бинарник leakcheck в исключения Varmlen (выбор файла →
//!      target/release/leakcheck), подключить VPN в общем режиме:
//!      обе строки должны показывать РЕАЛЬНЫЙ IP и низкий пинг — обход работает.
//!   3. Убрать из исключений (выключить тумблер) — IP должен смениться на VPN.
//!   4. Переключать локации и смотреть на алерты: real-IP не должен всплывать;
//!      строки `timeout` в момент переключения — это НОРМА (блокировка держит).
//!
//! Только std, без зависимостей. `--interval-ms 300` для ловли коротких утечек.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

use serde::Serialize;

const HTTP_HOST: &str = "api.ipify.org";
const STUN_SERVER: &str = "stun.cloudflare.com:3478";
const STUN_MAGIC: u32 = 0x2112_a442;

const RED: &str = "\x1b[1;31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

/// Публичный IP + RTT по TCP: время connect() до HTTP-хоста, тело ответа — IP.
/// HTTP/1.0 → сервер не использует chunked, тело = голый адрес.
fn tcp_check(timeout: Duration) -> Result<(String, u32), String> {
    let addr: SocketAddr = (HTTP_HOST, 80)
        .to_socket_addrs()
        .map_err(|e| format!("resolve: {e}"))?
        .find(|a| a.is_ipv4())
        .ok_or("no ipv4")?;
    // RTT = запрос → ПЕРВЫЙ байт ответа. Не connect() (через туннель на SYN
    // отвечает сам xray локально, ≈0ms) и не EOF (xray закрывает соединение с
    // задержкой, что накидывало секунды) — только честный сквозной путь.
    let mut s = TcpStream::connect_timeout(&addr, timeout).map_err(|e| format!("connect: {e}"))?;
    s.set_read_timeout(Some(timeout)).ok();
    s.set_write_timeout(Some(timeout)).ok();
    let started = Instant::now();
    write!(
        s,
        "GET / HTTP/1.0\r\nHost: {HTTP_HOST}\r\nUser-Agent: leakcheck\r\n\r\n"
    )
    .map_err(|e| format!("send: {e}"))?;
    let mut raw = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut rtt: Option<u32> = None;
    loop {
        match s.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                rtt.get_or_insert_with(|| started.elapsed().as_millis() as u32);
                raw.extend_from_slice(&chunk[..n]);
            }
            Err(e) if raw.is_empty() => return Err(format!("recv: {e}")),
            Err(_) => break, // тело уже есть; поздний таймаут закрытия не важен
        }
    }
    let rtt = rtt.ok_or("empty reply")?;
    let resp = String::from_utf8_lossy(&raw).to_string();
    let body = resp.split("\r\n\r\n").nth(1).unwrap_or("").trim();
    body.parse::<std::net::IpAddr>()
        .map(|ip| (ip.to_string(), rtt))
        .map_err(|_| format!("bad body: {body:.40}"))
}

fn stun_request(transaction: [u8; 12]) -> [u8; 20] {
    let mut request = [0_u8; 20];
    request[0..2].copy_from_slice(&0x0001_u16.to_be_bytes());
    request[4..8].copy_from_slice(&STUN_MAGIC.to_be_bytes());
    request[8..20].copy_from_slice(&transaction);
    request
}

fn parse_stun_ipv4(response: &[u8], transaction: [u8; 12]) -> Option<Ipv4Addr> {
    if response.len() < 20
        || u16::from_be_bytes([response[0], response[1]]) != 0x0101
        || u32::from_be_bytes([response[4], response[5], response[6], response[7]]) != STUN_MAGIC
        || response[8..20] != transaction
    {
        return None;
    }
    let message_length = u16::from_be_bytes([response[2], response[3]]) as usize;
    let mut offset = 20_usize;
    let end = (20 + message_length).min(response.len());
    while offset + 4 <= end {
        let kind = u16::from_be_bytes([response[offset], response[offset + 1]]);
        let length = u16::from_be_bytes([response[offset + 2], response[offset + 3]]) as usize;
        let value = offset + 4;
        if value + length > end {
            return None;
        }
        if length >= 8 && response[value + 1] == 0x01 {
            let mut address = [
                response[value + 4],
                response[value + 5],
                response[value + 6],
                response[value + 7],
            ];
            if kind == 0x0020 {
                for (byte, mask) in address.iter_mut().zip(STUN_MAGIC.to_be_bytes()) {
                    *byte ^= mask;
                }
                return Some(Ipv4Addr::from(address));
            }
            if kind == 0x0001 {
                return Some(Ipv4Addr::from(address));
            }
        }
        offset = value + ((length + 3) & !3);
    }
    None
}

/// Публичный IP + RTT по настоящему UDP-пути через STUN.
fn udp_check(id: u16, timeout: Duration) -> Result<(String, u32), String> {
    let address = STUN_SERVER
        .to_socket_addrs()
        .map_err(|error| format!("resolve: {error}"))?
        .find(SocketAddr::is_ipv4)
        .ok_or("no STUN IPv4 address")?;
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("bind: {e}"))?;
    sock.set_read_timeout(Some(timeout)).ok();
    let mut transaction = [0_u8; 12];
    transaction[0..2].copy_from_slice(&id.to_be_bytes());
    transaction[2..6].copy_from_slice(&std::process::id().to_be_bytes());
    transaction[6..12].copy_from_slice(&(id as u64).wrapping_mul(0x9e37_79b9).to_be_bytes()[2..8]);
    let request = stun_request(transaction);
    let started = Instant::now();
    sock.send_to(&request, address)
        .map_err(|e| format!("send: {e}"))?;
    let mut buf = [0u8; 2048];
    let (n, _) = sock.recv_from(&mut buf).map_err(|e| format!("recv: {e}"))?;
    let rtt = started.elapsed().as_millis() as u32;
    parse_stun_ipv4(&buf[..n], transaction)
        .map(|ip| (ip.to_string(), rtt))
        .ok_or_else(|| "bad STUN response".into())
}

/// Пометка IP относительно реального: реальный — красным (после подключения VPN
/// его появление = утечка/обход), любой другой — зелёным.
fn tag(ip: &str, real: &Option<String>) -> String {
    match real {
        Some(r) if r == ip => format!("{RED}{ip} <- РЕАЛЬНЫЙ{RESET}"),
        Some(_) => format!("{GREEN}{ip}{RESET}"),
        None => ip.to_string(),
    }
}

fn fmt_result(r: &Result<(String, u32), String>, real: &Option<String>) -> String {
    match r {
        Ok((ip, rtt)) => format!("{:<28} {:>4}ms", tag(ip, real), rtt),
        Err(e) => format!("{YELLOW}{e:<28.28}{RESET}     -"),
    }
}

#[derive(Default)]
struct Options {
    interval_ms: u64,
    duration: Option<Duration>,
    real: Option<String>,
    json: bool,
    expected_tcp_ip: Option<String>,
    expected_udp_ip: Option<String>,
    forbidden_ip: Option<String>,
    max_outage_ms: Option<u64>,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options {
        interval_ms: 1000,
        ..Default::default()
    };
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--interval-ms" => {
                options.interval_ms = parse_next(&mut args, "--interval-ms")?;
            }
            "--duration" => {
                options.duration = Some(Duration::from_secs(parse_next(&mut args, "--duration")?));
            }
            "--real" => options.real = Some(next_value(&mut args, "--real")?),
            "--json" => options.json = true,
            "--expect-tcp-ip" => {
                options.expected_tcp_ip = Some(next_value(&mut args, "--expect-tcp-ip")?);
            }
            "--expect-udp-ip" => {
                options.expected_udp_ip = Some(next_value(&mut args, "--expect-udp-ip")?);
            }
            "--forbid-ip" => {
                options.forbidden_ip = Some(next_value(&mut args, "--forbid-ip")?);
            }
            "--expect-no-outage-ms" => {
                options.max_outage_ms = Some(parse_next(&mut args, "--expect-no-outage-ms")?);
            }
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    let has_assertions = options.expected_tcp_ip.is_some()
        || options.expected_udp_ip.is_some()
        || options.forbidden_ip.is_some()
        || options.max_outage_ms.is_some();
    if has_assertions && options.duration.is_none() {
        options.duration = Some(Duration::from_secs(20));
    }
    Ok(options)
}

fn next_value(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn parse_next<T: std::str::FromStr>(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<T, String> {
    next_value(arguments, option)?
        .parse()
        .map_err(|_| format!("{option} has an invalid value"))
}

#[derive(Serialize)]
struct JsonSample<'a> {
    kind: &'static str,
    elapsed_ms: u64,
    tcp_ip: Option<&'a str>,
    tcp_rtt_ms: Option<u32>,
    tcp_error: Option<&'a str>,
    udp_ip: Option<&'a str>,
    udp_rtt_ms: Option<u32>,
    udp_error: Option<&'a str>,
}

#[derive(Serialize)]
struct JsonSummary<'a> {
    kind: &'static str,
    passed: bool,
    samples: u64,
    tcp_successes: u64,
    udp_successes: u64,
    max_tcp_outage_ms: u64,
    max_udp_outage_ms: u64,
    violations: &'a [String],
}

#[derive(Default)]
struct PathStats {
    successes: u64,
    outage_started: Option<Instant>,
    max_outage_ms: u64,
}

impl PathStats {
    fn observe(&mut self, success: bool, now: Instant) {
        if success {
            self.successes += 1;
            self.close_outage(now);
        } else if self.outage_started.is_none() {
            self.outage_started = Some(now);
        }
    }

    fn close_outage(&mut self, now: Instant) {
        if let Some(started) = self.outage_started.take() {
            self.max_outage_ms = self
                .max_outage_ms
                .max(now.duration_since(started).as_millis() as u64);
        }
    }
}

fn check_ip(
    path: &str,
    result: &Result<(String, u32), String>,
    expected: Option<&str>,
    forbidden: Option<&str>,
    violations: &mut Vec<String>,
) {
    let Ok((ip, _)) = result else {
        return;
    };
    if let Some(expected) = expected {
        if expected != ip {
            violations.push(format!("{path} returned {ip}, expected {expected}"));
        }
    }
    if forbidden.is_some_and(|value| value == ip) {
        violations.push(format!("{path} exposed forbidden IP {ip}"));
    }
}

fn run(options: Options) -> Result<bool, String> {
    let timeout = Duration::from_millis(2500);
    if !options.json {
        println!("{BOLD}leakcheck{RESET} — публичный IP + RTT по TCP и игровому UDP/STUN");
        println!(
            "интервал {}ms; таймауты при смене локации с киллсвитчем = НОРМА (блок держит)\n",
            options.interval_ms
        );
    }

    let start = Instant::now();
    let (mut last_tcp, mut last_udp): (Option<String>, Option<String>) = (None, None);
    let mut real = options.real.clone();
    let mut transaction_id: u16 = 0x1a2b;
    let mut tcp_stats = PathStats::default();
    let mut udp_stats = PathStats::default();
    let mut samples = 0_u64;
    let mut violations = Vec::new();
    loop {
        transaction_id = transaction_id.wrapping_add(1);
        let tcp = tcp_check(timeout);
        let udp = udp_check(transaction_id, timeout);
        let observed_at = Instant::now();
        samples += 1;
        tcp_stats.observe(tcp.is_ok(), observed_at);
        udp_stats.observe(udp.is_ok(), observed_at);
        check_ip(
            "TCP",
            &tcp,
            options.expected_tcp_ip.as_deref(),
            options.forbidden_ip.as_deref(),
            &mut violations,
        );
        check_ip(
            "UDP",
            &udp,
            options.expected_udp_ip.as_deref(),
            options.forbidden_ip.as_deref(),
            &mut violations,
        );

        // Первый удачный замер — базовый «реальный» IP (если не задан --real).
        if real.is_none() {
            if let Ok((ip, _)) = &tcp {
                if !options.json {
                    println!("{BOLD}базовый IP: {ip} — считаю его РЕАЛЬНЫМ.{RESET}");
                    println!("{YELLOW}если leakcheck запущен при УЖЕ подключённом VPN — задайте реальный адрес через --real <ip>{RESET}");
                }
                real = Some(ip.clone());
            }
        }

        if options.json {
            let sample = JsonSample {
                kind: "sample",
                elapsed_ms: start.elapsed().as_millis() as u64,
                tcp_ip: tcp.as_ref().ok().map(|value| value.0.as_str()),
                tcp_rtt_ms: tcp.as_ref().ok().map(|value| value.1),
                tcp_error: tcp.as_ref().err().map(String::as_str),
                udp_ip: udp.as_ref().ok().map(|value| value.0.as_str()),
                udp_rtt_ms: udp.as_ref().ok().map(|value| value.1),
                udp_error: udp.as_ref().err().map(String::as_str),
            };
            println!(
                "{}",
                serde_json::to_string(&sample).map_err(|error| error.to_string())?
            );
        } else {
            let elapsed = start.elapsed().as_secs_f32();
            println!(
                "[{elapsed:8.1}s] TCP {} | UDP {}",
                fmt_result(&tcp, &real),
                fmt_result(&udp, &real)
            );
            for (label, current, last) in
                [("TCP", &tcp, &mut last_tcp), ("UDP", &udp, &mut last_udp)]
            {
                if let Ok((ip, _)) = current {
                    if let Some(previous) = last.as_ref() {
                        if previous != ip {
                            println!(
                                "{RED}{BOLD}  !!! {label}: IP сменился {previous} -> {ip}{RESET}"
                            );
                        }
                    }
                    *last = Some(ip.clone());
                }
            }
        }

        if options
            .duration
            .is_some_and(|duration| start.elapsed() >= duration)
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(options.interval_ms));
    }

    let finished_at = Instant::now();
    tcp_stats.close_outage(finished_at);
    udp_stats.close_outage(finished_at);
    if options.expected_tcp_ip.is_some() && tcp_stats.successes == 0 {
        violations.push("TCP produced no successful samples".into());
    }
    if options.expected_udp_ip.is_some() && udp_stats.successes == 0 {
        violations.push("UDP produced no successful samples".into());
    }
    if let Some(limit) = options.max_outage_ms {
        if tcp_stats.max_outage_ms > limit {
            violations.push(format!(
                "TCP outage {}ms exceeded {}ms",
                tcp_stats.max_outage_ms, limit
            ));
        }
        if udp_stats.max_outage_ms > limit {
            violations.push(format!(
                "UDP outage {}ms exceeded {}ms",
                udp_stats.max_outage_ms, limit
            ));
        }
    }
    let passed = violations.is_empty();
    if options.json {
        println!(
            "{}",
            serde_json::to_string(&JsonSummary {
                kind: "summary",
                passed,
                samples,
                tcp_successes: tcp_stats.successes,
                udp_successes: udp_stats.successes,
                max_tcp_outage_ms: tcp_stats.max_outage_ms,
                max_udp_outage_ms: udp_stats.max_outage_ms,
                violations: &violations,
            })
            .map_err(|error| error.to_string())?
        );
    } else if !passed {
        for violation in &violations {
            eprintln!("{RED}FAIL: {violation}{RESET}");
        }
    }
    Ok(passed)
}

fn main() {
    let options = parse_options().unwrap_or_else(|error| {
        eprintln!("leakcheck: {error}");
        eprintln!(
            "usage: leakcheck [--interval-ms N] [--real IP] [--duration SEC] [--json] \
             [--expect-tcp-ip IP] [--expect-udp-ip IP] [--forbid-ip IP] \
             [--expect-no-outage-ms N]"
        );
        std::process::exit(2);
    });
    match run(options) {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(error) => {
            eprintln!("leakcheck: {error}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_stun_ipv4, stun_request, STUN_MAGIC};

    #[test]
    fn stun_binding_request_has_magic_and_transaction() {
        let transaction = [7_u8; 12];
        let request = stun_request(transaction);
        assert_eq!(&request[0..2], &[0, 1]);
        assert_eq!(&request[4..8], &STUN_MAGIC.to_be_bytes());
        assert_eq!(&request[8..20], &transaction);
    }

    #[test]
    fn parses_xor_mapped_ipv4_address() {
        let transaction = [9_u8; 12];
        let public = [93_u8, 184, 216, 34];
        let mut response = vec![0x01, 0x01, 0, 12];
        response.extend_from_slice(&STUN_MAGIC.to_be_bytes());
        response.extend_from_slice(&transaction);
        response.extend_from_slice(&[0, 0x20, 0, 8, 0, 1, 0, 0]);
        for (byte, mask) in public.into_iter().zip(STUN_MAGIC.to_be_bytes()) {
            response.push(byte ^ mask);
        }
        assert_eq!(
            parse_stun_ipv4(&response, transaction),
            Some("93.184.216.34".parse().unwrap())
        );
    }

    #[test]
    fn rejects_stun_transaction_mismatch() {
        assert_eq!(parse_stun_ipv4(&[0_u8; 40], [1_u8; 12]), None);
    }
}
