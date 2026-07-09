//! leakcheck — диагностика split-tunneling и утечек для Varmlen.
//!
//! Каждый цикл делает ДВЕ независимые проверки и печатает публичный IP + RTT:
//!   - TCP: HTTP-запрос к api.ipify.org (путь браузера / обычных приложений).
//!     Каждый цикл — НОВОЕ соединение, так что строка отражает текущую
//!     маршрутизацию, а не старый установленный поток.
//!   - UDP: DNS-запрос `myip.opendns.com A` напрямую к resolver1.opendns.com —
//!     OpenDNS отвечает адресом, с которого пришёл запрос. Это путь игр (UDP)
//!     и заодно детектор DNS-перехвата: если запрос уехал в туннель и его
//!     разрешил чужой резолвер, вернётся НЕ ваш адрес.
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

const HTTP_HOST: &str = "api.ipify.org";
/// resolver1.opendns.com — намеренно IP-литерал: сам замер не должен зависеть
/// от системного DNS (и не входит в allowlist киллсвитча, в отличие от 1.1.1.1).
const DNS_SERVER: &str = "208.67.222.222:53";
const MYIP_QNAME: &str = "myip.opendns.com";

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
    write!(s, "GET / HTTP/1.0\r\nHost: {HTTP_HOST}\r\nUser-Agent: leakcheck\r\n\r\n")
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

/// Сформировать DNS-запрос `<qname> A IN` с заданным id.
fn dns_query(id: u16, qname: &str) -> Vec<u8> {
    let mut q = Vec::with_capacity(12 + qname.len() + 6);
    q.extend_from_slice(&id.to_be_bytes());
    q.extend_from_slice(&[0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]); // RD, QDCOUNT=1
    for label in qname.split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.extend_from_slice(&[0, 0, 1, 0, 1]); // root, TYPE=A, CLASS=IN
    q
}

/// Первый A-record из ответа (вопрос пропускается; имя — метки или указатель).
fn parse_a_record(p: &[u8]) -> Option<Ipv4Addr> {
    if p.len() < 12 {
        return None;
    }
    let qd = u16::from_be_bytes([p[4], p[5]]) as usize;
    let an = u16::from_be_bytes([p[6], p[7]]) as usize;
    let mut i = 12;
    for _ in 0..qd {
        while i < p.len() && p[i] != 0 && p[i] & 0xc0 != 0xc0 {
            i += p[i] as usize + 1;
        }
        i += if i < p.len() && p[i] & 0xc0 == 0xc0 { 2 } else { 1 };
        i += 4; // QTYPE + QCLASS
    }
    for _ in 0..an {
        if i >= p.len() {
            return None;
        }
        if p[i] & 0xc0 == 0xc0 {
            i += 2;
        } else {
            while i < p.len() && p[i] != 0 {
                i += p[i] as usize + 1;
            }
            i += 1;
        }
        if i + 10 > p.len() {
            return None;
        }
        let typ = u16::from_be_bytes([p[i], p[i + 1]]);
        let rdlen = u16::from_be_bytes([p[i + 8], p[i + 9]]) as usize;
        i += 10;
        if typ == 1 && rdlen == 4 && i + 4 <= p.len() {
            return Some(Ipv4Addr::new(p[i], p[i + 1], p[i + 2], p[i + 3]));
        }
        i += rdlen;
    }
    None
}

/// Публичный IP + RTT по UDP: myip.opendns.com у OpenDNS = адрес источника запроса.
fn udp_check(id: u16, timeout: Duration) -> Result<(String, u32), String> {
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("bind: {e}"))?;
    sock.set_read_timeout(Some(timeout)).ok();
    let q = dns_query(id, MYIP_QNAME);
    let started = Instant::now();
    sock.send_to(&q, DNS_SERVER).map_err(|e| format!("send: {e}"))?;
    let mut buf = [0u8; 512];
    let (n, _) = sock.recv_from(&mut buf).map_err(|e| format!("recv: {e}"))?;
    let rtt = started.elapsed().as_millis() as u32;
    if n < 2 || buf[..2] != id.to_be_bytes() {
        return Err("bad dns reply".into());
    }
    // myip.opendns.com отвечает адресом ТОЛЬКО при прямом запросе к OpenDNS.
    // Пустой ответ = запрос перехватил другой резолвер по пути — так выглядит
    // UDP:53 через туннель (xray hijack). Для исключённого приложения это
    // сигнал «обход НЕ работает», для неисключённого — норма.
    parse_a_record(&buf[..n])
        .map(|ip| (ip.to_string(), rtt))
        .ok_or_else(|| "перехвачен (туннель?)".into())
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

