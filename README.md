# uPLC Studio

A modern, high-performance Rust Ladder Diagram (PLC) editor and real-time simulator inspired by **MICROPLC** and **LDMICRO**.

## Features

- **Ladder Logic Editor**:
  - Industrial Dark CAD Canvas with Power Rails (L1 Hot Rail & L2 Ground Rail).
  - Rung management (Add, Insert, Move Up/Down, Delete).
  - Elements:
    - Contacts: Normally Open (`--| |--`), Normally Closed (`--|/|--`)
    - Coils: Normal (`--(--)--`), Set Latch (`--(S)--`), Reset Unlatch (`--(R)--`), Negated
    - Timers: `TON` (On-Delay), `TOF` (Off-Delay)
    - Counters: `CTU` (Up Counter), `CTD` (Down Counter), `CTC`
    - Branches: Parallel seal-in / bypass logic (`[┬] PARALLEL`)
    - Math & Comparison: `ADD`, `SUB`, `CMP`, `MOV`
    - Rung Comments: `//`
- **Real-Time Simulation Engine**:
  - Cyclic 10ms logic scan.
  - Live **Power Flow Visualization**: wires and energized elements glow bright RED when active.
  - **Live Virtual PLC Rack**:
    - Digital Inputs `X0`–`X7` (interactive toggle switches)
    - Digital Outputs `Y0`–`Y7` (LED indicator lights)
    - Internal Relays / Markers `M0`–`M15`
    - Live Timer Accumulators and Done status
  - Click on contacts directly in the diagram to toggle inputs during simulation!
- **OpenPLC & CODESYS Compatibility (IEC 61131-3)**:
  - Open & export standard IEC 61131-3 Instruction List (`.il`) files.
  - Transparent auto-detection between IEC 61131-3 IL and legacy formats.
  - Hardware mapping (Arduino UNO R3, Nano, Mega, ESP32).
  - Live preview & export of IEC 61131-3 IL source code.

## How to Run

```bash
cargo run -p uplc-studio
```

## Run Tests

```bash
cargo test
```

## Build Release

```bash
cargo build --release -p uplc-studio
```
