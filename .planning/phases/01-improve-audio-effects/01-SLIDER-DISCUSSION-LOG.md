# Phase 01: Mejora de Sliders - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-02
**Phase:** 01 - Slider Widget & Keyboard Navigation
**Areas discussed:** Separación foco/hover, Conflicto teclado, Input de valores, Estilo EQ desactivado, Recomendaciones adicionales

---

## 1. Separación Foco/Hover

| Option | Description | Selected |
|--------|-------------|----------|
| Solo con clic | Focus se activa al hacer clic en el slider | |
| Clic o arrastrar | Focus se activa al hacer clic o arrastrar | |
| Clic en otro slider | Focus se pierde al hacer clic en otro slider | |
| Clic en otro slider + Escape | Focus se pierde con clic en otro slider o Escape | |
| Sí, persiste | El focus visual persiste aunque el mouse salga | |
| Solo mientras hover + focus | Focus visual solo cuando mouse + focus activos | |
| handle_focus_color + border_hover | API con campos separados | ✓ |
| Reutilizar existentes | Mapear con lógica interna | |

**User's choice:** El focus NO debe activar teclado — es solo visual (mouse sobre slider). El hover se activa SOLO con clic y es el único que habilita teclado. El focus se pierde cuando el mouse sale del slider. API con nombres separados: `handle_focus_color`, `handle_hover_color`, `border_focus_color`, `border_hover_color`.

**Notes:** El usuario usa "focus" = mouse over (visual) y "hover" = clic activo (visual + teclado). Terminología no estándar pero clara. Borde del hover debe ser inset (interno).

---

## 2. Conflicto de Teclado Slider vs Biblioteca/Playlist

| Option | Description | Selected |
|--------|-------------|----------|
| Que el slider capture | Slider consume evento Status::Captured, no llega a GlobalKeyDown | |
| Usar AppFocus::AudioCenter | Control centralizado en app.rs con AppFocus | ✓ |
| Navegar library/playlist | Si ningún slider activo, flechas pasan a módulos de abajo | ✓ |
| No navegar nada | Centro de audio bloquea teclado aunque ningún slider activo | |

**User's choice:** Usar `AppFocus::AudioCenter` explícito con callback `on_hover_state_change` desde el slider. Cuando un slider tiene hover activo, asigna `AppFocus::AudioCenter` y GlobalKeyDown no pasa flechas. Cuando audio center abierto pero ningún slider activo, flechas navegan library/playlist normalmente. Reemplazar completamente el guard de 150ms.

**Notes:** El usuario quiere poder navegar library/playlist con teclado aunque el centro de audio esté abierto, siempre que ningún slider tenga hover activo. También poder hacer clic en espacio vacío del centro de audio para desactivar hover y luego navegar library/playlist normalmente.

---

## 3. Input de Valores por Teclado

| Option | Description | Selected |
|--------|-------------|----------|
| Doble clic en handle | Double-click abre input | |
| Clic derecho (menú) | Menú contextual con opción de ingresar valor | |
| Tecla Enter con hover | Enter abre input cuando slider tiene hover | |
| Combinación doble clic o Enter | Ambos métodos | |
| Popup flotante | Input aparece como recuadro flotante | |
| Reemplazar handle | Input reemplaza el handle temporalmente | |
| Arriba, abajo | Solo dos posiciones | |
| Arriba, abajo, izquierda, derecha | Cuatro posiciones | ✓ |

**User's choice:** `enable_keyboard_input = true` → input SIEMPRE visible junto al slider. No se activa por interacción — es permanente. Posición: arriba, abajo, izquierda, derecha. Default automático según orientación. Layout alineado lado a lado, gap 10px default configurable. Tamaño auto por defecto, configurable a fixed. Estilo default: bg #000000, borde #5B5B5B 1px radius 4px, borde focus #FF003D (solo al escribir, no hover), font Inter Medium 13px #AFAFAF. Enter o clic fuera para confirmar. Inválido → rechazar.

**Notes:** El borde focus del input solo se muestra cuando se hace clic en el input para escribir, no con solo pasar el mouse sobre él.

---

## 4. Estilo EQ Desactivado

| Option | Description | Selected |
|--------|-------------|----------|
| Configuración desde audio_center.rs | Sliders EQ configuran estilos según equalizer_enabled | ✓ |
| Flag 'disabled' en CustomSliderOptions | Flag genérico que aplica estilo automático | |
| No, solo el handle | El track no cambia | ✓ |
| Sí, track también | Track también se atenúa | |

**User's choice:** Solo EQ, no efectos DSP. Solo handle, no track. Configuración desde `audio_center.rs`. EQ desactivado: handle_color = COLOR_CONTRAST, border_color = COLOR_ACCENT 2px; focus = handle_focus_color COLOR_ACCENT; hover = handle_hover_color COLOR_CONTRAST + border_hover_color COLOR_TEXT_SECONDARY 2px.

---

## 5. Recomendaciones Adicionales

| Option | Description | Selected |
|--------|-------------|----------|
| Estado visual de arrastre | Efecto visual al arrastrar el handle | ✗ (rechazado) |
| Doble clic para resetear | Doble clic resetea a default | ✗ (rechazado) |
| Marcas de step en el track | Indicadores visuales de step | ✗ (rechazado → snap a grid) |
| Animación suave | Transición animada en cambios programáticos | ✗ (rechazado) |
| Snap a grid magnético | Handle se alinea automáticamente al step más cercano al soltar | ✓ (nueva recomendación) |

**User's choice:** No quiere efectos visuales de arrastre. No quiere marcas visuales en el track. En su lugar, quiere un "snap a grid" magnético: al soltar el arrastre, el valor se redondea automáticamente al múltiplo de `step_size` más cercano. Siempre activo, sin flag. Se adapta automáticamente al tipo de valor del slider (0.1, 0.01, 1.0).

---

## the agent's Discretion

- Nombres exactos de mensajes `AudioCenterMessage` queda a discreción del implementador.
- `on_hover_state_change` puede ser un solo callback `Box<dyn Fn(bool) -> Message>` o dos separados (`on_hover_activated` + `on_hover_deactivated`). Priorizar simplicidad.
- Snap a grid no aplica durante keyboard input (input puede setear cualquier valor). Snap solo al soltar drag.

## Deferred Ideas

- **Efecto visual de arrastre** — No implementar.
- **Doble clic para resetear** — No implementar (clic derecho ya resetea).
- **Marcas de step en el track** — Reemplazado por snap a grid.
- **Animación suave** — No implementar.
- **Estilo desactivado para efectos DSP** — Solo EQ por ahora.
