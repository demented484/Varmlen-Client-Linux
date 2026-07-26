# Card Surfaces and Version

## Scope

Apply the same presentation changes to the Linux and Android clients:

- show the installed Varmlen version at the bottom of Settings;
- remove neutral outer outlines from card-like surfaces throughout the app;
- remove separators inside grouped cards;
- keep functional outlines on controls, menus, errors, status indicators, and
  the VPN power button.

## Visual behaviour

Cards keep their elevated fill, corner radius, spacing, and hover states. Only
their neutral outer outline is removed. Grouped rows have no separator line;
spacing and hover states provide the structure without cutting through the card.

The active theme tile uses a distinct fill and soft accent glow instead of an
outline. Subscription pin state remains visible through the existing pin icon.

The Settings footer is quiet, centred, and muted:

```text
Varmlen 0.2.0
```

The numeric value comes from Tauri's runtime application metadata via
`getVersion()`, so it follows future package version bumps without duplicated
UI constants.

## Boundaries

The change includes shared `.card` and `.list` surfaces, subscription cards,
theme tiles, empty states, the core-version list, and the split-tunnel
application picker.
It does not remove outlines from form fields, dropdown controls and menus,
buttons, badges, error banners, the tab bar, or the VPN power control.

No VPN connection, route, DNS, helper, or split-tunnelling logic changes.
