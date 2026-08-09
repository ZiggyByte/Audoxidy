# Phase 03: Volumen y Mezcla - Context

**Gathered:** 2026-07-18
**Updated:** 2026-07-18 (post-UAT rounds 1-4)
**Status:** Executed with UAT fixes — fades/smoothing corrected, Gain fijo → Replay gain fijo, persistence in app.rs

<domain>
## Phase Boundary

Nueva pestaña **"Volumen y Mezcla"** (índice 3) en el Centro de Audio Avanzado, con grupos de funciones de volumen aplicadas en tiempo real al pipeline f64: (1) suavizado de cambio de volumen + fade-in/fade-out, (2) eliminación de silencios (duración/umbral configurables + bordes), (3) Replay gain fijo (ganancia fija aplicada siempre, sin condición de etiquetas), (4) Replay Gain con offsets ±dB por fuente y análisis en tiempo real como fallback. Incluye 2 widgets globales reutilizables en `widgets.rs` (`StandardCheckbox` 12×12, `NumberStepper` 84×14 con unidades dB/ms), layout de 2 columnas iguales con separador vertical, persistencia inmediata de todo (sin botón aplicar) incluido el volumen del player, y 2 fixes del flujo de audio: dropout de efectos por `try_write()` y parseo con sufijo en CustomSlider. El grupo "Normalizar Volumen" fue **eliminado por completo** en los ajustes UAT; el "Gain fijo" fue reemplazado por "Replay gain fijo" (se aplica siempre, no solo sin etiquetas).

**No es:** crossfade entre pistas, pre-cálculo de RG en el scanner, LUFS K-weighted, memoria de ganancia por pista (todas diferidas), ni cambios en los efectos DSP existentes o en el ecualizador.

</domain>

<decisions>
## Implementation Decisions

### Arquitectura / Gain-staging (cadena de audio)
- **D-01:** Punto **único** de ganancia de sonoridad al frente de la cadena, suma en dominio dB: `(album_tag + offset_album) + (track_tag + offset_track) + (análisis_RT + offset_RT, solo si no hay etiquetas) + (replay_gain_fijo, siempre si está activo)`. Se convierte a lineal **una sola vez** (`10^(db/20)`) y se aplica en un solo punto de multiplicación por frame, ANTES de la DspChain (posición actual del RG, `decoder.rs:800-813`).
- **D-02:** [informational] Cada "pre-amplificador" nuevo es **independiente** de los demás y del preamp del EQ. El preamp del EQ (`dsp.rs:54-57`) **se queda acoplado al EQ como está** — NO desacoplarlo.
- **D-03:** Orden final de la cadena: ganancia de sonoridad (D-01) → DspChain existente (preamp EQ → EQ → efectos → **Limiter al final**) → envolvente de fades × volumen de usuario (juntos, al final, `decoder.rs:826-828`) → ringbuf. Los fades van al final para que ningún efecto los "deshaga"; todas las ganancias que suben nivel van antes del limiter (red anti-clipping).
- **D-04:** [informational] Ringbuf se mantiene **f32** (NO migrar a f64). Todo el procesamiento ya es f64; f32 equivale a ~24 bits de precisión (≥ cualquier DAC) y CPAL raramente acepta f64 nativo. La conversión f64→f32 en el boundary (`decoder.rs:844-846`) es transparente.
- **D-05:** ~~Un solo controlador de sonoridad compartido (Normalización + Análisis RT)~~ — **ELIMINADO** con la normalización. El análisis RT es el único lazo de medición que queda, solo como fallback sin etiquetas.
- **D-06:** Todo el procesamiento nuevo se implementa en **f64** (coherente con el pipeline existente).

