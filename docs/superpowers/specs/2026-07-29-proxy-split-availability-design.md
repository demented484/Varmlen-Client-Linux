# Proxy split availability and Linux permission cleanup

## Goal

Make the Linux UI accurately reflect what local Proxy mode can enforce:
per-application split tunnelling requires TUN, while per-site routing can still
be handled by Xray for traffic that applications send to the local proxy.
Remove the misleading manual network-permissions setup.

## User experience

- In Proxy mode, the `Apps` tab on the Split page remains visible but looks
  inactive and cannot change per-app settings.
- Hovering it with a mouse, focusing it with a keyboard, or pressing it on a
  touchscreen shows the same in-app explanation:
  `Per-app split tunnelling is unavailable in Proxy mode. Switch to TUN mode.`
- The inactive tab remains focusable so the explanation is accessible; it does
  not use the native `disabled` attribute and never changes the active tab.
- The explanation is a compact Varmlen-styled notice near the tabs, not a native
  browser or Android dialog. It disappears after a short delay.
- If the page is opened while Proxy mode is active, `Websites` is selected
  automatically. Switching back to TUN restores normal access to `Apps`.
- The `Websites` tab remains editable in both modes.

## Routing behavior

- TUN behavior is unchanged: app and site rules continue to be included in the
  native-TUN Xray configuration, and Linux per-app exclusions continue through
  the daemon split backend.
- Proxy mode continues to expose only the local SOCKS proxy; it does not
  attempt process discovery, transparent interception, or per-app routing.
- Proxy mode includes site rules in Xray routing:
  - `General`: listed sites go direct; other proxied traffic uses the VPN.
  - `Selective`: listed sites use the VPN; other traffic submitted to the local
    proxy goes direct.
- DNS for Xray routing continues through the existing DoH path.
- App entries remain stored while Proxy mode is active and take effect again
  when the user returns to TUN.

## Linux permission cleanup

- Remove the `Network permissions` settings card and its setup/status state.
- Remove the unused frontend API wrappers and Tauri commands that only started
  the privileged daemon while claiming to grant capabilities.
- Correct the mode label from `SOCKS/HTTP` to `SOCKS` and remove the `no root`
  claim; both modes use the installed privileged daemon for lifecycle
  management.
- Privilege escalation remains lazy: the daemon is started through the existing
  `pkexec` path when the first operation actually requires it.

## Error handling and accessibility

- Direct navigation to `/split` in Proxy mode cannot expose editable app
  controls because the page itself enforces availability.
- The inactive Apps tab has `aria-disabled="true"` and the notice is announced
  through a polite live region.
- Changing modes clears any stale notice and does not delete split settings.

## Verification

- Frontend tests cover the Proxy/TUN availability decision and source contracts
  for the disabled Apps tab, translated notice, and removed permission UI.
- Rust tests verify that Proxy configuration ignores app rules but honors both
  general and selective site rules.
- Existing TUN split, daemon protocol, DNS, and killswitch tests remain green.
- Run frontend checks/tests/build plus focused Rust tests without starting,
  stopping, reconnecting, or inspecting the user's active VPN.
