# Phase 01: Mejora de Sliders y Navegación en Centro de Audio

**Gathered:** 2026-06-02
**Status:** Ready for planning

<domain>
## Phase Boundary

Mejora del widget `CustomSlider` y sistema de navegación por teclado en el Centro de Audio Avanzado. Separación de estados visuales (focus/hover), resolución de conflictos de teclado entre sliders y módulos de biblioteca/playlist, implementación de input directo de valores, y personalización de estilos para sliders del ecualizador. **No incluye cambios en algoritmos DSP.**

**Relación con Phase 01 original:** La `01-CONTEXT.md` existente cubre mejoras DSP (compresor, limitador, reverb). Este documento complementa con mejoras UI/widget del mismo centro de audio. Ambos deben implementarse de forma compatible.

</domain>

<decisions>
## Implementation Decisions

### 1. Separación Focus / Hover

**Definición de estados:**

| Estado | Activación | Visual | Teclado |
|--------|-----------|--------|---------|
| **Focus** | Mouse sobre el slider (sin clic) | Handle: `handle_focus_color` | NO |
| **Hover** | Clic en el slider | Handle: `handle_hover_color` + borde interno | SÍ |
| Normal | Ninguno | Handle: `handle_color` | NO |

- **Focus** es puramente visual. No tiene ninguna función de activación de teclado.
- **Focus** se activa cuando el puntero del mouse entra en los límites del slider. Se pierde cuando el mouse sale del slider.
- **Hover** se activa SOLO con clic en el slider. NO se activa con solo pasar el mouse.
- **Hover** se pierde al hacer clic en OTRO slider o en espacio vacío del centro de audio.
- El borde del hover es INSET (interno), dibujado dentro del handle reduciendo el área de color.

**Nuevos campos en `CustomSliderOptions`:**

```
handle_focus_color: Option<Color>    // Default: None → COLOR_TEXT_PRIMARY
handle_hover_color: Option<Color>    // Default: None → COLOR_TEXT_SECONDARY
border_focus_color: Option<Color>    // Default: None → transparente
border_focus_width: f32              // Default: 0.0
border_hover_color: Option<Color>    // Default: None → COLOR_ACCENT
border_hover_width: f32              // Default: 0.0 (pero EQ disabled setea 2.0)
```

**Comportamiento legacy (migración):**
- El `handle_hover_color` existente se mapea al nuevo `handle_focus_color` (ya que el bug actual hacía que hover = mouse over, que ahora es focus).
- El nuevo `handle_hover_color` se usa para el estado de clic activo.
- Todos los sliders existentes deben revisarse: si usaban `handle_hover_color` para efecto visual de hover-mouse, ahora deben usar `handle_focus_color`.

### 2. Conflicto de Teclado Slider vs Biblioteca/Playlist

**Mecanismo: `AppFocus::AudioCenter` vía callback**

- Agregar a `CustomSlider` el callback opcional:
  ```rust
  on_hover_state_change: Option<Box<dyn Fn(bool) -> Message + 'a>>
  ```
- Se dispara con `true` cuando el slider recibe clic (hover activado).
- Se dispara con `false` cuando se pierde hover (clic en otro slider o espacio vacío).
- Audio center conecta a `AudioCenterMessage::SliderHoverActive(bool)`.
- `app.rs` maneja:
  - `SliderHoverActive(true)` → `self.focus = AppFocus::AudioCenter`
  - `SliderHoverActive(false)` → restaura el `AppFocus` anterior (Library o Playlist)
- El slider consume eventos de flecha a nivel widget (retorna `Status::Captured`) SOLO cuando tiene hover activo.
- En `GlobalKeyDown` handler: si `self.focus == AppFocus::AudioCenter`, NO pasar flechas a ningún módulo.
- **Eliminar** completamente el guard de 150ms (`last_audio_center_key_event` y todas sus referencias en `app.rs`).
- Cuando el centro de audio está abierto pero ningún slider tiene hover activo, las flechas fluyen normalmente a library/playlist.
- Cubre TODOS los sliders: EQ y efectos DSP por igual.

### 3. Input de Valores por Teclado

**Activación:** `enable_keyboard_input = true` en `CustomSliderOptions`. El input es SIEMPRE visible junto al slider cuando esta flag está activa. No requiere acción del usuario para aparecer.

**Posición (input_position):**
- Enum: `InputPosition { Top, Bottom, Left, Right }`
- Default automático según orientación del slider:
  - `Horizontal` → `Right`
  - `Vertical` → `Bottom`
- Personalizable por slider si se requiere posición específica.

**Layout:**
- Alineado lado a lado (o arriba/abajo según posición).
- Gap entre slider e input: `input_gap: f32` (default 10.0), personalizable.

