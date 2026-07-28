# Location rows, editors, and subscription refresh

## Scope

The same location-list and editor behavior is required in the Linux and
Android clients. Background refresh while the application is closed is Android
only. The Linux daemon and VPN lifecycle are out of scope.

## Location list

- Draw a visible one-pixel divider between adjacent locations. The divider is
  inset past the location icon so it separates content without boxing rows.
- Remove the selected row's leading accent stripe. Selection is communicated
  only by the existing row background.
- When a location has no country flag, show a neutral gray globe SVG. It has
  one outer circle and exactly four inner arcs: two meridians and two latitude
  arcs.

## Location editor

The editor mode is determined by the location source:

- A provider JSON location shows only its exact editable JSON source.
- A share-link/non-JSON location shows only structured fields. The form covers
  the label, protocol, host, port, protocol credentials, transport, security,
  and conditional transport/security fields. Extra query parameters are
  editable key/value rows.
- The generated internal `VlessServer` object is never presented as JSON.

Saving is permissive. The exact field draft or JSON text is persisted even when
it cannot currently be parsed into a working Xray outbound. A valid draft
updates the parsed location immediately. An invalid draft remains visible in
the editor and causes a clear configuration error if the user tries to connect
with it; the application must not silently use an older configuration.

Each `ServerEntry` therefore retains both the last parsed location and an
optional persisted edit draft. Code that prepares a connection must compile the
draft first and stop with its parse/configuration error when compilation fails.
List presentation may use directly readable draft values such as the edited
label, while JSON that cannot be parsed keeps the last known row label.

An explicit or automatic provider refresh is authoritative: it replaces local
location drafts, including edited JSON, with the provider response.

## Auto-refresh setting

Add one global “Automatic subscription updates” switch to Settings. It defaults
to enabled for existing and new installations.

Turning it off:

- cancels Android background work;
- cancels frontend timers;
- leaves manual Refresh available;
- does not delete cached subscriptions or local edits.

Turning it on registers current remote subscriptions again and calculates their
next update from the most recent successful provider refresh.

## Android background refresh

Use Android WorkManager, independent of `VarmlenVpnService`, so updates continue
with the UI process closed and with no active VPN.

The frontend synchronizes each remote subscription's stable ID, URL, selected
User-Agent, advertised interval, and next due time to a native private schedule.
Android creates unique one-time work per subscription with a network
constraint. After a successful request, the worker stores the response body and
relevant subscription headers in app-private storage, updates the interval when
the provider advertises a new one, and schedules the next run. Android may
delay work under Doze; no exact-alarm permission is requested.

The worker does not launch the UI, reconnect the VPN, log subscription URLs or
contents, or require notification permission. On the next normal foreground
start, the frontend drains staged responses and applies them through the same
parser and atomic replacement path as manual Refresh. A failed request keeps
the old subscription, uses WorkManager backoff, and records only a bounded,
non-sensitive error status for display after the next launch.

Opening the application does not itself perform a network refresh.

## Linux refresh timing

Remove the immediate `refreshDue()` call from application mount. While the
client process is running, schedule the next future interval boundary with a
one-shot timer and reschedule after every result. If the process was closed
during one or more boundaries, those runs are skipped rather than replayed at
the next launch.

## State and update flow

1. Import/manual refresh parses and atomically stores the provider response.
2. The successful refresh timestamp and interval determine `nextUpdateAt`.
3. Android receives or cancels native work whenever subscriptions, User-Agent,
   interval, or the global setting changes.
4. A background response is staged natively and parsed only through the existing
   trusted Rust parsing path.
5. Applying a provider response clears per-location edit drafts, reconciles the
   selected stable location key, and does not reconnect a running VPN.

## Tests and verification

- Unit-test next-update calculation, skipped Linux intervals, toggle behavior,
  and authoritative overwrite of edited locations.
- Unit-test field-draft and JSON-draft persistence, valid compilation, and
  invalid-draft connection errors.
- Contract-test the visible dividers, absent selection stripe, conditional
  editor modes, and four-arc fallback globe.
- Unit-test Android schedule serialization, cancellation, staged-response
  storage, and retry behavior without making real network requests.
- Build and lint both clients and build the signed Android APK.
- Verify the UI in local previews/screenshots. Do not connect, disconnect,
  install, or otherwise touch the user's active VPN.
