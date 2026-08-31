# Serving the desktop app on the web

Run the Vite development server and open it from a device on the LAN:

```bash
pnpm dev
```

The dev server listens on `http://0.0.0.0:1420`.

To serve a production build:

```bash
pnpm build && pnpm preview
```

The production preview listens on `http://0.0.0.0:1420`.

Tauri-native APIs such as `invoke` are unavailable in a plain browser, and the STT/infra server must be reachable from the browser; start it with `docker compose up` in `/Users/janghyeok/orca/workspaces/make_app/meeting-recorder-mvp/infra/stt-server`.
