# Audoxidy — Roadmap

> Plan de desarrollo de Audoxidy, reproductor de audio con ecualizador gráfico y efectos DSP.

---

## Phase 01: Audio Effects Improvement

**Goal:** Mejorar la interfaz de efectos de audio y la gestión de estado DSP en el Centro de Audio.

**Status:** ✅ Complete

**Plans:** 3 plans (completed)

---

## Phase 02: EQ Preset Management

**Goal:** Sistema completo de gestión de presets de ecualizador con persistencia SQLite, diálogos modales de carga/guardado, importación/exportación JSON y 3 botones de icono SVG.

**Status:** 🔄 Planning

**Status:** ✅ Complete

**Plans:** 5/5 plans complete

**Plans:**

- [x] 02-01-PLAN.md — Backend: DB eq_presets table, DSP methods, state structs
- [x] 02-02-PLAN.md — UI: Icon buttons + Load dialog with preset list
- [x] 02-03-PLAN.md — Save dialog + Reset + Delete
- [x] 02-04-PLAN.md — Import/Export via JSON file I/O
- [x] 02-05-PLAN.md — Tests for DB, DSP, JSON round-trip

---

## Phase 03: Volumen y Mezcla

**Goal:** Nueva pestaña "Volumen y Mezcla" en el Centro de Audio Avanzado: suavizado de volumen y fades naturales, eliminación de silencios (medio y bordes), normalización RMS en tiempo real con limiter auto-on, Replay Gain con offsets por fuente y análisis en tiempo real de fallback, 2 widgets globales reutilizables (StandardCheckbox 12px + NumberStepper 64×14), persistencia inmediata en APP_SETTINGS (incluido volumen), y fixes del flujo de audio (dropout DSP por try_write, parseo con sufijo en CustomSlider).

**Status:** 🔄 Context gathered

**Canonical refs:** `.planning/phases/03-volumen-y-mezcla/03-CONTEXT.md`
