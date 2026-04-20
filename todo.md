# TODO

## Next day:

- fix proto: 5B or 9B (for data BP flags)?
- git commit name :-( -> ask

## General

- proper logging (eg tracing crate)
- more extensive docs (and readme)
- clean exit on either connection drop
- mby stuff some channel inside the sesh or?
- MSIM stopped at: add reason
- msim event vs resp interpretation fiasco

## Feats

- make MSIM listener handle MSIM reload etc gracefully (so IDE can hold on)

- Pause
- SetFunctionBreakpoints (just convert to regular BPs)
- SetInstructionBreakpoints
- BreakpointLocations
- register query / display
- Next
- Restart, Terminate - should add MSIM support too
- Threads
- run in terminal launch MSIM (have to add to dap-rs - PR)

### Moonshots
- Variables
- Disassemble
- LoadedSources
- SetVariable/SetExpression
- opt-in kernel threads via custom debuggging device

---

## Claude's clean TODOs

### Immediate (before merge)
- Fix `.expect()` panics in `attach()` and `launch()` — should be `?`, currently bypasses error handling
- `set_breakpoints` should push unverified breakpoints (`verified: false` + message) instead of silently dropping them — DAP spec requires this
- Update `Debugger::run()` to match new `DebugEventResult` channel type (`Ok(Ok(e))` / `Ok(Err(fatal))` / `Err(_)`)
- Remove `unreachable!()` from `handle_event` — `FatalError` variant no longer exists on `DebugEvent`

### Protocol redesign (do before adding new request types)
- Extend MSIM binary protocol to 9-byte fixed frames: `[type u8][arg_a u32][arg_b u32]`
  - `arg_a` = address / primary argument, `arg_b` = secondary (count, flags, reg_id etc.), unused = 0
  - Fixed frame keeps C receiver trivially simple (`read(fd, buf, 9)`, no conditional buffering)
  - Covers all planned request types without variable-length complexity

### Near-term features (no new MSIM protocol needed)
- `SetFunctionBreakpoints` — resolve function name → entry address via DWARF, reuse existing `SetBreakpoint`. Zero protocol cost.
- `SetInstructionBreakpoints` — raw instruction address → `SetBreakpoint` directly, skip DWARF
- `BreakpointLocations` — pure DWARF query, return valid breakpoint positions for a source range
- `Pause` — single type byte, no args. Needs one new MSIM request type, trivial on both sides.
- `StoppedAt` reason field — extend MSIM event to carry stop reason (breakpoint, step, pause)

### Near-term features (needs new MSIM protocol)
- `GetRegister(id) → RegVal(val)` — fixed 9-byte request/response, reg_id in arg_a
  - Live registers only (currently executing thread)
  - Feeds into DAP `Scopes` + `Variables` to display registers in IDE
- Stepping: `Next`, `StepIn`, `StepOut` — arg_a = step kind or separate type bytes, no address needed
- `Restart` / `Terminate` — needs MSIM-side support, straightforward

### Moonshots
- `ReadMemory` — chunked fixed-size reads (e.g. 64 bytes/chunk), adapter handles iteration
  - Prerequisite for disassembly, memory inspection, and logical thread support
- `Disassemble` — fetch memory chunks via `ReadMemory`, disassemble in adapter using a Rust crate (e.g. `riscv` disassembler since MSIM is RISC-V)
- `Variables` / `Scopes` — full inspection chain: `StackTrace` → `Scopes` → `Variables`
  - Live registers via `GetRegister`, stack via `ReadMemory` + DWARF unwind
- `SetVariable` / `SetExpression` — write registers/memory, lower priority than reads
- `LoadedSources` — list source files known to DWARF index

### Logical kernel threads (opt-in, long-term)
- Design a MSIM debug device mapped to a magic high address range (same pattern as the stdout printer device, configurable in MSIM)
- Students opt in by writing thread metadata to known addresses: `(thread_id, pc, sp, name_ptr)` on context switch
- Device accumulates a thread table; adapter reads it on DAP `Threads` request
- Sleeping threads expose saved register context via `ReadMemory` of their saved context block (not hardware registers)
- No hardcoded memory layout convention needed — device interface is the contract
- Could intercept context switches automatically if students register their scheduler entry point with the device