### Fades y suavizado de volumen
- **D-07:** "Suavizar el cambio de volumen" es el **master del grupo**: OFF → fade-in y fade-out se desactivan, se muestran grises y no aplican.
- **D-08:** Suavizado del cambio de volumen de usuario: **500 ms fijos**, rampa **lineal**, sin widget. El cambio se hace **desde el nivel actual del volumen hacia el nuevo** (NUNCA desde 0). El smoothing usa `previous_vol` como coeff inicial. Corregido en UAT round 4: el bug de "baja a 0 y sube" era causado por el fade-out disparándose en cada batch (usaba `effective_end_sec` que rastrea la posición en vivo en vez de `total_duration_sec`).
- **D-09:** Fade-in: en **cada inicio de pista** cuando está activo (corregido en UAT round 4 — antes solo en avance natural). Toggle propio `fade_in_enabled`. Default **1000 ms**, rango 0–10000, paso 50 ms.
- **D-10:** Fade-out: al **final de la pista**. Toggle propio `fade_out_enabled`. Default **1000 ms**, rango 0–10000, paso 50 ms. **Trigger corregido**: usa `total_duration_sec − fade_ms` (NO `effective_end_sec`, que causaba oscilación). Fade-out se dispara solo cuando `current_pos_sec >= total_duration_sec − fade_out_ms`.
- **D-11:** Stop / skip / pausa manual = **corte inmediato**, sin rampa.
- **D-12:** Fades musicales (fade-in/fade-out) usan curva **equal-power**. El suavizado de 500 ms usa rampa lineal simple.
- **D-13:** Implementación: máquina de estados de envolvente en el decoder (f64, coeficiente 0.0→1.0 interpolado por muestra, multiplicado junto al volumen). Cero allocs en el hot path. `combined = vol * fade_coeff` para fades, pero para **Smoothing** `combined = fade_coeff` (el coeff YA es el nivel de volumen — corregido en UAT round 3).

### Eliminación de silencios
- **D-14:** Mecanismo = **descarte de frames en el decoder** (NO seeks, NO pre-escaneo). El decoder corre ~100 ms por delante: cuando el silencio acumulado supera la "Duración", deja de escribir frames al ringbuf hasta que vuelve señal ≥ umbral. `current_pos_sec` sigue avanzando por timestamps de paquetes → la UI "salta" sola.
- **D-15:** Detección = **peak por frame** (max |sample| de todos los canales) vs umbral. Métrica conservadora: cualquier golpecito cuenta como audio.
- **D-16:** Defaults de usuario: duración **1000 ms** (rango 100–10000, paso 50) / umbral **-50 dB** (rango -80..0, paso 0.25).
- **D-17:** Histéresis interna de **+3 dB** (entra a silencio a -50, sale a -47) para evitar parpadeo en la frontera.
- **D-18:** "Eliminar silencio en los bordes" (checkbox propio `silence_edge_trim_enabled`, UAT round 2): umbral interno fijo **-50 dB**, **sin duración mínima**. Borde inicial: descartar frames hasta el primer contenido audible. Borde final: se elimina el rastro de `effective_end` (el fade-out usa `total_duration_sec`); el recorte de cola se descarta si se detecta silencio hasta EOF.
- **D-19:** La detección de silencio se mide **PRE-fade** (en el punto de ganancia de sonoridad). Crítico: si midiera post-fade, el propio fade-out se detectaría como silencio y dispararía el salto.
- **D-20:** Seek, pausa y cambio de pista resetean contadores de silencio y estado de fades.

### Normalizar Volumen (ELIMINADO en UAT)
- **D-21 a D-25 (ELIMINADOS):** El grupo "Normalizar Volumen" (RMS en tiempo real, target -14 dB, cap +6 dB, limiter auto-on) fue **eliminado por completo** durante los ajustes UAT. No queda ningún rastro en `audio_center.rs`, `decoder.rs`, `engine.rs` ni `app.rs`. El limiter auto-on también fue retirado (solo existía para normalización).

### Replay gain fijo (reemplaza al "Gain fijo")
- **D-21b:** "Replay gain fijo" — ganancia fija en dB que se aplica **SIEMPRE cuando está activa**, sin la condición de ausencia de etiquetas que tiene el análisis RT. Reemplaza al antiguo "Gain fijo". Implementado como `rg_fixed_enabled: bool` y `rg_fixed_db: f32` en `AudioState` (`engine.rs`). En el decoder se suma en el punto único de ganancia (D-01) junto a los demás RG, siempre que `rg_fixed_enabled`. Default 0 dB, rango -30..12, paso 0.25. Se mantiene el tope +12 dB. Persistencia: claves `vol_rg_fixed_enabled` / `vol_rg_fixed_db` en `APP_SETTINGS`, cargadas en `app.rs` al inicio.

