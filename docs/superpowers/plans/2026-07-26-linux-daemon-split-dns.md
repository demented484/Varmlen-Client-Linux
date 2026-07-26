# Linux Daemon, Split Tunnelling, and DNS Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the user-owned capability helper with a portable root-owned daemon that provides fail-closed reconnect, DNS interception, crash recovery, and verified TCP/UDP application exclusions.

**Architecture:** A root-owned `varmlend` process exclusively owns Xray, TUN, nftables, DNS, cgroup/BPF, and recovery state. The GUI becomes an authenticated Unix-socket client. The daemon core is independent of the init system; packaging supplies optional launch adapters.

**Tech Stack:** Rust, Tokio, Unix domain sockets, `SO_PEERCRED`, nftables, Linux cgroup v2/BPF, Xray, Tauri, shell package hooks.

## Global Constraints

- No executable in a user-writable directory receives Linux capabilities.
- The daemon must not depend on systemd, NetworkManager, or systemd-resolved.
- Every reconnect installs and verifies a hold-block before tearing down the old tunnel.
- DNS interception failure is fatal; `/etc/resolv.conf` is never rewritten.
- Split exclusions cover TCP, UDP, existing processes, new processes, descendants, Steam launchers, and Flatpak process trees.
- User-provided paths are never resolved outside a daemon-owned root.
- All behavior changes use red-green-refactor and end in independently reviewable commits.

---

### Task 1: Versioned daemon protocol and peer authentication

**Files:**
- Create: `daemon/Cargo.toml`
- Create: `daemon/src/lib.rs`
- Create: `daemon/src/protocol.rs`
- Create: `daemon/src/server.rs`
- Create: `src-tauri/src/daemon_client.rs`
- Modify: `Cargo.toml`
- Modify: `src-tauri/Cargo.toml`
- Test: `daemon/src/server.rs`

**Interfaces:**
- Produces: `RequestEnvelope { version: u16, operation_id: u64, command: DaemonCommand }`
- Produces: `ResponseEnvelope { version: u16, operation_id: u64, result: Result<DaemonState, DaemonError> }`
- Produces: `DaemonClient::connect(path: &Path) -> Result<Self, ClientError>`
- Produces: `DaemonClient::request(command: DaemonCommand) -> Result<DaemonState, ClientError>`

- [ ] **Step 1: Write failing protocol and peer-policy tests**

```rust
#[test]
fn rejects_unknown_protocol_version() {
    let request = RequestEnvelope::new(PROTOCOL_VERSION + 1, 7, DaemonCommand::Status);
    assert_eq!(validate_request(&request), Err(DaemonErrorCode::UnsupportedVersion));
}

#[test]
fn peer_policy_accepts_only_configured_uid() {
    assert!(PeerPolicy::new(1000).authorize(1000));
    assert!(!PeerPolicy::new(1000).authorize(1001));
    assert!(!PeerPolicy::new(1000).authorize(0));
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test --manifest-path daemon/Cargo.toml protocol peer_policy`

Expected: compilation fails because protocol and peer-policy types do not exist.

- [ ] **Step 3: Implement protocol framing and peer checks**

```rust
pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub version: u16,
    pub operation_id: u64,
    pub command: DaemonCommand,
}

pub struct PeerPolicy {
    allowed_uid: u32,
}

impl PeerPolicy {
    pub fn authorize(&self, uid: u32) -> bool {
        uid == self.allowed_uid
    }
}
```

Use length-prefixed JSON with a 1 MiB frame limit and obtain peer credentials
from the accepted Unix socket before reading a request.

- [ ] **Step 4: Verify GREEN and full crate tests**

Run: `cargo test --manifest-path daemon/Cargo.toml`

