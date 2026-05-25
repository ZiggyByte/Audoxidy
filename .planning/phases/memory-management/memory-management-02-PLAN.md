---
phase: memory-management
plan: 02
type: execute
wave: 1
depends_on: []
files_modified:
  - src/gui/app.rs
  - src/gui/playlist.rs
autonomous: false
requirements: [D-02, D-03, D-06, D-07]
user_setup: []

must_haves:
  truths:
    - "After scan finishes, memory purge waits 40 seconds (not 30) before executing"
    - "When system RAM exceeds 75%, GlobalMemoryPurge fires immediately from Tick handler"
    - "PlaylistManager.unload preserves active tab data even when not focused/playing"
    - "Audio buffers are purged on both Stop and PlayPause (pause direction) — verified"
  artifacts:
    - path: "src/gui/app.rs"
      provides: "Hard cap RAM check in Tick handler"
      min_lines: 4830
      contains: "is_ram_over_hard_cap"
    - path: "src/gui/app.rs"
      provides: "Post-scan delay at 40 seconds"
      contains: "40"
    - path: "src/gui/playlist.rs"
      provides: "Granular unload with active tab tracking"
      contains: "is_active_tab"
  key_links:
    - from: "Message::Tick handler"
      to: "MemoryManager::is_ram_over_hard_cap"
      via: "RAM threshold check before timer"
      pattern: "is_ram_over_hard_cap"
    - from: "Message::Tick handler"
      to: "scan_finished_at delay"
      via: "30 → 40 seconds"
      pattern: "40"
    - from: "playlist_manager.unload"
      to: "is_active_tab parameter"
      via: "keeps data when active tab flag is true"
      pattern: "is_active_tab"
---

<objective>
**Integrate memory management changes into the app's update loop: raise post-scan delay, add hard cap RAM check, implement granular playlist unloading, and verify buffer purge on pause/stop.**

Purpose: Wire the core memory_manager.rs changes from Plan 01 into the application:
- D-02: Post-scan delay moves from 30s → 40s
- D-03 (integration): Tick handler checks RAM > 75% and forces GlobalMemoryPurge
- D-06: PlaylistManager.unload() becomes granular — keeps active tab data 
- D-07: Verify that audio buffer purge already fires on Stop and PlayPause (pause)

Output: Modified `src/gui/app.rs` and `src/gui/playlist.rs`
</objective>

<execution_context>
@/home/Tix/Proyectos/Audoxidy/Audoxidy/.opencode/get-shit-done/workflows/execute-plan.md
@/home/Tix/Proyectos/Audoxidy/Audoxidy/.opencode/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/memory-management/memory-management-CONTEXT.md
@/home/Tix/Proyectos/Audoxidy/Audoxidy/src/gui/app.rs
@/home/Tix/Proyectos/Audoxidy/Audoxidy/src/gui/playlist.rs

# Related plans:
# Plan 01 adds FIXED_PURGE_INTERVAL_SECS, RAM_HARD_CAP_PERCENT, is_ram_over_hard_cap()
# to src/utils/memory_manager.rs

# Code context:
# app.rs:886-891 — should_run_global_purge call (timer-based)
# app.rs:893-920 — post-scan detection and delay (currently 30s)
# app.rs:924-964 — GlobalMemoryPurge handler (calls unload)
# app.rs:934 — self.playlist_manager.unload(is_playlist_focused, is_playing || is_loaded_in_player)
# playground.rs:227-240 — current unload() implementation (todo o nada)
# app.rs:1063-1070 — PlayPause handler with purge_buffers on pause
# app.rs:1127-1131 — Stop handler with purge_buffers

# AGENTS.md conventions: AudioError enum, cargo check after changes
</context>

<tasks>