### Replay Gain
- **D-26:** Los ±dB de cada fuente son **offsets en dominio dB dentro de la etapa RG** (NO preamps/amplificadores separados): se suman al valor de su etiqueta antes de la única conversión a lineal. Heredan el tope de seguridad **+12 dB** existente (`decoder.rs:800-813`, se mantiene).
- **D-27:** Album + canción **se suman** cuando ambos checkboxes están activos (comportamiento actual del engine, se mantiene).
- **D-28:** "Analizar archivo en tiempo real" = **fallback solo cuando las fuentes de etiquetas activadas no tienen datos** para la pista. Beneficio extra: cubre archivos fuera de la biblioteca (el decoder no lee metadatos, `decoder.rs:339-342`).
- **D-29:** Offsets default **0 dB**, rango **±12 dB**, paso 0.25. Cambios de offset desde UI se aplican con rampa corta ~100 ms anti-click.
- **D-30:** Los flags `replay_gain_track_enabled` / `replay_gain_album_enabled` ya existen en `AudioState` (default true, `engine.rs:97-138`) sin API pública — la UI los escribe via `audio_manager.state().write()` (patrón de `AudioStateToggle`, `audio_center.rs:1160-1177`). El master "Replay Gain" gobierna el grupo (OFF → sub-funciones grises y ganancia RG = 0 dB).

### Widgets globales (`src/gui/widgets.rs`)
- **D-31:** `StandardCheckbox` — 12×12 px. OFF: fondo `COLOR_BG`, borde 1 px `COLOR_TEXT_SECONDARY`. ON: fondo `COLOR_ACCENT`, borde 1 px `COLOR_ACCENT`, check "✓" en `COLOR_TEXT_PRIMARY`. Widget custom-dibujado (no existe iced::checkbox en la GUI actual). API: `StandardCheckbox::new(checked, on_toggle)`.
- **D-32:** `NumberStepper` — rectángulo **84×14 px** fondo `COLOR_BG`; chevrons SVG 14 px (`assets/icons/arrow-left-chevron.svg`, `arrow-right-chevron.svg`) en los laterales, fondo **`COLOR_CONTRAST`** (normal) y **`COLOR_ACCENT`** (hover/pressed), color de icono `COLOR_TEXT_PRIMARY`; valor centrado 12 px `COLOR_TEXT_PRIMARY` con sufijo de unidad visible ("dB"/"ms") pero **sin sufijo al editar por teclado** (solo números, más fácil borrar).
- **D-33:** API del stepper: `NumberStepper::new(value, range, StepperUnit::Decibels, on_change)` / `StepperUnit::Milliseconds`. Pasos de flecha: **0.25** (dB) / **50** (ms). Flechas ←/→ del widget y ↑/↓ del teclado hacen step ±.
- **D-34:** Input manual: **nunca se redondea al paso** (1.78 dB se queda 1.78; 342 ms se queda 342). Filtro de caracteres (solo dígitos, `.`, `-`), Backspace borra a la izquierda del cursor y Delete/Supr a la derecha (edición estándar), ←/→ mueven el cursor, **Enter aplica**, **clic fuera aplica** (copiar mecanismo del slider, `widgets.rs:2999-3010`), **Escape cancela**. El sufijo se muestra pero se **excluye antes de parsear**.
- **D-35:** Ambos widgets siguen el patrón CustomSlider: widget custom `iced::advanced::Widget`, input custom-dibujado con cursor `|` (no iced text_input), y callback `on_selected_state_change` → `AudioCenterMessage::SliderHoverActive` → `AppFocus::AudioCenter` para que las flechas no se escapen a biblioteca/playlist (`app.rs:3879-3891`, `app.rs:2526-2646`).