**Tamaño (input_width):**
- Enum: `InputWidth { Auto, Fixed(f32) }`
- `Auto` (default): el input se ajusta al ancho del valor formateado.
- `Fixed(f32)`: ancho fijo en píxeles.

**Estilos default y campos:**

| Campo | Tipo | Default |
|-------|------|---------|
| `input_bg_color` | `Option<Color>` | `Some(COLOR_BG)` → #000000 |
| `input_border_color` | `Option<Color>` | `Some(COLOR_TEXT_SECONDARY)` → #5B5B5B |
| `input_border_focus_color` | `Option<Color>` | `Some(COLOR_ACCENT)` → #FF003D (solo al escribir) |
| `input_border_width` | `f32` | 1.0 |
| `input_border_radius` | `f32` | 4.0 |
| `input_font_size` | `f32` | 13.0 |
| `input_font_color` | `Option<Color>` | `Some(COLOR_TEXT_PRIMARY)` → #AFAFAF |
| `input_gap` | `f32` | 10.0 |
| `input_width` | `InputWidth` | `Auto` |

**Comportamiento:**
- `input_border_focus_color` se muestra SOLO cuando el usuario hace clic en el input para escribir. No cambia en hover del input.
- Confirmar valor: tecla `Enter` o clic en espacio vacío.
- Valor inválido (no numérico o fuera de rango): se rechaza y el slider conserva su valor anterior.
- El input formatea el valor actual del slider al abrirse (usa `format_fn` del slider si existe, o el formato por defecto).

### 4. Estilo EQ Desactivado

Solo aplica a la pestaña de ecualizador. No afecta sliders de efectos DSP.

Solo cambia el **handle**. El track no se modifica.

**Configuración desde `audio_center.rs`:** Cuando `equalizer_enabled == false`, los sliders del EQ se configuran con:

| Campo | EQ Activado | EQ Desactivado |
|-------|------------|----------------|
| `handle_color` | COLOR_ACCENT (default) | COLOR_CONTRAST |
| `border_color` | transparente (default) | COLOR_ACCENT |
| `border_width` | 0.0 (default) | 2.0 |
| `handle_focus_color` | COLOR_TEXT_PRIMARY (default) | COLOR_ACCENT |
| `handle_hover_color` | COLOR_TEXT_SECONDARY (default) | COLOR_CONTRAST |
| `border_hover_color` | COLOR_ACCENT (default) | COLOR_TEXT_SECONDARY |
| `border_hover_width` | 0.0 (default) | 2.0 |

### 5. Snap a Grid (siempre activo)

**Función nativa del widget, siempre activa. No requiere flag.**

- Al soltar el arrastre del slider (fin del drag), el valor se redondea al múltiplo de `step_size` más cercano.
- Usa `step_size` de cada slider automáticamente:
  - `step_size = 0.1` → snap al 0.1 más cercano
  - `step_size = 0.01` → snap al 0.01 más cercano
  - `step_size = 1.0` → snap al entero más cercano
- No hay snap durante el arrastre — solo al soltar, para mantener sensación suave.
- Esto garantiza que sliders con el mismo valor tengan exactamente la misma posición visual del handle.
- Compatible con teclado: las flechas ya usan `step_size`, los valores ya están en grid.

### the agent's Discretion

- Los nombres exactos de los nuevos mensajes `AudioCenterMessage` (SliderHoverActive vs nombres alternativos) quedan a discreción del implementador.
- La implementación del callback `on_hover_state_change` puede usar un solo callback `Box<dyn Fn(bool) -> Message>` o dos callbacks separados (`on_hover_activated` + `on_hover_deactivated`). A discreción del implementador, priorizando simplicidad.
- El snap a grid no se aplica durante keyboard input (el input puede setear cualquier valor dentro del rango). El snap ocurre al soltar el handle del slider, no al escribir un valor en el input.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Custom Slider Widget
- `src/gui/widgets.rs` — Widget `CustomSlider` completo (líneas 2230-2951), `CustomSliderOptions` (2252-2266), `CustomSliderState` (2289-2293)
- `src/gui/widgets.rs:2644-2703` — Manejo actual de eventos de teclado (BUG: hover→keyboard_focused automático)
- `src/gui/widgets.rs:2833` — Color del handle según hover (lógica a refactorizar)
- `src/gui/widgets.rs:2766-2861` — `draw_vertical_slider` (vertical)
- `src/gui/widgets.rs:2864-2943` — `draw_horizontal_slider` (horizontal)

