# Project Blueprint: Autonomous Edge Micro-Datacenter

This document serves as the engineering reference for the design, architecture, and rationale of the off-grid edge computing array.

## 1. Project Overview

The system is a fully autonomous, solar-powered, heterogeneous edge computing cluster built entirely from decommissioned and recycled smartphones. Functioning as a "junkyard micro-datacenter," it distributes complex multi-agent spatial simulations (flocking, herding, and attraction logic) across constrained hardware.

The architecture proves that disparate mobile devices can be unified under a single control plane to execute high-frequency mathematical workloads natively, streaming live telemetry to a mobile client without relying on traditional cloud infrastructure.

### Current phase vs. target architecture

The flocking/herding logic currently running on-device is synthetic — a physics simulation generating agent coordinates internally, with no camera or sensor input. This was a deliberate starting point: it let the Rust/Wasm/WasmEdge/WebSocket toolchain get validated end-to-end (build pipeline, on-device execution, binary streaming) without also debugging computer vision at the same time.

The target architecture replaces that synthetic core with camera-based computer vision: phones observe their real physical surroundings and stream detected positions/motion instead of simulated ones. The simulation phase is scaffolding, not the end state — everything below describes the system with that in mind.

### Privacy note

Real camera input is a materially different question from the synthetic-simulation prototype, and needs its own real answer rather than being sidestepped. Pointing a phone camera at a real space raises the same considerations any camera-based hobby project does under Japanese privacy norms (Shōzōken — the right to control one's own likeness — and building/apartment rules against cameras facing shared or neighboring areas). The working constraint for Tier 1 is: indoor, pointed at a controlled space the operator has a right to observe, not at people or at any area outside that space. This still needs to be designed deliberately (e.g. what the CV model is trained to key on, whether faces are ever in frame, retention/deletion of any captured frames) rather than assumed solved — see open questions below.

## 2. Motivation

This project grew out of a simple question: how much useful compute can you squeeze out of hardware that's already been thrown away, if you're willing to be strict about power and memory budgets?

A few things shaped the specific design choices:

- **Compute-per-watt under real constraints.** Companies building solar-powered field hardware — Halter's livestock-tracking collars are a good public example — have to solve the same core problem this project does: run meaningful on-device computation (spatial tracking, inference) on a tight, unreliable power budget, with no cloud fallback. Replicating that constraint on recycled phones was a way to actually feel the tradeoffs involved, rather than just read about them.
- **Green/edge computing as a research direction.** Work in this space — Microsoft's biodiversity and edge-sensing projects among others — points at the same idea: push computation to the environment being observed instead of streaming raw data to a datacenter. This project is a small, personal-scale version of that same idea.
- **Academic interest.** Benchmarking distributed evolutionary/simulation architectures on genuinely low-power edge nodes is a niche that's underexplored relative to how much attention cloud-scale ML gets.

## 3. The Tiered "Multi-Suite" Deployment Architecture

The system is designed as a monorepo that scales from a single device to an off-grid server rack using Infrastructure as Code (IaC).

- **Tier 1: The Autonomous Observer (Single Node):** The minimum viable prototype. A single phone securely mounted indoors, running on-device computer vision against a controlled indoor space and streaming detected positions to a laptop or mobile client. Framing and subject matter are chosen to stay clear of Shōzōken concerns and apartment rules against cameras facing shared/neighboring or public areas — see §2's privacy note for the constraint this implies. (Earlier development used a synthetic flocking/herding simulation in place of real CV input to validate the toolchain first; see the README's Status section for where that stands.)

- **Tier 2: The Local Mesh:** A decentralized network of 3–5 phones operating on an air-gapped Wi-Fi subnet. A Raspberry Pi acts as the gateway. This phase validates the telemetry aggregation and tests the React Native UI's ability to render multi-agent data without dropping frames.

- **Tier 3: The Distributed Solar Hive:** The final 100% off-grid array. Phones are hardwired via a Gigabit switch and powered by a 12V LiFePO4 battery and monocrystalline solar panel. K3s orchestration dynamically routes workloads based on hardware capability and thermal limits.