fn main() {
    let mut interval_ms: u64 = 1000;
    let mut real: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--interval-ms" => interval_ms = args.next().and_then(|v| v.parse().ok()).unwrap_or(1000),
            "--real" => real = args.next(),
            _ => {
                eprintln!("usage: leakcheck [--interval-ms N] [--real <ip>]");
                std::process::exit(2);
            }
        }
    }
    let timeout = Duration::from_millis(2500);
    println!("{BOLD}leakcheck{RESET} — публичный IP + RTT по TCP (браузерный путь) и UDP (игровой путь)");
    println!("интервал {interval_ms}ms; таймауты при смене локации с киллсвитчем = НОРМА (блок держит)\n");

    let start = Instant::now();
    let (mut last_tcp, mut last_udp): (Option<String>, Option<String>) = (None, None);
    let mut dns_id: u16 = 0x1a2b;
    loop {
        dns_id = dns_id.wrapping_add(1);
        let tcp = tcp_check(timeout);
        let udp = udp_check(dns_id, timeout);

        // Первый удачный замер — базовый «реальный» IP (если не задан --real).
        if real.is_none() {
            if let Ok((ip, _)) = &tcp {
                println!("{BOLD}базовый IP: {ip} — считаю его РЕАЛЬНЫМ.{RESET}");
                println!("{YELLOW}если leakcheck запущен при УЖЕ подключённом VPN — это IP локации, а не реальный; тогда перезапустите без VPN или задайте --real <ip>{RESET}");
                real = Some(ip.clone());
            }
        }

        let t = start.elapsed().as_secs_f32();
        println!(
            "[{t:8.1}s] TCP {} | UDP {}",
            fmt_result(&tcp, &real),
            fmt_result(&udp, &real)
        );
        for (label, cur, last) in [("TCP", &tcp, &mut last_tcp), ("UDP", &udp, &mut last_udp)] {
            if let Ok((ip, _)) = cur {
                if let Some(prev) = last.as_ref() {
                    if prev != ip {
                        println!("{RED}{BOLD}  !!! {label}: IP сменился {prev} -> {ip}{RESET}");
                    }
                }
                *last = Some(ip.clone());
            }
        }
        std::thread::sleep(Duration::from_millis(interval_ms));
    }
}

#[cfg(test)]
mod tests {
    use super::{dns_query, parse_a_record};

    #[test]
    fn query_shape() {
        let q = dns_query(0xabcd, "myip.opendns.com");
        assert_eq!(&q[..2], &[0xab, 0xcd]);
        assert_eq!(q[2], 0x01); // RD
        assert_eq!(&q[12..17], &[4, b'm', b'y', b'i', b'p']);
        assert_eq!(&q[q.len() - 4..], &[0, 1, 0, 1]); // A IN
    }

    #[test]
    fn parse_answer_with_pointer_name() {
        // header: id=1, QR, qd=1 an=1 + question myip.opendns.com A IN +
        // answer: ptr to 0x0c, TYPE A, CLASS IN, TTL 0, RDLEN 4, 93.184.216.34
        let mut p = vec![0, 1, 0x81, 0x80, 0, 1, 0, 1, 0, 0, 0, 0];
        for label in ["myip", "opendns", "com"] {
            p.push(label.len() as u8);
            p.extend_from_slice(label.as_bytes());
        }
        p.extend_from_slice(&[0, 0, 1, 0, 1]);
        p.extend_from_slice(&[0xc0, 0x0c, 0, 1, 0, 1, 0, 0, 0, 0, 0, 4, 93, 184, 216, 34]);
        assert_eq!(parse_a_record(&p), Some("93.184.216.34".parse().unwrap()));
    }

    #[test]
    fn parse_garbage_is_none() {
        assert_eq!(parse_a_record(&[0u8; 5]), None);
        assert_eq!(parse_a_record(&[0u8; 40]), None);
    }
}
