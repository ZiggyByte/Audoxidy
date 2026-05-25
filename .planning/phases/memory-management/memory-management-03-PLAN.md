---
phase: memory-management
plan: 03
type: execute
wave: 1
depends_on: []
files_modified:
  - src/gui/library.rs
  - src/gui/library_filters.rs
autonomous: true
requirements: [D-04, D-05]
user_setup: []

must_haves:
  truths:
    - "Library viewport margin is 50% of viewport height (25% in low-resource mode) instead of fixed 100px/300px"
    - "Filter panel margin is max(5, total_visible_items / 2) instead of fixed 5 items"
  artifacts:
    - path: "src/gui/library.rs"
      provides: "Dynamic viewport-relative margin in get_visible_elements"
      contains: "viewport_h"
    - path: "src/gui/library.rs"
      provides: "Dynamic viewport-relative margin in get_visible_grid_elements"
      contains: "viewport_h"
    - path: "src/gui/library_filters.rs"
      provides: "Dynamic item-count-based margin in get_visible_tree_items"
      contains: "total_visible_items"
  key_links:
    - from: "get_visible_elements"
      to: "self.last_viewport.height"
      via: "margin = viewport_h × 0.5"
      pattern: "viewport_h.*0.5"
    - from: "get_visible_grid_elements"
      to: "viewport_h"
      via: "margin = viewport_h × 0.5 (low-res: 0.25)"
      pattern: "viewport_h.*0.5"
    - from: "get_visible_tree_items"
      to: "viewport_height / item_height"
      via: "margin = max(5, total_visible_items / 2)"
      pattern: "total_visible_items"
---

<objective>
**Replace fixed pixel safety margins in library and filter virtualization with dynamic, viewport-relative margins (50% of viewport height) and item-count-based margins for the filter panel.**

Purpose: 
- D-04: Replace 100px (detailed/thumbnail) and 300px (grid) fixed margins in `library.rs` with a dynamic margin equal to 50% of the current viewport height (`viewport_h × 0.5`). In low-resource mode, use half of that (25% of viewport). This prevents flickering during fast scrolling on large libraries and adjusts automatically to window size.
- D-05: Replace the fixed 5-item margin in `library_filters.rs` with `max(5, total_visible_items / 2)` where `total_visible_items = viewport_height / item_height`. This scales the margin to the visible content size instead of a hardcoded item count.

Output: Modified `src/gui/library.rs` and `src/gui/library_filters.rs`
</objective>

<execution_context>
@/home/Tix/Proyectos/Audoxidy/Audoxidy/.opencode/get-shit-done/workflows/execute-plan.md
@/home/Tix/Proyectos/Audoxidy/Audoxidy/.opencode/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/memory-management/memory-management-CONTEXT.md
@/home/Tix/Proyectos/Audoxidy/Audoxidy/src/gui/library.rs
@/home/Tix/Proyectos/Audoxidy/Audoxidy/src/gui/library_filters.rs

# Code locations:
# library.rs:564-771 — get_visible_elements() — detailed/thumbnail view
# library.rs:592 — margin = 100px (normal), 50px (low-resource)
# library.rs:773-914 — get_visible_grid_elements() — grid view
# library.rs:845-851 — lazy_margin = 300px (normal), 150px (low-resource)
# library_filters.rs:295-334 — get_visible_tree_items() — filter panel
# library_filters.rs:311 — margin = 5 (fixed items)
# library_filters.rs:314 — visible_count uses margin * 2

# utils/mod.rs:43 — is_low_resource() helper
# Each function already captures last_viewport for scroll management
</context>

<tasks>

<task type="auto">
  <name>Task 1: Dynamic viewport margin in get_visible_elements (detailed/thumbnail)</name>
  <files>src/gui/library.rs</files>
  <read_first>src/gui/library.rs:564-771</read_first>
  <action>
    In `get_visible_elements()` at `library.rs:591-594`, replace the fixed pixel margins with dynamic viewport-relative margins:
    
    **Current code (lines 591-594):**
    ```rust
    // Margen reducido para menos redraws — la virtualización manual es eficiente
    let margin = if crate::utils::is_low_resource() { 50.0 } else { 100.0 };
    let render_min = view_min - margin;
    let render_max = view_max + margin;
    ```
    
    **Replace with:**
    ```rust
    // D-04: Margen dinámico = 50% del viewport (25% en low-resource)
    let viewport_margin_ratio: f32 = if crate::utils::is_low_resource() { 0.25 } else { 0.5 };
    let margin = (viewport_h * viewport_margin_ratio).max(50.0); // mínimo 50px para safety
    let render_min = view_min - margin;
    let render_max = view_max + margin;
    ```
    
    The `.max(50.0)` ensures a minimum safety margin of 50px even on very small viewports. `viewport_h` is already available from line 580-584 as the `last_viewport.height` value.
    
    Note: `viewport_margin_ratio` uses `f32` because `viewport_h` is `f32`. The `.max(50.0)` comparison is fine since both are `f32`.
    
    Run `cargo check` after the change.
  </action>
  <verify>
    <automated>grep -c 'viewport_margin_ratio\|viewport_h.*0.5' src/gui/library.rs && cargo check 2>&1 | tail -5</automated>
  </verify>
  <done>`get_visible_elements()` uses `viewport_h * 0.5` for margin. `cargo check` passes.</done>