## 4. Technology Stack & Decision Matrix

Every layer of the stack was chosen to minimize memory footprint, reduce CPU cycles, and preserve battery life.

|**Layer**|**Selected Tech**|**The Alternative**|**Strategic Rationale for Decision**|
|---|---|---|---|
|**Execution Engine**|**WebAssembly (Wasm)**|Docker / Containers|Docker carries heavy OS-level overhead (100MB+). Wasm provides sub-millisecond cold starts, requires under 10MB of memory, and is architecture-agnostic, running identically on 32-bit and 64-bit donor phones.|
|**Compute Language**|**Rust**|Go (TinyGo) / Python|While Go allows for faster prototyping, Rust features zero-cost abstractions and lacks a garbage collector. This prevents micro-stutters in the simulation and extracts the absolute maximum compute-per-watt from the solar budget.|
|**Orchestration**|**K3s (Kubernetes)**|Standard K8s / Swarm|Standard K8s would melt the Raspberry Pi control plane. K3s provides the same granular node-labeling (routing heavy math to wired phones and light telemetry to Wi-Fi phones) with a fraction of the background RAM requirement.|
|**Data Pipeline**|**WebSockets + FlatBuffers**|REST API + JSON|High-frequency telemetry (60fps) chokes standard HTTP requests. Persistent WebSockets eliminate connection overhead, and FlatBuffers compress the data into raw bytes, bypassing the CPU-heavy string parsing required by JSON.|
|**Client UI**|**React Native (Expo)**|React Web (DOM)|To mirror enterprise mobile-first tools, a native mobile app is essential. Using `expo-gl` paired with Three.js Fiber bypasses the standard React Native bridge bottlenecks, enabling hardware-accelerated 3D rendering on mobile without thermal throttling.|

## 5. Engineering Hurdles & Resolved Constraints

Throughout the system design phase, several physical and software barriers were addressed:

### How do we handle hardware variance in donated phones?

- **The OS Constraint:** Apple iOS aggressively kills background processes and lacks a native terminal, making iPhones unviable for the cluster. The focus remains strictly on Android devices.

- **The Architecture Constraint:** Older phones run 32-bit chips; newer phones run 64-bit. Compiling the Rust core logic down to `wasm32-wasi` solves this entirely, as WasmEdge acts as a universal translator.

- **Data Sanitization:** A strict 3-phase wiping protocol (Pre-Wipe, Cryptographic Erasure via factory reset, and Secure Overwrite via ADB scripts) ensures all donor data is irreversibly destroyed before integration.

### How do we prevent power loss on older Micro-USB phones?

- **The Problem:** Legacy Micro-USB ports cannot handle data host mode (OTG) and incoming power simultaneously. Hardwiring them to the Ethernet switch causes them to drain their batteries and die.

- **The Solution:** A hybrid network topology. Modern USB-C phones handle the heavy math via wired Ethernet hubs with Power Delivery pass-through. Legacy phones are strictly connected to power cables and transmit their lightweight telemetry wirelessly over the local Wi-Fi mesh.

### How do we handle real camera input responsibly? (open)

Moving from synthetic simulation to real computer vision raises questions that aren't fully resolved yet:

- What is the CV model actually trained to detect/key on, and does that ever include people or faces?
- Are captured frames retained anywhere, or processed in-memory and discarded once positions are extracted?
- Is the camera's field of view physically constrained (mounting, lens masking) to the controlled space, independent of what the software does?

These need concrete answers before Tier 1 moves from synthetic simulation to live camera input, not just a design intention.

### Can React Native handle 60fps 3D multi-agent rendering?

- **The Problem:** Pumping high-frequency spatial vectors across the React Native JSI (JavaScript Interface) bridge to the native UI thread can cause dropped frames.

- **The Solution:** The architecture leverages Expo's WebGL implementation. By ensuring the Rust/Wasm node does all the heavy behavioral calculations (the "attraction logic") and only transmits final `[x, y, z]` coordinates, the mobile GPU strictly handles rendering, keeping the client-side frame rate smooth.
