---
name: iec-ladder-bytecode
description: >-
  Use this skill when adding or modifying IEC 61131-3 Ladder Diagram elements,
  Global Symbol Table synchronization (GlobalSymbolEntry), IL code generation and
  parsing (il_format.rs), 32-bit Micro-VM bytecode compilation/decompilation
  (bytecode.rs / crates/vm), or differential simulation tests.
---

# IEC 61131-3 Ladder, Global Symbol Table & Bytecode Skill

This skill provides the architectural rules and verification steps for modifying the core PLC logic engine in **uPLC Studio**.

## Core Modules & Responsibilities

1. **Ladder Data Model & Global Symbol Table**: [ladder.rs](../../../crates/core/src/ladder.rs) & [document.rs](../../../crates/core/src/document.rs)
   - `LadderProgram::symbol_table` (`Vec<GlobalSymbolEntry>`) is the **Single Source of Truth** for address-to-symbol mappings across the project.
   - Calling `Document::set_global_symbol(address, symbol, comment)` normalizes the IEC/legacy address key (`normalize_address_key`) and propagates the symbol to all matching `LadderElement`s and `IOPortMapping`s.
2. **IEC 61131-3 IL Parser & Exporter**: [il_format.rs](../../../crates/core/src/il_format.rs)
   - Exports and parses standard `VAR ... END_VAR`, `VAR RETAIN ... END_VAR`, and `(* @NET_1: ... *)` network blocks.
   - Any new `TypeTag` or function block parameter must roundtrip cleanly through `export_to_il_str()` and `load_from_il_str()`.
3. **32-bit Fixed-Width Bytecode & Decompiler**: [bytecode.rs](../../../crates/core/src/bytecode.rs) & [lib.rs](../../../crates/vm/src/lib.rs)
   - Each instruction is encoded as 4 bytes `[op, addr, imm_lo, imm_hi]`.
   - Complex function blocks (`PID`, `SCALE`, `LIMIT`, `BITWISE`, `SHIFT`, `UART`, `MB_MASTER`) push auxiliary `OPND` (`0x6F`) instructions prior to the primary opcode (`FEATURE_LEVEL = 2`).
   - `BytecodeProgram::decompile_to_ladder()` reconstructs the visual Ladder Diagram directly from PLC bytecode uploaded via `CMD_UPLOAD_PROGRAM (0x04)`.

## Verification Checklist

Whenever modifying any file in `crates/core` or `crates/vm`:

1. Run the differential simulator vs. `uplc_vm` integration test suite:
   ```bash
   cargo test --test ladder_test
   ```
2. Verify all 10 bundled example `.il` files parse, compile to bytecode, and decompile with matching CRC16 signatures:
   ```bash
   cargo test --workspace
   ```
