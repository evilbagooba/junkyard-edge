# Running Rust-Compiled WebAssembly on a Recycled Android Phone: A Debugging Log

_Part of the Autonomous Edge Micro-Datacenter project_

## The Goal

Before scaling up to a multi-phone cluster, I needed to validate one specific claim: that a Rust program, compiled to WebAssembly and executed by the WasmEdge runtime, could actually run on real, unmodified Android hardware — not just my development machine. The target device: a Samsung Galaxy A32 5G, a decommissioned phone destined to become one node in a solar-powered edge computing array.

This is the log of what it actually took to get there, including the dead ends.

## Step 1: Getting a Real Linux Userspace onto Android

Android doesn't give you a general-purpose shell environment out of the box — no package manager, no root, restricted syscalls. The fix is **Termux**, installed via F-Droid (the Play Store build has been stale for years). Termux isn't an emulator; it runs directly on the same Linux kernel Android itself uses, giving you native execution — just without the Android/Java runtime wrapped around it.

## Step 2: The File Transfer Maze

WasmEdge has no Android installer. The documentation says, in effect, "download the release binary and adb-push it to the device" — which sounds simple and isn't, because of how Android sandboxes app storage.

The actual chain of discovery:

1. `adb push file.txt /sdcard/` succeeds silently, but the file doesn't show up in the Files app. Turns out `/sdcard/` writes land fine at the filesystem level — they're just invisible to the **MediaStore index** that UI apps query against. Checking via `adb shell ls` confirms the file is really there.
2. Termux's own home directory (`/data/data/com.termux/files/home`) is a _separate app's private storage_ — `adb push` can't write there directly, since it runs as the `shell` user, not as Termux's UID.
3. The bridge is `termux-setup-storage`, which symlinks shared storage into `~/storage/shared` inside Termux, from where you can `cp` into `$HOME`.
4. Critically: shared/FUSE-mounted storage on Android is commonly mounted `noexec`. A file can sit in `~/storage/shared` and be completely non-executable no matter what `chmod` says — it _has_ to be copied into Termux's private `$HOME` to run at all.

Once I actually needed to fetch the WasmEdge release itself, though, this entire pipeline turned out to be a red herring for that particular file — because Termux has its own shell and its own network access. `curl` from inside Termux downloads straight into the exec-permitted home directory, no adb hop required at all. The multi-step transfer pipeline remains necessary for anything that originates _outside_ the phone (like my own compiled `.wasm` binaries) — just not for public downloads.

## Step 3: Picking the Right Release Asset

GitHub release pages are a minefield of near-identical filenames. My first pick — `WasmEdge-0.17.1-darwin_arm64_static.tar.gz` — matched the CPU architecture (`arm64`) but not the OS (`darwin` is macOS, not Android/Linux). The correct asset was `WasmEdge-0.17.1-android_aarch64.tar.gz`.

Extracting it revealed `bin/`, `lib/`, and `include/` — meaning `bin/wasmedge` is **dynamically linked**, not a self-contained static binary. It needs `lib/libwasmedge.so` to be discoverable at runtime via `LD_LIBRARY_PATH`, and both `PATH` and `LD_LIBRARY_PATH` need to be set persistently in `~/.bashrc` (Termux's default shell) so they survive across sessions.

## Step 4: First Successful Execution

```
export LD_LIBRARY_PATH=$HOME/lib:$LD_LIBRARY_PATH
export PATH=$HOME/bin:$PATH
wasmedge --version
```

Output: `0.17.1`. That's the actual milestone — genuine, sandboxed WebAssembly execution on ARM64 Android silicon, under Termux, no root required.

## Step 5: The Networking Rabbit Hole

Getting the runtime working was the easy half. Getting the actual simulation server reachable was harder, and the debugging process is a good illustration of layered troubleshooting:

- **Bind address**: the server originally bound `127.0.0.1` (loopback-only). Changing it to `0.0.0.0` was necessary to accept connections from any interface — but a hardcoded `println!` logging the old address kept lying about it, a good reminder that log strings aren't automatically synced to actual runtime state.
- **Reachability**: `ping` between the dev machine and the phone succeeded, ruling out Wi-Fi/subnet isolation.
- **WSL as a red herring**: briefly suspected WSL's virtual network adapter as the cause, until testing from a native Windows browser produced the identical failure — eliminating it as a variable.
- **TCP vs. application layer**: `Test-NetConnection` on Windows confirmed the raw TCP handshake succeeded. So the listener was accepting connections at the socket level — the failure was happening somewhere above that.
- **Isolating further with curl**: a raw HTTP `Upgrade` request via `curl`, sent to `127.0.0.1` **from a second Termux session on the same device**, removed every remaining network variable (Wi-Fi, subnet, firewall, WSL) at once. The connection hung indefinitely, and the server-side log line for a new client connection never printed.
- **The control experiment**: the exact same `.wasm` binary, run under the exact same version of WasmEdge, on a desktop machine instead of the phone, completed the identical handshake correctly and instantly.

That comparison was the actual answer: identical bytecode, identical runtime version, different result based purely on host platform. The Rust code, and the WASI-compatible crates it depends on (`tokio_wasi`, `warp_wasi` — themselves already the correct choice for async sockets under WASI), were exonerated entirely. The gap sits in the **Android build of the WasmEdge runtime itself** — specifically its socket-handling implementation for _inbound_ connections, since no separate sockets plugin was present in the extracted release tree to account for it.

## Where This Leaves the Architecture

Rather than treat this as a blocker, it reframes the design in a way that arguably fits the project's own goals better: instead of each phone node independently binding a listener and serving clients directly, the phone becomes a **producer** — computing simulation state and pushing it outward — while a separate process (a laptop, or eventually a Raspberry Pi acting as the Tier 2 gateway) does the listening and serving. This is closer to how the project's own Tier 2/Tier 3 architecture was already meant to work, just arrived at earlier than planned.

The next concrete test: confirm whether _outbound_ connections from the same wasm binary succeed on this Android build, which would confirm the fix is architectural rather than a dead end requiring a different runtime entirely.

## Takeaways

- A "prebuilt binary" claim on a project's install page is a starting point, not a guarantee — always verify OS/ABI match in the filename, not just CPU architecture.
- Android's storage sandboxing (app-private storage, `noexec` shared mounts, MediaStore indexing) creates several silent failure modes that look like permission bugs but are actually storage-location bugs.
- When debugging a networking failure across two machines, always run the control experiment on a single machine first (or drop to loopback) — it's the fastest way to separate "my code is wrong" from "my environment is wrong."
- Identical WebAssembly bytecode running under identical runtime versions on different host platforms is a legitimate, decisive comparison — if behavior differs, the bug is in the runtime's platform-specific implementation, not in the portable bytecode.