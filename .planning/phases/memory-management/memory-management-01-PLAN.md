---
phase: memory-management
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/utils/memory_manager.rs
autonomous: true
requirements: [D-01, D-03]
user_setup: []

must_haves:
  truths:
    - "Global memory purge runs on a strict 120-second fixed timer, not an adaptive RAM-based interval"
    - "RAM usage above 75% triggers an immediate forced purge regardless of the timer"
    - "The dynamic interval function is removed (dead code replaced by fixed constant)"
  artifacts:
    - path: "src/utils/memory_manager.rs"
      provides: "Fixed purge interval + hard cap function"
      min_lines: 110
      contains: "FIXED_PURGE_INTERVAL_SECS"
    - path: "src/utils/memory_manager.rs"
      provides: "Hard cap public API"
      contains: "is_ram_over_hard_cap"
  key_links:
    - from: "should_run_global_purge"
      to: "FIXED_PURGE_INTERVAL_SECS"
      via: "120-second fixed interval comparison"
      pattern: "FIXED_PURGE_INTERVAL_SECS"
    - from: "is_ram_over_hard_cap"
      to: "get_ram_usage_percent"
      via: "returns true when RAM > 75%"
      pattern: "RAM_HARD_CAP_PERCENT"
---

<objective>
**Replace adaptive RAM-based purge interval with a fixed 120-second cycle and add a hard cap function that identifies when RAM exceeds 75% for immediate forced purging.**

Purpose: Remove the adaptive `get_dynamic_purge_interval_mins()` logic (D-01) and add a hard cap utility function `is_ram_over_hard_cap()` (D-03) that the app's Tick handler will use to force immediate purges when system RAM exceeds 75%.

Output: Modified `src/utils/memory_manager.rs` with:
- Constants `FIXED_PURGE_INTERVAL_SECS` (120) and `RAM_HARD_CAP_PERCENT` (75.0)
- `should_run_global_purge()` using fixed interval, not dynamic
- `is_ram_over_hard_cap()` public function
- Removal of `get_dynamic_purge_interval_mins()` (dead code)
</objective>

<execution_context>
@/home/Tix/Proyectos/Audoxidy/Audoxidy/.opencode/get-shit-done/workflows/execute-plan.md
@/home/Tix/Proyectos/Audoxidy/Audoxidy/.opencode/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/memory-management/memory-management-CONTEXT.md
@/home/Tix/Proyectos/Audoxidy/Audoxidy/src/utils/memory_manager.rs

# Referenced by this plan:
# AGENTS.md: AudioError enum pattern, parking_lot::RwLock, std::sync::Mutex conventions
# memory_manager.rs:25 — get_ram_usage_percent() already exists and is reusable
# memory_manager.rs:78 — should_run_global_purge() currently uses get_dynamic_purge_interval_mins()
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add fixed-purgue constants to MemoryManager impl</name>
  <files>src/utils/memory_manager.rs</files>
  <read_first>src/utils/memory_manager.rs</read_first>
  <action>
    Add two new constants after the existing `static SYSINFO` declaration (line 13):
    
    1. `const FIXED_PURGE_INTERVAL_SECS: u64 = 120;` — replaces the dynamic interval logic. Per D-01: strict 2-minute cycle, no backoff or adaptive logic.
    
    2. `const RAM_HARD_CAP_PERCENT: f64 = 75.0;` — threshold for hard cap. Per D-03: if RAM exceeds this, force immediate purge.
    
    Place them between `static SYSINFO` and `fn get_sysinfo()`. Use uppercase snake_case per Rust naming conventions.
  </action>
  <verify>
    <automated>grep -c 'FIXED_PURGE_INTERVAL_SECS' src/utils/memory_manager.rs && grep -c 'RAM_HARD_CAP_PERCENT' src/utils/memory_manager.rs</automated>
  </verify>
  <done>Both constants exist in the file at the module level.</done>
</task>