<task type="auto">
  <name>Task 1: Change post-scan delay from 30s to 40s</name>
  <files>src/gui/app.rs</files>
  <read_first>src/gui/app.rs:893-920</read_first>
  <action>
    In the `Message::Tick` handler, find the post-scan delay at line 910:
    ```rust
    if now.saturating_sub(finished_at) >= 30 {
    ```
    Change `30` to `40`. Per D-02: después de que el escáner termine, esperar 40 segundos antes de ejecutar la purga de carátulas y `GlobalMemoryPurge`.
    
    The surrounding context (lines 893-919):
    - `self.was_scanning` tracks scan transition (no change needed)
    - `self.scan_finished_at` stores scan-finished timestamp (no change needed)
    - The delay comparison on line 910 currently uses 30 — change to 40
    
    No other changes needed for D-02.
  </action>
  <verify>
    <automated>grep -n 'saturating_sub.*40' src/gui/app.rs | head -3</automated>
  </verify>
  <done>Line 910 now compares against 40 instead of 30. Post-scan delay is 40 seconds.</done>
</task>

<task type="auto">
  <name>Task 2: Add hard cap RAM check in Tick handler</name>
  <files>src/gui/app.rs</files>
  <read_first>src/gui/app.rs:880-923</read_first>
  <action>
    In the `Message::Tick` handler, add a hard cap RAM check BEFORE the existing timer-based purge check (line 886). Per D-03: si la RAM del sistema supera el 75%, forzar una purga inmediata sin esperar el ciclo de 2 minutos.
    
    Insert this block right before line 886 (`// 5. Sistema de Purga Automática`):
    ```rust
    // 5a. Hard Cap de RAM: si supera el 75%, purgar inmediatamente (D-03)
    if crate::utils::memory_manager::MemoryManager::is_ram_over_hard_cap() {
        println!("Audoxidy GC: RAM over 75% hard cap — forcing immediate purge");
        return Task::done(Message::GlobalMemoryPurge);
    }
    ```
    
    Then update the comment on the existing timer-based check to clarify it runs only when RAM is below the hard cap. Change:
    `// 5. Sistema de Purga Automática (Ciclo fijo de 2 minutos)`
    to:
    `// 5b. Purga por timer (solo cuando RAM está por debajo del hard cap)`
    
    The hard cap check must come FIRST so it bypasses the timer when RAM is critical. The existing should_run_global_purge call continues to handle the timer-based cycle.
  </action>
  <verify>
    <automated>grep -c 'is_ram_over_hard_cap' src/gui/app.rs && cargo check 2>&1 | tail -5</automated>
  </verify>
  <done>Hard cap check inserted before timer check. `cargo check` passes.</done>
</task>

<task type="auto">
  <name>Task 3: Granular playlist unloading with active tab tracking</name>
  <files>src/gui/playlist.rs, src/gui/app.rs</files>
  <read_first>src/gui/playlist.rs:227-240, src/gui/app.rs:924-964</read_first>
  <action>
    Modificar `PlaylistManager::unload()` de "todo o nada" a granular (D-06):
    
    **Step 1 — Modify `playlist.rs:227`:**
    Change the function signature from:
    ```rust
    pub fn unload(&mut self, is_focused: bool, is_playing: bool) {
    ```
    to:
    ```rust
    pub fn unload(&mut self, is_focused: bool, is_playing: bool, is_active_tab: bool) {
    ```
    
    Update the guard condition (line 228):
    ```rust
    if is_focused || is_playing || is_active_tab {
        return;
    }
    ```
    
    Update the doc comment to document the granular behavior:
    - Keeps data if focused, playing, or this is the active playlist tab
    - Only unloads when all three are false (tab inactive, not playing, not focused)
    
    **Step 2 — Update call site in `app.rs:934`:**
    Inside `Message::GlobalMemoryPurge`, change:
    ```rust
    self.playlist_manager.unload(is_playlist_focused, is_playing || is_loaded_in_player);
    ```
    to:
    ```rust
    // D-06: La playlist activa siempre se conserva en RAM
    let is_active_playlist_tab = true; // PlaylistManager siempre contiene la pestaña activa
    self.playlist_manager.unload(
        is_playlist_focused, 
        is_playing || is_loaded_in_player,
        is_active_playlist_tab,
    );
    ```
    
    The logic: the current PlaylistManager always holds data for the active tab. Per D-06, the active tab must be preserved even when focus is on Library and nothing is playing. This prevents the "empty tab" flash when switching back from Library.
    
    Run `cargo check` after both changes.
  </action>
  <verify>
    <automated>grep -c 'is_active_tab' src/gui/playlist.rs && grep -c 'is_active_tab' src/gui/app.rs && cargo check 2>&1 | tail -5</automated>
  </verify>
  <done>`unload()` takes `is_active_tab: bool`. Call site passes `true`. `cargo check` passes.</done>
