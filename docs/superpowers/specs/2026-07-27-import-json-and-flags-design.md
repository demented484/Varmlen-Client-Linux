# Import, JSON, Flags, and Platform UA

## Scope

Apply the same behaviour to the Linux and Android clients:

- render country flags as bundled rectangular SVG assets instead of emoji;
- split subscription import into clipboard, link, and JSON paths;
- make link entry a single-line input and keep JSON entry as a multiline editor;
- retain JSON returned by remote subscription URLs for preview and editing;
- identify the Varmlen platform in the subscription request User-Agent.

No VPN routing, DNS, kill-switch, or split-tunnelling behaviour changes.

## Flags

Use the MIT-licensed `flag-icons` package and bundle its SVG assets with the
application. Country flag emoji in server labels are converted to ISO country
codes and rendered as compact 4:3 images with a small radius. Non-country
leading symbols keep a text fallback, and entries without an icon keep the same
alignment.

The server name remains free of the leading emoji. The location-details header
uses the same flag component as the server row.

## Import flow

The initial Add subscription view has three actions:

1. **Paste from clipboard** — keep the fast path and detect whether the pasted
   value is JSON or a link.
2. **Enter link** — show one text input for an HTTP(S) subscription URL or a
   supported share link. Enter submits it.
3. **Enter JSON** — show the existing multiline editor for an Xray config,
   outbound object, array, or JSON container supported by the parser.

The link and JSON modes have a quiet Back action. Errors stay in the selected
mode and never discard entered text.

## JSON sources and editing

The backend marks imports whose actual payload is JSON and returns the original
JSON source. This applies to both pasted JSON and HTTP(S) endpoints that return
JSON. URL-fetched JSON is parsed as JSON instead of being passed to the
line-based share-link parser.

Each subscription persists `sourceJson` and `jsonEdited`:

- `sourceJson` is the latest valid JSON payload;
- `jsonEdited` is true after a local edit to JSON fetched from a URL.

Subscriptions with JSON expose **View JSON** in their menu. The editor opens
with formatted JSON, supports direct editing, and saves only after parsing
succeeds and at least one usable server is found. Saving atomically replaces
the source JSON and parsed server list; invalid JSON leaves the subscription
unchanged and shows a specific error.

For pasted JSON, Save also replaces the subscription's local source. For remote
JSON, Save keeps the original URL but marks the subscription locally edited.
Automatic refresh skips locally edited remote JSON so it cannot silently erase
changes. An explicit normal Refresh fetches the URL again, replaces the local
JSON, and clears `jsonEdited`.

## Subscription User-Agent

Subscription HTTP requests use:

```text
Varmlen/<app-version> (<platform>; <architecture>)
```

Examples:

```text
Varmlen/0.2.0 (Linux; x86_64)
Varmlen/0.2.0 (Android; arm64)
```

Platform and architecture are compile-time target values. The client does not
send `x-hwid`, a device model, OS version, or any unique installation ID.

## Verification

- unit-test flag emoji to ISO-code conversion;
- unit-test JSON payload classification and preservation in Rust;
- unit-test the platform User-Agent format;
- unit-test local JSON edits, failed edits, and auto-refresh eligibility;
- verify both Svelte frontends with `vitest`, `svelte-check`, and Vite builds;
- run Rust tests for both client repositories;
- visually inspect the Add subscription and JSON editor flows locally without
  connecting, disconnecting, or probing the VPN.