### Layout y comportamiento de la pestaña
- **D-36:** Tab índice 3 "Volumen y Mezcla": añadir nombre al array `tab_names` (`audio_center.rs:1310`) + brazo `3 => view_volumen_mezcla(...)` (`audio_center.rs:1358-1363`) + variantes de `AudioCenterMessage` + handlers en `update()` (`audio_center.rs:604`).
- **D-37:** Dos columnas de **igual ancho**: copiar el layout de `view_audio_config` (`audio_center.rs:2293-2325`) cambiando `FillPortion(5)/FillPortion(4)` → `FillPortion(1)/FillPortion(1)`, con el mismo separador vertical de 2 px `COLOR_CONTRAST` (`audio_center.rs:2284-2289`). SIN fila de botones inferior (no hay aplicar).
- **D-38:** Separador horizontal **1 px** `COLOR_TEXT_SECONDARY`, en la **MISMA fila** que el checkbox + texto principal del grupo (adaptativo, `Length::Fill` a la derecha del texto). Corregido en UAT (antes estaba debajo de la fila, a 2px).
- **D-39:** Checkboxes alineados a la **izquierda** del nombre de la función; steppers alineados a la **derecha** del nombre. Todos en la **misma fila**.
- **D-40:** Grupo OFF → sub-funciones grises (patrón `scale_alpha(0.5)` + sin `on_press`, `audio_center.rs:2240-2282`) y su aporte al engine = 0. Master ON → sub-funciones restauran su estado individual guardado y sus valores aplican inmediatamente.
- **D-41:** Distribución: **columna 1** = todos los grupos (Fades, Eliminar silencio, Replay gain fijo, Replay Gain); **columna 2** = **vacía por ahora** (para funciones de mezcla futuras). Spacing vertical entre grupos **13 px**.

### Persistencia y fixes
- **D-42:** Guardado inmediato (sin botón): añadir las nuevas variantes de mensaje a los match de `app.rs:3934-3960` → nueva función `save_volumen_settings_to_db()` en `AudioCenterManager` (espejo de `save_dsp_settings_to_db`, `audio_center.rs:278-315`) → tabla `APP_SETTINGS` (`database.rs:2033-2053`). **Carga en `App::new` (`app.rs:600-630`) para que las settings se apliquen desde el arranque** (UAT round 3 — antes se cargaba en `sync_from_engine` que solo corre al abrir el panel).
- **D-43:** **Persistir el volumen del player** en APP_SETTINGS. Guardar en el handler `PlayerScroll` (`app.rs:4134-4150`) con clave `player_volume` y cargar en `App::new` al inicio. Nota UAT: el mensaje `VolumeChanged` nunca se emite (no hay slider que lo produzca) — la persistencia real va en `PlayerScroll`.
- **D-44:** **Fix B1 (dropout de efectos):** `decoder.rs:820-840` usa `try_write()` y si la GUI tiene el lock del DSP, esos frames pasan SIN procesar (efectos se apagan a rachas al arrastrar sliders). Fix: nunca saltarse frames de DSP — esperar el lock brevemente o aplicar desde snapshot consistente.
- **D-45:** **Fix B2 (sufijo rompe parseo):** corregir también el `CustomSlider` (quitar sufijo antes de parsear en Enter/clic-fuera, `widgets.rs:3135-3187`, `widgets.rs:2961-3010`) — habilita input de teclado en sliders con unidad.

### Tabla de defaults aprobada (usuario)
| # | Función | Rango widget | Default |
|---|---------|--------------|---------|
| 1 | Fade-in al iniciar canción | 0–10000 ms (paso 50) | **1000 ms** |
| 2 | Fade-out al terminar canción | 0–10000 ms (paso 50) | **1000 ms** |
| 3 | Duración silencio para saltar | 100–10000 ms (paso 50) | **1000 ms** |
| 4 | Umbral detección de silencio | -80..0 dB (paso 0.25) | **-50 dB** |
| 5 | Replay gain fijo | -30..12 dB (paso 0.25) | **0 dB** |
| 6 | Offset RG Album | ±12 dB (paso 0.25) | **0 dB** |
| 7 | Offset RG Canción | ±12 dB (paso 0.25) | **0 dB** |
| 8 | Offset RG Análisis RT | ±12 dB (paso 0.25) | **0 dB** |
| 9 | Suavizado cambio de volumen (interno) | — | **500 ms, lineal** |
| 10 | Curva fades musicales (interno) | — | **equal-power** |
| 11 | Detección de silencio (interno) | — | **peak por frame** |
| 12 | Umbral bordes (interno) | — | **-50 dB, sin duración mínima** |
| 13 | Tope etapa RG (existente) | — | **+12 dB** |
| 14 | Trigger fade-out (interno) | — | `total_duration − fade_ms` (corregido) |
| 15 | Ringbuf (se mantiene) | — | **f32** |
| 16 | Corte stop/skip/pausa manual (interno) | — | **inmediato** |

