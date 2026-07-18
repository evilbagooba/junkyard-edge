# Junkyard Edge

An autonomous, solar-powered edge computing cluster built entirely from decommissioned Android smartphones. Phones run on-device computer vision against their real physical surroundings and stream detected positions/motion to a client UI over a raw-binary WebSocket — no cloud infrastructure involved.

Full rationale, architecture tiers, and technology decisions live in [docs/ARCHITECTURE.md](ARCHITECTURE.md).

## How it works

A Rust core compiles to WebAssembly (`wasm32-wasip1`) and runs under the [WasmEdge](https://wasmedge.org/) runtime on each donor phone via Termux. It streams state over a WebSocket as raw binary (via `bytemuck`/`#[repr(C)]` struct casting) rather than JSON, so a JS/WebGL client can read coordinates straight out of an `ArrayBuffer` with `DataView` at high frame rates without garbage-collector overhead.

Currently, the core generates its coordinates from a synthetic flocking/herding physics simulation rather than real camera input — see Status below for why, and [ARCHITECTURE.md](ARCHITECTURE.md) §1 for how that's expected to change.

```
simulation-engine/   Rust -> Wasm simulation core (WasmEdge / WASI)
client-bridge/       Minimal HTML/JS client that consumes the binary WebSocket stream
```

## Architecture tiers

1. **Tier 1 — Autonomous Observer:** a single phone runs the simulation locally and streams to a laptop/simulator.
2. **Tier 2 — Local Mesh:** 3–5 phones on an air-gapped Wi-Fi subnet, gatewayed through a Raspberry Pi.
3. **Tier 3 — Distributed Solar Hive:** a fully off-grid array, wired Ethernet + 12V LiFePO4/solar power, orchestrated with K3s.

See ARCHITECTURE.md §3 for details on each tier and §4 for the full stack decision matrix (Wasm vs. Docker, Rust vs. Go, K3s vs. K8s, WebSockets+FlatBuffers vs. REST+JSON, React Native/Expo for the client).

## Status

- Rust → Wasm build pipeline working locally (`wasm32-wasip1` target, `wasmedge` runner configured via `.cargo/config.toml`). See [Session1.md](Session1.md) for the dev environment setup (Neovim/rust-analyzer) and the Tokio single-threaded/WASI constraints that had to be worked around.
- First successful on-device execution: WasmEdge running natively under Termux on a Samsung Galaxy A32 5G, no root required.
- Known constraint: the Android build of WasmEdge doesn't currently accept **inbound** WebSocket connections (socket-handling gap in that runtime build). The architecture already treats phones as telemetry *producers* rather than listeners, so this pushes the design toward that model earlier than planned — a laptop or Raspberry Pi (the Tier 2 gateway) does the listening/serving instead. Next validation step: confirm outbound connections succeed from the same binary. Full debugging log in [Session2.md](Session2.md).
- The synthetic flocking/herding simulation currently in `simulation-engine/` is a stand-in used to validate the Rust/Wasm/WasmEdge/WebSocket pipeline first. Real on-device computer vision (replacing the simulation as the source of streamed coordinates) is the target, not yet built — see ARCHITECTURE.md §1 and its open camera-privacy questions in §5 before that lands.

## Development

Requires the Rust toolchain with the `wasm32-wasip1` target and `rustup component add rust-analyzer` for editor support. `simulation-engine/.cargo/config.toml` pins the build target and routes `cargo run` through the `wasmedge` runner.

```
cd simulation-engine
cargo run
```

On an Android donor device, install [Termux](https://f-droid.org/packages/com.termux/) (F-Droid build, not Play Store) and run `termux-setup-storage`, then follow the transfer/setup steps in Session2.md to get WasmEdge and a compiled `.wasm` binary onto the device.

## Motivation

This project explores compute-per-watt engineering for constrained, battery/solar-powered devices — see [ARCHITECTURE.md](ARCHITECTURE.md) §2 for the full context.