<task type="auto">
  <name>Task 2: Replace dynamic interval with fixed 120s in should_run_global_purge</name>
  <files>src/utils/memory_manager.rs</files>
  <read_first>src/utils/memory_manager.rs</read_first>
  <action>
    Refactor the `should_run_global_purge(_interval_mins: u64, is_scanning: bool) -> bool` function:
    
    1. Remove the internal call to `get_dynamic_purge_interval_mins()` that currently exists at line 93.
    2. Use `FIXED_PURGE_INTERVAL_SECS` (120) directly for the timer comparison.
    3. Keep the `_interval_mins` parameter but rename it to `_ignored: u64` to avoid unused-variable warnings (we keep it for backward-compatible call signature).
    4. The comparison at line 95 changes from `interval_mins * 60` to `FIXED_PURGE_INTERVAL_SECS`.
    
    Also update the function doc comment (line 76-77): remove reference to dynamic calculation, document that it uses a fixed 120s interval per D-01.
    
    After this change, `get_dynamic_purge_interval_mins()` becomes dead code (no callers).
  </action>
  <verify>
    <automated>grep -c 'FIXED_PURGE_INTERVAL_SECS' src/utils/memory_manager.rs && cargo check 2>&1 | tail -5</automated>
  </verify>
  <done>`should_run_global_purge` uses FIXED_PURGE_INTERVAL_SECS. `cargo check` passes.</done>
</task>

<task type="auto">
  <name>Task 3: Remove get_dynamic_purge_interval_mins function</name>
  <files>src/utils/memory_manager.rs</files>
  <read_first>src/utils/memory_manager.rs</read_first>
  <action>
    Remove the `get_dynamic_purge_interval_mins() -> u64` function (lines 40-51) and its doc comment. This function was only called by `should_run_global_purge` which has been refactored in Task 2 to use the fixed constant instead. Per D-01, the interval must NOT be dynamic — keeping dead code that expresses the opposite intent is misleading.
    
    Remove the full function block including the `pub fn get_dynamic_purge_interval_mins()` declaration and its body.
    
    After deletion, run `cargo check` to verify no remaining references to this function exist.
  </action>
  <verify>
    <automated>cargo check 2>&1 | grep -v 'warning:' | grep -v '^$' | tail -3</automated>
  </verify>
  <done>`get_dynamic_purge_interval_mins` removed. `cargo check` passes with no errors related to this removal.</done>
</task>

<task type="auto">
  <name>Task 4: Add is_ram_over_hard_cap public function</name>
  <files>src/utils/memory_manager.rs</files>
  <read_first>src/utils/memory_manager.rs</read_first>
  <action>
    Add a new public function `is_ram_over_hard_cap() -> bool` to the `impl MemoryManager` block (after `force_free_to_os` at line 119, before `execute_global_purge` at line 122):
    
    ```rust
    pub fn is_ram_over_hard_cap() -> bool {
        let ram_pct = get_ram_usage_percent();
        ram_pct > RAM_HARD_CAP_PERCENT
    }
    ```
    
    Per D-03: this uses the existing `get_ram_usage_percent()` (line 23) which already handles the sysinfo lock and memory refresh. No need to re-fetch system info.
    
    Document that this is called by the app's Tick handler to trigger immediate `GlobalMemoryPurge` when the system exceeds 75% RAM usage. The hard cap bypasses the 2-minute purge cycle timer.
    
    Run `cargo check` after adding.
  </action>
  <verify>
    <automated>grep -c 'is_ram_over_hard_cap' src/utils/memory_manager.rs && cargo check 2>&1 | tail -3</automated>
  </verify>
  <done>`is_ram_over_hard_cap()` exists and is public. `cargo check` passes.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| memory_manager → OS sysinfo | reading /proc/meminfo via sysinfo crate |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-mem-01 | Information Disclosure | get_ram_usage_percent | accept | Reads /proc/meminfo — no PII, low-value. sysinfo crate is vetted. |
| T-mem-SC | Tampering | crate installs | mitigate | No new package deps added. Existing sysinfo already vendored. |
</threat_model>

<verification>
1. `cargo check` passes after all tasks.
2. `cargo clippy` passes (run after all tasks complete).
3. grep for `get_dynamic_purge_interval_mins` — must return zero matches.
4. grep for `FIXED_PURGE_INTERVAL_SECS` — must return at least one match in `should_run_global_purge`.
5. grep for `is_ram_over_hard_cap` — must return at least one match.
</verification>

<success_criteria>
- [ ] `should_run_global_purge` compares elapsed time against `FIXED_PURGE_INTERVAL_SECS` (120), not against a dynamic RAM-based interval
- [ ] `get_dynamic_purge_interval_mins()` removed (zero references in codebase)
- [ ] `is_ram_over_hard_cap()` returns `true` when `get_ram_usage_percent() > 75.0`
- [ ] All existing callers of `should_run_global_purge` still compile (no API-breaking change)
- [ ] `cargo check` passes
</success_criteria>

<output>
Create `.planning/phases/memory-management/memory-management-01-SUMMARY.md` when done
</output>