*El grupo "Normalizar Volumen" (filas 5-6 y 15-17 originales) fue eliminado por completo en UAT.*

### Claude's Discretion
- Nombres exactos de mensajes `AudioCenterMessage`, claves de APP_SETTINGS y funciones de guardado.
- Formato de sufijo recortando ceros ("-50 dB" vs "-50.25 dB").
- Detalles internos: histéresis exacta (+3 dB), rampa anti-click de offsets (~100 ms), reset de contadores en seek/pausa, RMS sumado de canales.
- Mecanismo concreto del fix B1 (bloqueo breve vs snapshot) — decidir en planificación con criterio de cero glitches de audio.
- Tipografía/geometría fina del stepper: el usuario la ajustará manualmente después si hace falta (partió de 64×14 px e iconos 14 px).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Motor de audio (cadena y puntos de integración)
- `src/audio/decoder.rs:797-846` — Punto de aplicación RG → DSP → volumen → clamp f32 → ringbuf (aquí van la ganancia de sonoridad única, los fades y el descarte de silencios)
- `src/audio/decoder.rs:820-840` — **Fix B1**: `try_write()` salta frames de DSP si la GUI tiene el lock
- `src/audio/decoder.rs:290-418` — Carga de pista (`AudioCommand::Load`), `total_duration_sec`, purge de estado
- `src/audio/decoder.rs:419-452` — Seek (Accurate) y purgas asociadas
- `src/audio/decoder.rs:541-578` — EOF natural (`is_playing=false`, `eof_reached`)
- `src/audio/decoder.rs:462-493` — Flow control: 100 ms latencia target, 40 ms min free (por qué el decoder corre por delante)
- `src/audio/decoder.rs:588` — `current_pos_sec` desde timestamps de paquetes
- `src/audio/decoder.rs:339-342` — Metadata loading deshabilitado en decoder (RG solo viene de DB)
- `src/audio/engine.rs:97-138` — `AudioState`: `replay_gain_track/album: Option<f32>`, flags enabled (default true), `volume` (default 0.3, `engine.rs:114`)
- `src/audio/engine.rs:803-805` — `set_volume` (clamp 0..1)
- `src/audio/dsp.rs:48-106` — `DspChain::process_frame` (orden de etapas, f64)
- `src/audio/dsp.rs:54-57` — Preamp EQ acoplado a EQ enabled (NO tocar)
- `src/audio/dsp.rs:1911-2023` — Limiter true-peak (techo -1 dB, auto-on con normalización)
- `src/audio/manager.rs:78-137,282-297` — API pública: `seek`, `load_file`, `set_volume`, `with_dsp`/`with_dsp_mut`
- `src/db/scanner.rs:266-268` — Lectura de etiquetas RG (lofty) → DB
- `src/db/database.rs:664-678` — `get_replay_gain_by_path`
- `src/db/database.rs:2033-2053` — Tabla APP_SETTINGS (`get_setting`/`set_setting`)

