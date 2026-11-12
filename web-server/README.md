# Private web edition

This independent Axum server compiles the existing Rust API, networking, image decryption and HLS rewrite modules directly. Native builds keep their Tauri commands. The web build changes only the frontend transport and the media origin.

## Build and run locally

```
pnpm install --frozen-lockfile
pnpm exec tsc --noEmit
VITE_WEB=1 pnpm exec vite build --outDir dist-web
cargo test --manifest-path web-server/Cargo.toml
cargo build --release --manifest-path web-server/Cargo.toml
HG_PASSWORD_FILE=/absolute/private/password.txt web-server/target/release/hghub-web
```

Create a random password of at least 16 characters in a mode-0600 file. Never commit it. Default bind is `127.0.0.1:8900`; the static directory defaults to `dist-web`. Password changes take effect on restart, which also invalidates all login sessions. Sessions expire after 24 hours. This is a personal single-owner application; upstream account state and line settings are shared among browsers with the access password.

## Linux deployment

Prepare `/opt/hghub/hghub-web` and `/opt/hghub/dist-web`, a dedicated `hghub` system user, `/etc/hghub/password` readable only by that user, and `/etc/hghub/server.env` from the example. Install the supplied unit into `/etc/systemd/system/`. Run `systemctl daemon-reload` and `systemctl enable --now hghub-web`. Verify `is-active`, `is-enabled`, and the journal. The unit runs with a read-only filesystem except `/var/lib/hghub`.

Verify on the server's loopback interface before configuring any public listener or cloud firewall. Use HTTPS for the externally reachable login; configure `HG_ORIGIN` to the exact HTTPS origin and reverse-proxy to `127.0.0.1:8900`. Do not expose the plaintext backend port directly to the Internet. No cloud firewall changes have been performed by these files.

## Required acceptance evidence

- Anonymous `/` and static assets redirect to `/login`; API and media return 401.
- Wrong passwords fail; cross-origin writes fail; logout invalidates the session.
- After login, feed contains real items and a selected item's detail loads.
- Its rewritten HLS playlist, a referenced segment and cover return 200 through `/media`.
- Repeat these tests from outside the server, then verify Safari on iPhone.

Building successfully alone does not prove deployment or real playback. Do not publish upstream response bodies, source media URLs, credentials or private logs.

## Validation status before server setup

The macOS local build passed 41 Rust tests (5 upstream network tests intentionally skipped), frontend type checks and a mobile-viewport Chromium login check. The real local feed request was blocked by this machine's mixed FakeIP/private DNS results. Real feed, HLS, segment, cover, public access and real iPhone Safari acceptance are still pending. A Linux Actions build prepares the deployment archive; it does not by itself certify playback or server deployment.

## Public browsing and downloads

Set `HG_PUBLIC_ACCESS=true` to permit anonymous browsing and playback. It defaults to private mode. Public requests use a separate guest API session; account changes, source changes, diagnostics and other administrative commands still require the site password. Public media uses bounded, opaque, expiring references minted from API responses and playlists; arbitrary public proxy URLs are rejected.

Serve the included `downloads/` directory through Nginx at `/downloads/` and publish only the six installer files listed in `manifest.json` under `/downloads/files/`. Do not place credentials, signing keys or server configuration in this directory. Static downloads support HTTP Range and versioned caching. Keep the Rust listener on loopback.