</task>

<task type="auto">
  <name>Task 2: Dynamic viewport margin in get_visible_grid_elements (grid view)</name>
  <files>src/gui/library.rs</files>
  <read_first>src/gui/library.rs:773-914</read_first>
  <action>
    In `get_visible_grid_elements()` at `library.rs:844-851`, replace the fixed pixel margins with dynamic viewport-relative margins:
    
    **Current code (lines 844-851):**
    ```rust
    // 2. Filtrar por visibilidad — margen reducido (grid mide más alto por filas de álbumes)
    let lazy_margin = if crate::utils::is_low_resource() {
        150.0
    } else {
        300.0
    };
    let render_min = viewport_y - lazy_margin;
    let render_max = viewport_y + viewport_h + lazy_margin;
    ```
    
    **Replace with:**
    ```rust
    // D-04: Margen dinámico en grid = 50% del viewport (25% en low-resource)
    let grid_margin_ratio: f32 = if crate::utils::is_low_resource() { 0.25 } else { 0.5 };
    let lazy_margin = (viewport_h * grid_margin_ratio).max(100.0); // mínimo 100px para filas de álbumes
    let render_min = viewport_y - lazy_margin;
    let render_max = viewport_y + viewport_h + lazy_margin;
    ```
    
    The `.max(100.0)` ensures grid rows (which are taller — 252px per row) get at least a 100px safety margin. `viewport_h` is already a parameter of this function.
    
    Run `cargo check` after the change.
  </action>
  <verify>
    <automated>grep -c 'grid_margin_ratio\|viewport_h.*grid' src/gui/library.rs && cargo check 2>&1 | tail -5</automated>
  </verify>
  <done>`get_visible_grid_elements()` uses `viewport_h * 0.5` for margin. `cargo check` passes.</done>
</task>

<task type="auto">
  <name>Task 3: Dynamic filter margin in get_visible_tree_items</name>
  <files>src/gui/library_filters.rs</files>
  <read_first>src/gui/library_filters.rs:295-334</read_first>
  <action>
    In `get_visible_tree_items()` at `library_filters.rs:309-314`, replace the fixed 5-item margin with a dynamic margin based on visible item count:
    
    **Current code (lines 309-314):**
    ```rust
    let start_index = (scroll_y / item_height).floor() as usize;
    // Margen de seguridad para evitar parpadeos
    let margin = 5;
    let start_index = start_index.saturating_sub(margin);
    
    let visible_count = (viewport_height / item_height).ceil() as usize + (margin * 2);
    ```
    
    **Replace with:**
    ```rust
    let start_index = (scroll_y / item_height).floor() as usize;
    // D-05: Margen dinámico = max(5, total_visible_items / 2)
    let total_visible_items = (viewport_height / item_height).ceil() as usize;
    let margin = (total_visible_items / 2).max(5);
    let start_index = start_index.saturating_sub(margin);
    
    let visible_count = (viewport_height / item_height).ceil() as usize + (margin * 2);
    ```
    
    Per D-05: `max(5, total_visible_items / 2)` where `total_visible_items = viewport_height / item_height`. The `/ 2` gives half the visible items as margin (so if 20 items are visible, margin is 10; if 8 are visible, margin is max(5,4) = 5). The `max(5)` ensures a minimum margin of 5 items even on tiny viewports.
    
    Run `cargo check` after the change.
  </action>
  <verify>
    <automated>grep -c 'total_visible_items' src/gui/library_filters.rs && cargo check 2>&1 | tail -5</automated>
  </verify>
  <done>`get_visible_tree_items()` uses `max(total_visible_items / 2, 5)`. `cargo check` passes.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| GUI virtualización | No external data crosses trust boundary — purely local rendering optimization |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-mem3-01 | Denial of Service | Margin calculation | accept | O(1) arithmetic, no performance concern. |
| T-mem3-SC | Tampering | crate installs | mitigate | No new packages. Pure Rust arithmetic. |
</threat_model>

<verification>
1. `cargo check` passes after all tasks.
2. Verify margins in library.rs:
   - `get_visible_elements`: grep for `viewport_h * 0.5` pattern in context of margin calculation.
   - `get_visible_grid_elements`: grep for `viewport_h * 0.5` pattern in context of grid margin.
   - Low-resource: check `0.25` appears in both margin calculations.
3. Verify margin in library_filters.rs:
   - `get_visible_tree_items`: grep for `total_visible_items / 2` with `.max(5)` pattern.
</verification>

<success_criteria>
- [ ] `get_visible_elements()` margin = `viewport_h × 0.5` (0.25 low-resource) clamped to minimum 50px
- [ ] `get_visible_grid_elements()` margin = `viewport_h × 0.5` (0.25 low-resource) clamped to minimum 100px
- [ ] `get_visible_tree_items()` margin = `max(total_visible_items / 2, 5)` where `total_visible_items = viewport_height / item_height`
- [ ] No fixed pixel constants (100px, 300px, 5 items) remain in the margin logic of these functions
- [ ] `cargo check` passes
</success_criteria>

<output>
Create `.planning/phases/memory-management/memory-management-03-SUMMARY.md` when done
</output>