### GUI (pestaña, widgets, persistencia)
- `src/gui/audio_center.rs:1310` — Array `tab_names` (añadir "Volumen y Mezcla")
- `src/gui/audio_center.rs:1357-1363` — Dispatch de contenido por tab (añadir brazo 3)
- `src/gui/audio_center.rs:2293-2325` — Layout 2 columnas de `view_audio_config` (copiar; FillPortion(5/4) → (1/1))
- `src/gui/audio_center.rs:2284-2289` — Separador vertical 2 px `COLOR_CONTRAST`
- `src/gui/audio_center.rs:2889-2898` — Separador horizontal de grupo (2 px `COLOR_TEXT_SECONDARY`)
- `src/gui/audio_center.rs:2240-2282` — Patrón disabled (`scale_alpha(0.5)`, sin `on_press`)
- `src/gui/audio_center.rs:13-55` — `AudioCenterMessage` enum (añadir variantes)
- `src/gui/audio_center.rs:57-77` — `DspEffect` / `AudioStateToggle` enums (patrón para toggles de AudioState)
- `src/gui/audio_center.rs:604` — `AudioCenterManager::update(msg, &AudioManager, &Mutex<Database>)`
- `src/gui/audio_center.rs:278-315` — `save_dsp_settings_to_db` (espejo para `save_volumen_settings_to_db`)
- `src/gui/audio_center.rs:317+` — `sync_from_engine` (carga de settings)
- `src/gui/audio_center.rs:1160-1177` — Patrón escritura directa a `AudioState` (para flags RG)
- `src/gui/app.rs:3879-3963` — Routing `AudioCenterMsg` + match de guardado inmediato (3934-3960, añadir variantes)
- `src/gui/app.rs:594-727` — Carga de settings al arranque (añadir volumen + nuevas claves)
- `src/gui/app.rs:3879-3891` — `SliderHoverActive` → `AppFocus::AudioCenter`
- `src/gui/app.rs:2526-2646` — `GlobalKeyDown` con gating de flechas por AppFocus
- `src/gui/app.rs:354-359` — `AppFocus` enum
- `src/gui/app.rs:1266-1269` — `Message::VolumeChanged` (añadir guardado de volumen)

### CustomSlider (base a copiar para NumberStepper + Fix B2)
- `src/gui/widgets.rs:2307-3679` — Widget completo (struct, options, state, impl Widget)
- `src/gui/widgets.rs:2942-2960` — Activación de input por clic (seed del buffer, cursor desde x)
- `src/gui/widgets.rs:2999-3010` — Clic-fuera aplica valor (copiar tal cual)
- `src/gui/widgets.rs:3135-3187` — Match de teclas del input (copiar + añadir Delete + filtro de chars + quitar sufijo antes de parsear)
- `src/gui/widgets.rs:3545-3671` — `draw_input` custom (cursor `|`) — patrón de dibujo del stepper
- `src/gui/widgets.rs:25` — `GLOBAL_SLIDER_SELECTED` (exclusión de un editor activo)

### Tema e iconos
- `src/gui/theme.rs:20-24` — `COLOR_ACCENT` 0xFF003D, `COLOR_BG` 0x000000, `COLOR_CONTRAST` 0x111111, `COLOR_TEXT_PRIMARY` 0xAFAFAF, `COLOR_TEXT_SECONDARY` 0x5B5B5B
- `src/gui/theme.rs:13-18` — `FONT_INTER_SANS_MEDIUM`
- `assets/icons/arrow-left-chevron.svg`, `assets/icons/arrow-right-chevron.svg` — chevrons del stepper
- `src/gui/audio_center.rs:1232-1247` — Patrón SVG por path con tinte hover
- `AGENTS.md` — Convenciones (threading, locks, build, `cargo check` tras cada cambio)

### Tests
- `src/audio/dsp_tests.rs` — Patrones de tests DSP (f64, epsilon, bypass, NaN guards)
- `src/audio/tests.rs:42-56` — Tests de defaults de `AudioState` (actualizar si cambian defaults)
- `src/audio/integration_tests.rs` — `MockDecoder` para tests del engine

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Pipeline f64 completo** (`decoder.rs` + `dsp.rs`): las nuevas funciones se enchufan en f64 sin conversión; solo el ringbuf es f32 (se mantiene, D-04).
- **Replay Gain funcional** (`engine.rs:97-138`, `decoder.rs:800-813`): etiquetas album/track desde DB, suma en dB, tope +12 dB. La UI solo expone flags y añade offsets.
- **Limiter true-peak** (`dsp.rs:1911`): red anti-clipping para normalización (auto-on) — ya existe, solo forzar/restaurar su flag.
- **Patrón de guardado inmediato** (`app.rs:3934-3960` + `save_*_to_db`): match sobre variantes de mensaje → APP_SETTINGS. La nueva pestaña se cuelga de ahí.
- **Patrón de input del CustomSlider** (`widgets.rs:2942-3010`, `3135-3187`, `3545-3671`): activación por clic, clic-fuera aplica, cursor fake `|`, draw custom. Se copia al stepper con 3 mejoras (Delete, filtro de chars, sufijo excluido del parseo).
- **Layout de `view_audio_config`** (`audio_center.rs:2293-2325`): columnas + separador vertical, listo para clonar con FillPortion(1).
- **`AudioStateToggle`** (`audio_center.rs:72-77,1160-1177`): patrón de escritura directa a `AudioState` para los flags RG.
- **Seguimiento de posición/duración** (`decoder.rs:352-358,588`): base para `effective_end` y trigger de fade-out.

