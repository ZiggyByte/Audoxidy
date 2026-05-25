Here's the plan for the updated AGENTS.md:
Structure
1. Build / Verify / Lint (top of file)
- cargo check — fast compile check (always run after changes)
- cargo build --release — release build with panic = 'abort', LTO, codegen-units = 1
- cargo fmt + cargo clippy — run before PRs
- cargo test — currently broken (14 errors, test module can't access private engine functions)
2. Architecture (short bullets)
- Single binary crate (not a workspace), Rust edition 2024, entrypoint: src/main.rs
- GUI is iced 0.14 with iced_aw — not egui (README is stale here)
- src/audio/ — audio engine, DSP chain, manager wrapper
- src/db/ — SQLite via rusqlite (bundled), scanner
- src/gui/ — iced app, theme, player, playlist, library, audio center
- src/integrations/ — souvlaki media controls (MPRIS/SMTC)
- src/utils/ — cover art, string interner, memory manager, formatting helpers
- Database file library.db + WAL/SHM are gitignored, auto-created at runtime
3. Threading & Locking Conventions
- Background decode thread spawned in AudioEngine::new(), reads crossbeam::unbounded channel commands, writes to ringbuf::HeapRb<f32> (8 MiB)
- CPAL output callback reads from ringbuf, writes to device
- parking_lot::RwLock for AudioState and DspChain
- parking_lot::Mutex for ringbuf handles  
- std::sync::Mutex for Database (shared GUI + audio)
- Never use ringbuf::arc module — ringbuf crate is for audio engine only (existing AGENT.md warns about this)
4. Key Gotchas
- Ringbuf 0.4 type aliases: HeapProducer<T> = Caching<Arc<HeapRb<T>>, true, false>, HeapConsumer<T> = Caching<Arc<HeapRb<T>>, false, true> (defined in src/audio/engine.rs:19-20)
- rust-version is whatever stable supports edition 2024
- No CI, no pre-commit hooks, no toolchain pinning
- library.db* is gitignored — it's a runtime artifact
5. Other Conventions (from AGENT.md, verified)
- Run cargo check after every implementation
- Check crate docs for DSP/audio/GUI packages before coding against them (API changes between versions)
- Help/planning files should be in Spanish (not code files)
- Code for high performance, robustness, security, clean maintainability
6. ## Build / Verify / Lint

```bash
cargo check                # fast compile check (run after every change)
cargo build --release      # release: panic='abort', LTO, codegen-units=1
cargo fmt && cargo clippy  # before PRs
cargo test                 # 116 tests, all passing
```

## Architecture

- Single binary crate (not a workspace), Rust edition 2024, entrypoint: `src/main.rs`
- GUI is **`iced` 0.14** + `iced_aw` — NOT egui (README is stale)
- `src/audio/` — audio engine, DSP chain, manager wrapper, error types, decoder
- `src/db/` — SQLite via `rusqlite` (bundled), scanner
- `src/gui/` — iced app, theme, player, playlist, library, audio center, widgets
- `src/integrations/` — souvlaki media controls (MPRIS/SMTC)
- `src/utils/` — cover art (AVIF), string interner (with auto-purge), memory manager, formatting helpers
- `library.db` + WAL/SHM are gitignored, auto-created at runtime

## Threading & Locking Conventions

- Background decode thread spawned in `AudioEngine::new()`, reads `crossbeam::unbounded` channel, writes to `ringbuf::HeapRb<f32>`
- CPAL output callback reads from ringbuf, writes to device
- `parking_lot::RwLock` for `AudioState` and `DspChain` (hot-shared state)
- `parking_lot::Mutex` for ringbuf handles
- `std::sync::Mutex` for `Database` (shared GUI + audio)
- **Never use `ringbuf::arc` module** — ringbuf crate is for audio engine only

## Key Gotchas

- Ringbuf 0.4 type aliases in `src/audio/engine.rs:19-20`:
  `HeapProducer<T>` = `Caching<Arc<HeapRb<T>>, true, false>`
  `HeapConsumer<T>` = `Caching<Arc<HeapRb<T>>, false, true>`
- `mix_channels_planar` takes 7 args, writes output to `&mut Vec<f64>` (returns `()`)
- `AudioError` enum (not `String`) for all audio module results — defined in `src/audio/error.rs`
- No CI, no pre-commit hooks, no toolchain pinning
- Rust version: whatever stable supports edition 2024

## Error Handling

- `src/audio/error.rs` — `AudioError` enum with Display + Error trait impls
- All audio functions return `Result<_, AudioError>` instead of `Result<_, String>`
- External callers can use `.expect()` or `format!("{e}")` transparently

## Testing

```bash
cargo test                 # 108 tests: 22 engine, 46 DSP, 2 channel mix, 38 utils
```
- Test files: `src/audio/tests.rs`, `src/audio/dsp_tests.rs`, inline `#[cfg(test)]` in `src/utils/mod.rs`
- DSP tests use `#[path = "audio/dsp_tests.rs"]` declaration in `src/main.rs`

## Other Conventions

- Run `cargo check` after every implementation
- Help/planning files in Spanish (not code files)
- Code for high performance, robustness, security, clean maintainability
- `PLAN_IMPLEMENTACION.md` — detailed implementation roadmap (Spanish)

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

When the user types `/graphify`, invoke the `skill` tool with `skill: "graphify"` before doing anything else.

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- Dirty graphify-out/ files are expected after hooks or incremental updates; dirty graph files are not a reason to skip graphify. Only skip graphify if the task is about stale or incorrect graph output, or the user explicitly says not to use it.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
