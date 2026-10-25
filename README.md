# ClickPlanet Project

A Rust client implementation for interacting with [ClickPlanet](https://clickplanet.lol), a multiplayer geographic clicking game.

Clickplanet itself is open source: [frontend](https://github.com/raphoester/clickplanet.lol-frontend)
and [backend](https://github.com/raphoester/clickplanet.lol-backend), by raphoester (also linked here as the
`original-frontend` / `original-backend` submodules). At the time of writing neither repository ships a licence
file, so no Clickplanet code is copied into this repository; the reverse-engineering notes that used to live in
`reverse-engineering/` were removed in favour of the upstream sources. A few API exploration commands are kept in
`docs/clickplanet-api-examples.sh`.

## Protocol

The game uses Protocol Buffers for all communication

## Features

- A country safeguard robot.
- A REST / Websocket client
- A server using CQRS architecture.
 - Stack: hyper / axium
 - Redis for cold state recovery
 - NATS for N-to-N click exchanges with full in-memory management

# Perfs
```
(base) ➜  clickplanet-client git:(feat-server) ✗ time curl 'http://localhost:3000/v2/rpc/leaderboard' \
  -H 'accept: */*' \
  -H 'accept-language: en-GB,en;q=0.9,fr-FR;q=0.8,fr;q=0.7,en-US;q=0.6' \
  -H 'content-type: application/json' \
  -H 'origin: https://clickplanet.lol' \
  -H 'priority: u=1, i' \
  -H 'referer: https://clickplanet.lol/' \
  -H 'sec-ch-ua: "Google Chrome";v="131", "Chromium";v="131", "Not_A Brand";v="24"' \
  -H 'sec-ch-ua-mobile: ?0' \
  -H 'sec-ch-ua-platform: "macOS"' \
  -H 'sec-fetch-dest: empty' \
  -H 'sec-fetch-mode: cors' \
  -H 'sec-fetch-site: same-origin' \
  -H 'user-agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36'|jq -r .data|base64 -d|protobuf_inspector
  % Total    % Received % Xferd  Average Speed   Time    Time     Time  Current
                                 Dload  Upload   Total   Spent    Left  Speed
100    75  100    75    0     0   2587      0 --:--:-- --:--:-- --:--:--  2678
root:
    1 <chunk> = message(1 <chunk> = "ru", 2 <varint> = 31473)
    1 <chunk> = message(1 <chunk> = "fr", 2 <varint> = 16478)
    1 <chunk> = message(1 <chunk> = "es", 2 <varint> = 874)
    1 <chunk> = message(1 <chunk> = "sr", 2 <varint> = 544)
    1 <chunk> = message:
        1 <chunk> = message:
            14 <message> = group (end 12) message()
        2 <varint> = 1

curl 'http://localhost:3000/v2/rpc/leaderboard' -H 'accept: */*' -H  -H  -H    0.01s user 0.01s system 37% cpu 0.055 total
jq -r .data  0.03s user 0.00s system 58% cpu 0.054 total
base64 -d  0.00s user 0.00s system 8% cpu 0.053 total
protobuf_inspector  0.03s user 0.01s system 71% cpu 0.053 total
```

Small benchmark.

```
cargo run --bin clickplanet-robot -- --target-country ru --wanted-country ru --port 3000 --host localhost --unsecure
cargo run --bin clickplanet-robot -- --target-country ru --wanted-country fr --port 3000 --host localhost --unsecure
```

NATS Storage.

```
2024-12-13 15:55:48 [1] 2024/12/13 14:55:48.096982 [INF] ---------------- JETSTREAM ----------------
2024-12-13 15:55:48 [1] 2024/12/13 14:55:48.096984 [INF]   Max Memory:      5.81 GB
2024-12-13 15:55:48 [1] 2024/12/13 14:55:48.096985 [INF]   Max Storage:     83.91 GB
2024-12-13 15:55:48 [1] 2024/12/13 14:55:48.096985 [INF]   Store Directory: "/tmp/nats/jetstream"
2024-12-13 15:55:48 [1] 2024/12/13 14:55:48.096986 [INF] -------------------------------------------
2024-12-13 15:55:48 [1] 2024/12/13 14:55:48.097781 [INF]   Starting restore for stream '$G > CLICKS'
2024-12-13 15:55:49 [1] 2024/12/13 14:55:49.227323 [INF]   Restored 13,367,380 messages for stream '$G > CLICKS'
2024-12-13 15:55:49 [1] 2024/12/13 14:55:49.227348 [INF]   Recovering 2 consumers for stream - '$G > CLICKS'
2024-12-13 15:55:49 [1] 2024/12/13 14:55:49.228354 [INF] Listening for client connections on 0.0.0.0:4222
2024-12-13 15:55:49 [1] 2024/12/13 14:55:49.228562 [INF] Server is ready
```


This generates gygabytes of events, and despite all of this, the websocket endpoints do not saturate

## Deployment

Will be possibly be done using cloud run and/or GKE and/or EKS. Or maybe just a simple, minikube tiny server.
It needs:
- 1-N NATS server / cluster
- 1-N redis server / cluster
- 1-N worker for redis storage
- 1-N servers

### Server modes: with or without NATS

Cargo features must be additive (a feature cannot remove a dependency), so NATS is the optional
part:

| Build | Mode |
|---|---|
| `cargo build -p clickplanet-server` (default feature `nats`) | NATS JetStream carries the clicks; `state-click-persister` writes them to Redis. Several servers can share one NATS. |
| `cargo build -p clickplanet-server --no-default-features --features nonats` | Single process, no NATS: clicks go through an in-memory `tokio::sync::broadcast`; Redis is optional (`REDIS_URL`) and gets every click directly. `state-click-persister` is not built. |

`nonats` only selects the single-process mode; `--no-default-features` alone gives the same build.
Enabling `nonats` together with `nats` (for example `--features nonats` without
`--no-default-features`) stops the build with a `compile_error!` that says so.

In both modes:
- clicks are `POST /v2/rpc/click` (and `/api/click`), as before;
- live updates are streamed with **Server-Sent Events** on `GET /v2/events` (`text/event-stream`):
  `event: update` carries an `UpdateNotification`, `event: click` a `Click`, both base64 protobuf in
  `data:`, with a monotonic `id:`. A client that reconnects with `Last-Event-ID` (`EventSource` does it by
  itself; `?lastEventId=` also works) gets what it missed from a ring buffer of `SSE_BUFFER` events
  (default 50,000). Slow clients that fall behind the broadcast channel are caught up the same way.
  When the missed events are gone, the server sends `event: resync` and the web app reloads the
  ownerships. A `:` comment is sent every `SSE_HEARTBEAT_SECS` (default 20 s) for proxies and Cloud Run;
- the `/v2/ws/listen` websocket is kept: the robots (`clickplanet-client`) use it, and the web app can
  use it with `?updates=ws` (for servers without SSE, such as clickplanet.lol). The web app uses SSE by
  default.

### Cloud Run

`Dockerfile.cloudrun` builds `click-server` without NATS on a distroless image (non-root). Cloud Run
constraints:

- **Port**: the server listens on `$PORT` (set by Cloud Run).
- **No local state survives**: without `REDIS_URL` the map lives in memory and is lost on every restart or
  scale to zero. Use Memorystore (`REDIS_URL=redis://<ip>:6379`, reached through Direct VPC egress or a
  VPC connector) to keep it.
- **Long requests**: SSE and websockets are supported, but a request lasts at most the service timeout
  (up to 60 minutes). `EventSource` reconnects by itself and resumes with `Last-Event-ID`; the web app's
  websocket client reconnects too.
- **One instance**: SSE solves the fan-out to clients, not the sharing of events between instances. Each
  instance only sees its own clicks, so the service must run with `--max-instances=1`
  (`--min-instances=1` avoids losing the in-memory state between requests when there is no Redis, and
  `--no-cpu-throttling` keeps the background click processing running).

```bash
# not run by CI; nothing is deployed from this repository
gcloud run deploy citywars-click-server \
  --image europe-west1-docker.pkg.dev/PROJECT/REPOSITORY/click-server-cloudrun:TAG \
  --region europe-west1 --allow-unauthenticated \
  --max-instances 1 --min-instances 1 --no-cpu-throttling \
  --timeout 3600 --concurrency 1000 \
  --network default --subnet default --vpc-egress private-ranges-only \
  --set-env-vars REDIS_URL=redis://10.0.0.3:6379
```

**Several instances (proposal, not implemented).** The simplest shared bus that fits this setup is a
**Redis Stream** on the Memorystore instance already used for persistence: every instance `XADD`s the
clicks it receives and runs one `XREAD` loop that feeds its in-memory map and its SSE hub. The stream
entry ids are global and ordered, so they can become the SSE `id:` and `Last-Event-ID` keeps working
when a client reconnects to another instance; `XTRIM` bounds the history. It would sit behind a
`redis-bus` feature next to `nats`. Cost: one more Redis round trip per click, Memorystore sized for the
click rate, and about two days of work (bus abstraction over NATS and Redis, tests with two
instances). Plain Redis pub/sub would be simpler but has no replay, so a reconnecting client could not
resume on another instance.


## Building

Requires Rust (stable) and `protoc` (Protocol Buffers compiler). To build and test the native crates:

```bash
cargo build --workspace
cargo test --workspace      # the Redis tests start a container, so Docker must be running
```

`clickplanet-webapp` only runs in the browser (`wasm32-unknown-unknown`); on native its binary is a stub, so
`cargo check --workspace` works everywhere. The web bundle is built with the Dioxus CLI **0.6.3**:

```bash
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --version 0.6.3 --locked
cd clickplanet-webapp && ./build-web.sh --release   # output in clickplanet-webapp/dist
```

The Dioxus CLI binary is called `dx`, like Deno's tool (e.g. Homebrew's `deno` ships a `dx`). `build-web.sh` does not
trust the first `dx` on `PATH`: it uses `$DX`, then `dx` on `PATH`, then `~/.cargo/bin/dx`, and keeps the first one
that reports `dioxus 0.6.3`. Set `DX=/path/to/dx` to force one.

The globe bundle also needs the wasm-bindgen CLI matching `Cargo.lock`
(`cargo install wasm-bindgen-cli --version 0.2.129 --locked`); `wasm-opt` is used when installed.

## Web app

The web app is a port of the [original Clickplanet front end](https://github.com/raphoester/clickplanet.lol-frontend)
(React + three.js) to Rust, without three.js. It is made of **two WebAssembly bundles** that only talk through events:

| Bundle | Crate | Role |
|---|---|---|
| Interface | `clickplanet-webapp` (Dioxus 0.6) | Menus, modals, leaderboard, country selection, HUD. Owns the network: RPC to the server, live updates, clicks. |
| Globe | `clickplanet-globe` (wgpu 25) | Earth, ~258k tiles, camera controls, picking. Fetches its own static data (`coordinates.json`, flag atlas, earth texture). |
| Contract | `clickplanet-globe-protocol` | Versioned, typed messages (`UiToGlobe`, `GlobeToUi`) encoded with postcard. |

- **Messages.** Interface → globe: `Init`, `SetCountries`, `SetOwners` (batches and live updates), `PlayerCountry`,
  `Settings`, `Camera`, `Pointer`, `Wheel`, `Resize`. Globe → interface: `TilesLoaded`, `Ready`, `Hovered`, `Clicked`
  (tile id, lon/lat), `Stats` (fps), `Error`. Any globe that speaks this protocol (for example a future tuile-based one)
  can replace `clickplanet-globe` without touching the interface.
- **Transport.** By default the globe runs in a **Web Worker** drawing on an **OffscreenCanvas**
  (`transferControlToOffscreen`), so rendering never blocks the Dioxus thread. Payloads are `ArrayBuffer`s passed as
  transferables. If OffscreenCanvas is missing, or the worker or the GPU fails to start, the interface restarts the
  globe on the **main thread**, connected through a `MessageChannel` with the same messages (`?globe=main` forces it).
- **Rendering.** WebGPU when the browser has it, otherwise **WebGL2** through wgpu's GL backend (`?renderer=webgl`
  forces it). Tiles are drawn as one instanced draw (one camera-facing disc per tile, flag from
  `countries/atlas.png`); ownership changes are partial writes to the instance buffer.
- **Picking.** Ray against the sphere on the CPU, then nearest tile in an R*-tree (`rstar`). It is synchronous, works
  the same on both backends and is cheap enough for hover on every pointer move; a GPU id buffer would need an extra
  pass and an asynchronous (WebGPU) or stalling (WebGL2) read-back.
- **Controls.** Same behaviour as the original `OrbitControls` setup: drag to rotate with inertia, wheel or pinch to
  zoom (×1 to ×50), auto-rotation while not zoomed in.

Browser support (worker + OffscreenCanvas, else main thread; WebGPU, else WebGL2):

| Browser | Globe in a worker | WebGPU | Notes |
|---|---|---|---|
| Chrome / Edge 113+ (desktop, Android) | yes | yes | |
| Firefox 105+ | yes (WebGL2) | 141+ on Windows | elsewhere WebGL2 |
| Safari 17+ (macOS, iOS) | yes (WebGL2) | Safari 26+ | Safari 16.4: worker + WebGL2 may fall back to the main thread |
| Older browsers | main thread | no | WebGL2 required |

Only Chromium was checked end to end (see the parity PR); Safari and Firefox support comes from their documented
APIs and the automatic fallbacks.

Run it locally:

```bash
# offline, with the fake backend (robots clicking for every country)
cd clickplanet-webapp && CITYWARS_STATIC_SITE=/static ./build-web.sh --release
# serve clickplanet-webapp/dist at / and clickplanet-static/static (+ coordinates.json) at /static, then open
#   http://localhost:8080/?backend=fake&stats
```

Query parameters: `backend=http|fake`, `updates=sse|ws` (live updates over SSE, the default, or the websocket),
`api=<server URL>` (default `CITYWARS_API_URL` at build time, else
`http://localhost:3000`), `renderer=auto|webgpu|webgl`, `globe=main`, `stats` (HUD with renderer and fps; the same
values are on `window.citywarsStats`).

## API Endpoints

- WebSocket: wss://clickplanet.lol/ws/listen
- Click: POST https://clickplanet.lol/api/click
- Ownerships: GET https://clickplanet.lol/api/ownerships
- Batch Ownerships: POST https://clickplanet.lol/api/ownerships-by-batch

## Dependencies

- prost: Protocol Buffers implementation
- tokio-tungstenite: WebSocket client
- tokio: Async runtime
- native-tls: TLS support

## Data files

- `coordinates.json` (repository root): tile coordinates used by the robots, the Docker images and
  `docker-compose.yml`. It is versioned once; `clickplanet-static/deploy_static.sh` uploads it to the static site.
  It cannot be regenerated reliably yet (see Known issues), so it stays in git.
- `tile_to_countries.json` / `country_to_tiles.json`: tile/country lookup tables read by the robots. The robots can
  rebuild them from `countries.geojson` when they are missing, but the output has not been checked against the
  versioned copies, so they stay in git.
- `countries.geojson` is no longer versioned. Download it with `./scripts/fetch-countries-geojson.sh` (pinned to
  [datasets/geo-countries](https://github.com/datasets/geo-countries) at commit `cbd21e1`, derived from Natural Earth,
  public domain; SHA-256 checked). The Docker images fetch the same pinned file at build time.

## Known issues

- The static site bucket used for `CITYWARS_STATIC_SITE` (`storage.googleapis.com/lv-project-313715-clickwars-static`)
  currently answers 403. This is a bucket permission issue, not a code issue. The build only embeds the URL as a
  compile-time string, so CI does not depend on it, but the web app cannot load its static assets (textures,
  `coordinates.json`, flags) until the bucket is readable again or `CITYWARS_STATIC_SITE` points to another host
  (for example a local server of `clickplanet-static/static`).
- `clickplanet-webapp/src/scripts/compute_points.rs` is wrong: it ignores the `detail` parameter and only emits the
  12 base icosahedron points, so it cannot regenerate `coordinates.json`. A fix has to reproduce three.js'
  `IcosahedronGeometry` vertex order exactly (tile ids are vertex indices), so it is left as is; it now compiles.

Fixed:

- `wasm-bindgen` 0.2.99 failed with "duplicate export name" on recent rustc; it is now 0.2.129 (with `js-sys` /
  `web-sys` 0.3.106 and `wasm-bindgen-futures` 0.4.79). Dioxus 0.6.3 is unchanged.
- `compute_points.rs` did not compile on native (`glam` was a wasm-only dependency).
- `cargo check --workspace` failed on native: `clickplanet-webapp` is now a stub outside wasm32, and the
  `clickplanet-server` binaries no longer rely on the never type fallback (denied by recent rustc).
- The Dioxus CLI / Deno `dx` name clash: see Building.