### Established Patterns
- `parking_lot::RwLock` para `DspChain` y `AudioState`; `std::sync::Mutex` para `Database`. La GUI escribe vía `audio_manager.state().write()` o `with_dsp_mut()`.
- Decoder thread corre ~100 ms por delante (100 ms target latency, 40 ms min free) — habilita descarte de silencios sin seeks.
- Mensajes iced: `AudioCenterMessage` → `Message::AudioCenterMsg` → `app.rs` routing + persistencia.
- Widgets custom implementan `iced::advanced::Widget` con captura de eventos (`Status::Captured`) y callbacks `on_selected_state_change` para AppFocus.
- Colores/fuentes siempre desde `theme.rs` (nada hardcodeado).

### Integration Points
- **Ganancia de sonoridad única** → `decoder.rs:800-813` (reemplaza/amplía el cálculo actual de `gain_db`).
- **Fades × volumen** → `decoder.rs:826-828` (coeficiente de envolvente multiplicado junto a `vol`).
- **Descarte de silencios** → decode loop antes del push al ringbuf (`decoder.rs:848-887`); medición pre-fade en el punto de ganancia.
- **`effective_end`** → campo nuevo en estado del decoder; lo consumen fade-out (trigger) y EOF temprano.
- **Tab + view + mensajes** → `audio_center.rs:1310/1358/13-55/604`; routing y persistencia en `app.rs:3879-3963`.
- **Carga de settings** → `app.rs:594-727` + `sync_from_engine` (`audio_center.rs:317+`).

### Bugs a corregir en esta fase
- **B1** (`decoder.rs:820-840`): `try_write()` → frames sin DSP bajo contención (D-44).
- **B2** (`widgets.rs:3135-3187`): sufijo rompe parseo en CustomSlider (D-45).
- **B3** (`engine.rs:114`): volumen no persiste (D-43).

</code_context>

<specifics>
## Specific Ideas

- El usuario ajustó personalmente: suavizado 500 ms, fades 1000/1000 ms, umbral de silencio -50 dB, stepper de 84 px de ancho.
- El usuario ajustará manualmente tipografía/geometría fina del stepper si hace falta — dejar constantes de diseño fáciles de localizar.
- Requisito explícito del usuario: el input manual del stepper NUNCA se redondea al paso de las flechas.
- Referencia de diseño de los offsets RG: "Preamp +X dB" de foobar2000 (offset sobre el valor RG, no etapa separada).
- Los cambios de esta pestaña aplican y persisten INMEDIATAMENTE (sin botón), igual que los efectos DSP existentes.
- UAT round 4: el usuario pidió eliminar "Gain fijo" y reemplazarlo por "Replay gain fijo" (igual que "gain para canciones sin etiqueta" pero sin condición de tags). El grupo "Normalizar Volumen" fue eliminado por completo (distorsión reportada). Los fades se corrigieron: el bug de oscilación venía de usar `effective_end_sec` (rastrea posición en vivo) en el trigger del fade-out.
- El usuario pidió explícitamente que la persistencia se cargue en `app.rs` (no en audio_center.rs) para que los ajustes se apliquen desde el arranque del reproductor.

</specifics>

<deferred>
## Deferred Ideas

- **Crossfade entre pistas** — reutilizará el motor de envolventes de esta fase; fase futura.
- **Pre-cálculo de ReplayGain en el scanner** para pistas sin etiquetas (background, escribe a DB) — el análisis RT de esta fase es el fallback en vivo.
- **LUFS K-weighted real** en vez de RMS plano (más preciso perceptualmente) — la normalización RMS fue eliminada, este ítem queda diferido.
- **Migrar ringbuf a f64** — evaluado y descartado: f32 ≥ resolución de cualquier DAC real; el procesamiento ya es f64 (D-04).

</deferred>

---

*Phase: 03-Volumen y Mezcla*
*Context gathered: 2026-07-18*