### Audio Center
- `src/gui/audio_center.rs` — Centro de Audio Avanzado completo (1-2781)
- `src/gui/audio_center.rs:13-42` — `AudioCenterMessage` enum
- `src/gui/audio_center.rs:2001-2033` — `vertical_slider()` helper (EQ sliders)
- `src/gui/audio_center.rs:2158-2339` — `view_effect_with_secondary()` (efectos DSP)
- `src/gui/audio_center.rs:2208-2227` — Creación de slider primario en efectos
- `src/gui/audio_center.rs:2261-2282` — Creación de slider secundario en efectos

### App (Keyboard / Focus routing)
- `src/gui/app.rs:343-348` — `AppFocus` enum (añadir `AudioCenter` existente pero sin uso)
- `src/gui/app.rs:364` — `last_audio_center_key_event` (ELIMINAR)
- `src/gui/app.rs:2365-2495` — `GlobalKeyDown` handler (modificar para AppFocus::AudioCenter)
- `src/gui/app.rs:3687-3694` — Asignación de `last_audio_center_key_event` (ELIMINAR)
- `src/gui/app.rs:4141-4167` — Render del centro de audio como overlay

### Theme / Colors
- `src/gui/theme.rs` — Constantes de color (7 colores) y fuentes (3 familias)

### Existing Phase 01 Context
- `.planning/phases/01-improve-audio-effects/01-CONTEXT.md` — Contexto existente de mejoras DSP

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **CustomSlider widget** (`src/gui/widgets.rs:2230-2951`): Implementación completa de widget iced con orientación vertical/horizontal, track coloreado, tooltip, y eventos de teclado. Base sobre la cual construir.
- **CustomSliderOptions** (`src/gui/widgets.rs:2252-2266`): Struct de configuración con 13 campos públicos. Se extenderá con nuevos campos para focus/hover/input.
- **CustomSliderState** (`src/gui/widgets.rs:2289-2293`): Estado interno del widget (is_dragging, keyboard_focused, tooltip_pos). Se modificará para separar focus de hover.

### Established Patterns
- **Widget-level event capture**: El slider implementa `iced::advanced::Widget` y captura eventos en `update()`, retornando `Status::Captured/Ignored`. Este patrón se mantiene y refuerza.
- **Mensajes iced**: Audio center usa `AudioCenterMessage` enum que `app.rs` envuelve en `Message::AudioCenterMsg`. El nuevo callback `on_hover_state_change` sigue este patrón.
- **Builder pattern**: `CustomSlider` usa builders (`with_arrow_keys()`, `show_tooltip()`, etc.). Los nuevos campos seguirán este patrón.

### Integration Points
- `app.rs`: El handler de `AudioCenterMsg` (línea 3687) es el punto de integración para `SliderHoverActive`. También donde se elimina `last_audio_center_key_event`.
- `GlobalKeyDown` (línea 2365): Punto donde se añade la verificación de `AppFocus::AudioCenter`.
- `audio_center.rs` `view_equalizer()` (línea 1925): Donde se configuran los estilos de sliders EQ según `equalizer_enabled`.

### Fix Required (Critical Bug)
- `widgets.rs:2648-2650`: `if is_hovered && !state.keyboard_focused { state.keyboard_focused = true; }` — Esta línea hace que hover=mouse-over active teclado. **ELIMINAR** esta línea. El `keyboard_focused` solo debe activarse con clic.

</code_context>

<specifics>
## Specific Ideas

- Diseño de focus (mouse over) → handle en COLOR_TEXT_PRIMARY. Inspirado en sliders de DAWs (Ableton, FL Studio) donde el hover visual es sutil.
- Diseño de hover (clic activo) → handle en COLOR_TEXT_SECONDARY + borde COLOR_ACCENT. El borde inset da sensación de "agarre" sin cambiar tamaño del handle.
- Input siempre visible junto al slider → inspirado en ecualizadores profesionales (FabFilter Pro-Q, izotope Ozone) donde el valor numérico siempre está visible y editable.
- Snap a grid usando step_size → comportamiento estándar en DAWs donde todos los sliders al mismo valor se ven idénticos.
- EQ desactivado cambia handle a COLOR_CONTRAST + borde COLOR_ACCENT → el borde rojo indica que el slider está "vivo" aunque el EQ esté desactivado (se puede ajustar el valor para cuando se reactive).

</specifics>

<deferred>
## Deferred Ideas

- **Efecto visual de arrastre (drag active state)** — Se consideró pero el usuario prefiere mantener el slider simple sin efecto de arrastre.
- **Doble clic para resetear valor** — Se consideró, no se implementa por ahora. El clic derecho ya resetea.
- **Marcas de step en el track** — Reemplazado por snap a grid que es más preciso.
- **Animación suave en cambios programáticos** — No implementar por ahora.
- **Aplicar estilo desactivado a sliders de efectos DSP** — Solo EQ por ahora.

</deferred>

---

*Phase: 01 - Slider UI Context*
*Context gathered: 2026-06-02*
