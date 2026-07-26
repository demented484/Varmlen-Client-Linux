# Changelog

## 0.2.0

### Security

- Replaced GUI-owned privileged networking with an authenticated, root-owned
  daemon and a bounded command protocol.
- Removed file capabilities and arbitrary privileged command/config paths.
- Made reconnect transactional and fail-closed, including crash recovery.
- Redirected system DNS into Xray and blocked direct DNS/DoT leakage even when
  LAN access is enabled, without requiring systemd-resolved.
- Added strict ownership, mode, listener, and configuration validation for
  privileged components.

### Split tunneling

- Applied bypass marks to both TCP and UDP sockets.
- Added cgroup-v2 process-tree tracking for native games and launchers.
- Added executable-open tracking for Proton/Windows game binaries.
- Added Flatpak command resolution and safe handling of existing sockets.

### Reliability

- Kept the tunnel alive independently of GUI restarts.
- Ignored stale frontend connection results after a newer disconnect/reconnect.
- Added daemon state recovery after GUI, Xray, or daemon crashes.
- Replaced per-ping Xray process launches with lightweight TCP probes.

### Packaging

- Added root-owned daemon, network helper, Xray, and polkit policy to DEB/RPM
  layouts.
- Removed duplicate privileged binaries from application resources.
- AppImage publishing is paused until a safe one-time daemon installer exists.