Expected: all daemon protocol and authentication tests pass.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml daemon src-tauri/Cargo.toml src-tauri/src/daemon_client.rs
git commit -m "Add authenticated daemon protocol"
```

### Task 2: Root-owned installation and portable daemon startup

**Files:**
- Create: `packaging/varmlend/varmlend.polkit.policy`
- Create: `packaging/varmlend/systemd/varmlend.service`
- Create: `packaging/varmlend/openrc/varmlend`
- Create: `packaging/varmlend/runit/run`
- Create: `packaging/varmlend/s6/run`
- Create: `scripts/install-varmlend.sh`
- Modify: `src-tauri/tauri.conf.json`
- Delete: `helper/varmlen-setcap.sh`
- Test: `scripts/test-install-layout.sh`

**Interfaces:**
- Consumes: daemon binary `varmlend`
- Produces: `/usr/libexec/varmlen/varmlend`, `/run/varmlen/daemon.sock`
- Produces: fixed polkit action `app.varmlen.client.start-daemon`

- [ ] **Step 1: Write a failing package-layout test**

```sh
test -x "$ROOT/usr/libexec/varmlen/varmlend"
test "$(stat -c %U "$ROOT/usr/libexec/varmlen/varmlend")" = root
test "$(stat -c %a "$ROOT/usr/libexec/varmlen/varmlend")" = 755
! getcap "$ROOT/usr/libexec/varmlen/varmlend" | grep -q .
! find "$ROOT/home" -type f -exec getcap {} + | grep -q .
```

- [ ] **Step 2: Verify RED**

Run: `sh scripts/test-install-layout.sh`

Expected: failure because the root-owned daemon layout is not installed.

- [ ] **Step 3: Implement deterministic installation and adapters**

The installer accepts `DESTDIR`, uses `install -o root -g root -m 0755`, and
never executes a source path supplied by the GUI. Adapter scripts only execute
the fixed `/usr/libexec/varmlen/varmlend` path.

- [ ] **Step 4: Verify GREEN**

Run: `sh scripts/test-install-layout.sh`

Expected: all ownership, mode, capability, and adapter checks pass.

- [ ] **Step 5: Commit**

```bash
git add packaging scripts src-tauri/tauri.conf.json helper/varmlen-setcap.sh
git commit -m "Install root-owned portable VPN daemon"
```

### Task 3: Transactional connection state machine

**Files:**
- Create: `daemon/src/state.rs`
- Create: `daemon/src/connection.rs`
- Create: `daemon/src/command.rs`
- Modify: `daemon/src/lib.rs`
- Test: `daemon/src/connection.rs`

**Interfaces:**
- Produces: `ConnectionManager<B: NetworkBackend>`
- Produces: `NetworkBackend::install_hold_block`, `verify_hold_block`, `prepare_tunnel`, `commit_tunnel`, `remove_old_tunnel`
- Produces: `ConnectionPhase::{Disconnected, Preparing, Connected, Blocking, Reconnecting, RecoveryRequired}`

- [ ] **Step 1: Write a failing reconnect preservation test**

```rust
#[tokio::test]
async fn failed_hold_block_preserves_old_tunnel() {
    let backend = FakeBackend::connected().fail_on(Action::InstallHoldBlock);
    let mut manager = ConnectionManager::new(backend);
    let error = manager.reconnect(test_config()).await.unwrap_err();
    assert_eq!(error.code, DaemonErrorCode::HoldBlockFailed);
    assert!(manager.backend().old_tunnel_is_running());
    assert!(!manager.backend().old_tunnel_was_removed());
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test --manifest-path daemon/Cargo.toml failed_hold_block_preserves_old_tunnel`

Expected: failure because `ConnectionManager` is absent.

- [ ] **Step 3: Implement the minimal fail-closed state machine**

```rust
pub async fn reconnect(&mut self, config: TunnelConfig) -> Result<DaemonState, DaemonError> {
    self.backend.install_hold_block().await?;
    self.backend.verify_hold_block().await?;
    let prepared = self.backend.prepare_tunnel(config).await?;
    self.backend.commit_tunnel(&prepared).await?;
    self.backend.remove_old_tunnel().await?;
    self.backend.remove_hold_block().await?;
    self.phase = ConnectionPhase::Connected;
    Ok(self.snapshot())
}
```

Add tests for failure at every boundary and assert either the old tunnel or the
verified block remains active.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test --manifest-path daemon/Cargo.toml connection`

Expected: all state-transition and injected-failure tests pass.

- [ ] **Step 5: Commit**

```bash
git add daemon/src
git commit -m "Make VPN reconnect transactional"
```

### Task 4: Portable DNS interception

**Files:**
- Create: `daemon/src/dns.rs`
- Create: `daemon/src/nft.rs`
- Modify: `daemon/src/connection.rs`
- Modify: `src-tauri/src/xray.rs`
- Test: `daemon/src/dns.rs`
- Test: `daemon/tests/dns_namespace.rs`

**Interfaces:**
- Produces: `DnsGuard::install(DnsPlan) -> Result<DnsLease, DaemonError>`
- Produces: `DnsLease::verify() -> Result<(), DaemonError>`
- Produces: nft chain `varmlen_dns_output`

- [ ] **Step 1: Write failing DNS policy tests**

```rust
#[test]
fn dns_rules_precede_lan_and_split_accept_rules() {
    let rules = render_dns_rules(test_plan());
    assert!(position(&rules, "udp dport 53 redirect") < position(&rules, "allow_lan"));
    assert!(position(&rules, "tcp dport 53 redirect") < position(&rules, "meta mark"));
    assert!(rules.contains("tcp dport 853 reject"));
}

#[tokio::test]
async fn failed_dns_verification_aborts_connection() {
    let backend = FakeBackend::connected().fail_on(Action::VerifyDns);
    assert_eq!(
        ConnectionManager::new(backend).connect(test_config()).await.unwrap_err().code,
        DaemonErrorCode::DnsVerificationFailed
    );
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test --manifest-path daemon/Cargo.toml dns`

Expected: failure because DNS policy and verification do not exist.

- [ ] **Step 3: Implement DNS inbound, redirect, and verification**

Render one atomic nft transaction that redirects TCP/UDP 53 before any LAN or
split accept, rejects physical/LAN port 53 escape, and rejects direct TCP 853.
Verify the local inbound is listening and a probe resolves through the tunnel
before reporting `Connected`. Remove all `/etc/resolv.conf` mutation.

- [ ] **Step 4: Verify unit and network-namespace tests**

Run: `cargo test --manifest-path daemon/Cargo.toml dns`

Run as root in CI/device test: `cargo test --manifest-path daemon/Cargo.toml --test dns_namespace -- --ignored`

Expected: unit tests pass; namespace test observes TCP and UDP DNS only at the
local inbound and no packets on the simulated physical DNS peer.

- [ ] **Step 5: Commit**

```bash
git add daemon/src src-tauri/src/xray.rs daemon/tests
git commit -m "Route system DNS through the tunnel"
```

### Task 5: Safe cgroup/BPF split tunnelling

**Files:**
- Create: `daemon/src/split/mod.rs`
- Create: `daemon/src/split/cgroup.rs`
- Create: `daemon/src/split/process.rs`
- Create: `daemon/src/split/bpf.rs`
- Modify: `daemon/src/connection.rs`
- Delete: `src-tauri/src/split_bypass.rs`
- Test: `daemon/src/split/cgroup.rs`
- Test: `daemon/tests/split_namespace.rs`

**Interfaces:**
- Produces: `SplitManager::apply(SplitPlan) -> Result<SplitLease, DaemonError>`
- Produces: `SplitStatus::{Active, Disabled}`
- Consumes executable identity and Flatpak application ID, never a raw cgroup path

- [ ] **Step 1: Write failing traversal and degraded-state tests**

```rust
#[test]
fn rejects_parent_and_symlink_escape() {
    assert_eq!(ValidatedComponent::new(".."), Err(SplitError::InvalidComponent));
    assert_eq!(ValidatedComponent::new("steam/../system.slice"), Err(SplitError::InvalidComponent));
}

#[tokio::test]
async fn watcher_failure_never_reports_active() {
    let backend = FakeSplitBackend::fail_watcher();
    assert_eq!(
        SplitManager::new(backend).apply(test_plan()).await.unwrap_err(),
        SplitError::WatcherUnavailable
    );
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test --manifest-path daemon/Cargo.toml split`

Expected: failure because safe split types do not exist.

- [ ] **Step 3: Implement daemon-owned cgroups and reconciliation**

Use fixed `/sys/fs/cgroup/varmlen.slice/user-<uid>/` roots, component validation,
descriptor-relative traversal with no symlinks, and daemon-derived paths. Move
matching existing processes and monitor exec/fork activity. Periodically
reconcile `/proc` so connector loss cannot silently miss new game processes.
Return an error rather than `Active` when BPF or monitoring cannot be verified.

- [ ] **Step 4: Verify TCP/UDP process-tree behavior**

Run: `cargo test --manifest-path daemon/Cargo.toml split`

Run as root: `cargo test --manifest-path daemon/Cargo.toml --test split_namespace -- --ignored`

Expected: an excluded parent and children use the physical test namespace for
both TCP and UDP before/after VPN connect; non-excluded controls use TUN.

- [ ] **Step 5: Commit**

```bash
git add daemon/src daemon/tests src-tauri/src/split_bypass.rs
git commit -m "Enforce safe TCP and UDP app exclusions"
```

### Task 6: Crash recovery and verified cleanup

**Files:**
- Create: `daemon/src/recovery.rs`
- Modify: `daemon/src/state.rs`
- Modify: `daemon/src/connection.rs`
- Modify: `daemon/src/server.rs`
- Test: `daemon/src/recovery.rs`

**Interfaces:**
- Produces: `RecoveryManager::reconcile() -> Result<RecoveryReport, DaemonError>`
- Produces: `ProcessIdentity { pid: u32, start_time_ticks: u64, executable: PathBuf }`
- Produces: `CleanupReport { removed: Vec<Resource>, remaining: Vec<Resource> }`

- [ ] **Step 1: Write failing stale-process and partial-cleanup tests**

```rust
#[test]
fn pid_reuse_does_not_kill_unrelated_process() {
    let saved = ProcessIdentity::new(42, 100, "/usr/libexec/varmlen/xray");
    let live = ProcessIdentity::new(42, 101, "/usr/bin/unrelated");
    assert!(!saved.matches(&live));
}

#[tokio::test]
async fn remaining_kernel_state_reports_recovery_required() {
    let backend = FakeBackend::with_stubborn(Resource::NftTable);
    let report = RecoveryManager::new(backend).cleanup().await.unwrap();
    assert_eq!(report.remaining, vec![Resource::NftTable]);
    assert_eq!(report.phase(), ConnectionPhase::RecoveryRequired);
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test --manifest-path daemon/Cargo.toml recovery`

Expected: failure because recovery types do not exist.

- [ ] **Step 3: Implement durable identity and reconciliation**

Persist state atomically under `/var/lib/varmlen`, compare PID plus
`/proc/<pid>/stat` start time and executable, inspect actual network resources,
and verify cleanup postconditions. Never report `Disconnected` while resources
remain.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test --manifest-path daemon/Cargo.toml recovery`

Expected: all recovery and cleanup tests pass.

- [ ] **Step 5: Commit**

```bash
git add daemon/src
git commit -m "Recover VPN state after process crashes"
```

### Task 7: Migrate the GUI to daemon-owned operations

**Files:**
- Modify: `src-tauri/src/vpn.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/daemon_client.rs`
- Modify: `src/lib/conn.svelte.ts`
- Test: `src-tauri/src/vpn.rs`
- Test: `src/lib/conn.test.ts`

**Interfaces:**
- Consumes: `DaemonClient::request`
- Produces: Tauri commands that translate daemon snapshots without spawning Xray
- Produces: frontend operation generation `connectionGeneration: number`

- [ ] **Step 1: Write failing stale-operation and no-local-helper tests**

```typescript
it("does not reconnect after an explicit disconnect", async () => {
  const state = createConnectionStore(fakeBackend);
  state.onConfigChanged();
  await state.disconnect();
  await timers.advanceByTimeAsync(500);
  expect(fakeBackend.connect).not.toHaveBeenCalled();
});
```

```rust
#[test]
fn vpn_command_uses_daemon_client_only() {
    assert!(!include_str!("vpn.rs").contains("setcap"));
    assert!(!include_str!("vpn.rs").contains("Command::new(xray"));
}
```

- [ ] **Step 2: Verify RED**

Run: `npm test -- conn.test.ts`

Run: `cargo test --manifest-path src-tauri/Cargo.toml vpn_command_uses_daemon_client_only`

Expected: tests fail against the current local-helper and timer behavior.

- [ ] **Step 3: Implement GUI migration and operation generations**

All connect, reconnect, disconnect, status, and recovery calls go through the
daemon client. Explicit disconnect cancels pending reapply timers and increments
the operation generation; stale completions are discarded.

- [ ] **Step 4: Verify GREEN**

Run: `npm test -- conn.test.ts`

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: frontend race tests and all Rust tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src src/lib/conn.svelte.ts src/lib/conn.test.ts
git commit -m "Use daemon for Linux VPN lifecycle"
```

### Task 8: Host-safe Linux release gate

**Files:**
- Create: `scripts/test-linux-package.sh`
- Modify: `tools/leakcheck/src/main.rs`
- Modify: `README.md`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: built package, injected daemon backends, and deterministic fixtures
- Produces: package-layout and regression-test results without touching host networking

- [x] **Step 1: Add machine-readable leakcheck assertions**

```sh
leakcheck --json --duration 20 \
  --expect-tcp-ip "$REAL_IP" \
  --expect-udp-ip "$REAL_IP" \
  --expect-no-outage-ms 1500
```

The diagnostic remains available for an operator who explicitly chooses to run
it, but it is never invoked by the automated release gate.

- [x] **Step 2: Add a failing package-layout check**

Run: `scripts/test-linux-package.sh target/release/bundle/deb/Varmlen_0.2.0_amd64.deb`

Expected before the packaging fix: failure because privileged binaries are
duplicated under `/usr/lib/Varmlen`.

- [x] **Step 3: Complete the host-safe gate**

Cover TCP/UDP marking, descendants, Proton executable matching, Flatpak command
resolution, reconnect ordering, DNS/LAN policy, recovery, package ownership,
modes, capabilities, and duplicate privileged payloads through unit tests and
archive inspection. Do not call `ip`, `nft`, `resolvectl`, public-IP services,
or VPN lifecycle commands on the host.

- [x] **Step 4: Run the complete Linux gate**

Run: `cargo test --workspace --locked`

Run: `cargo clippy --workspace --all-targets --locked -- -D warnings`

Run: `npm run check && npm run build`

Run: `scripts/test-install-layout.sh`

Run: `scripts/test-linux-package.sh target/release/bundle/deb/Varmlen_0.2.0_amd64.deb`

Expected: all deterministic checks pass without changing or bypassing the
developer host's active VPN.

- [ ] **Step 5: Commit**

```bash
git add scripts tests README.md CHANGELOG.md
git commit -m "Add Linux VPN security release gate"
```