</task>

<task type="checkpoint:human-verify">
  <name>Task 4: Verify D-07 buffer purge on pause/stop is already implemented</name>
  <files>N/A — verification only</files>
  <action>Read src/gui/app.rs and confirm that audio_manager.purge_buffers() is called on Stop (line 1127-1131), on PlayPause when pausing (line 1063-1070), and as backup in GlobalMemoryPurge (line 957-959). No code changes needed — D-07 is already implemented.</action>
  <verify>Read app.rs Stop handler, PlayPause handler, and GlobalMemoryPurge handler to confirm purge_buffers() calls exist in all three locations.</verify>
  <done>All three purge_buffers() call sites confirmed present in current codebase.</done>
  <how-to-verify>
    Per D-07, the following must already exist in src/gui/app.rs:
    
    1. **Message::Stop handler** (around line 1127-1131): Verify `self.audio_manager.purge_buffers()` is called after `self.audio_manager.stop()`.
       Expected: `let _ = self.audio_manager.purge_buffers(); // Purga profunda al detener`
    
    2. **Message::PlayPause handler** (around line 1063-1073): Verify `purge_buffers()` is called when `!is_playing()` (state changed from playing to paused).
       Expected: `if !self.audio_manager.is_playing() { ... self.audio_manager.purge_buffers(); ... }`
    
    3. **GlobalMemoryPurge handler** (around line 957-959): Verify backup purge exists.
       Expected: `if !is_playing { let _ = self.audio_manager.purge_buffers(); }`
    
    Read the relevant sections of app.rs and confirm all three are present. This task has no implementation work — it's a human verification checkpoint that D-07 is confirmed as already implemented in the current codebase.
  </how-to-verify>
  <resume-signal>Type "approved" — D-07 confirmed implemented</resume-signal>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| GUI update loop → OS | Reading /proc/meminfo via sysinfo crate in Tick (from is_ram_over_hard_cap) |
| GUI → Audio engine | Calling purge_buffers() on Stop/PlayPause recreates audio stream |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-mem2-01 | Denial of Service | is_ram_over_hard_cap in Tick | accept | Tick runs every 500ms; brief sys lock in get_ram_usage_percent may add ~1ms latency. Acceptable for GC. |
| T-mem2-02 | Repudiation | Post-scan purge delay | accept | 40s delay is sufficient for user to see new files; no logging changed. |
| T-mem2-SC | Tampering | crate installs | mitigate | No new package deps. Existing sysinfo already vendored. |
</threat_model>

<verification>
1. `cargo check` passes after all tasks.
2. grep for `is_ram_over_hard_cap` in app.rs — at least one match in Tick handler.
3. grep for `>= 40` in app.rs — post-scan delay.
4. grep for `is_active_tab` in playlist.rs and app.rs — at least one match each.
5. Manual verification of D-07 (Stop and PlayPause buffer purge calls).
</verification>

<success_criteria>
- [ ] Post-scan delay: `now.saturating_sub(finished_at) >= 40` (was 30)
- [ ] RAM hard cap check inserted before timer-based purge check in Tick handler
- [ ] `PlaylistManager::unload()` accepts `is_active_tab: bool` parameter
- [ ] Call site in `app.rs` passes `true` for `is_active_tab`
- [ ] D-07 confirmed already implemented (Stop + PlayPause + GlobalMemoryPurge all call purge_buffers)
- [ ] `cargo check` passes
</success_criteria>

<output>
Create `.planning/phases/memory-management/memory-management-02-SUMMARY.md` when done
</output>
