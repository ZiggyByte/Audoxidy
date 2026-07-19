# Phase 03: Volumen y Mezcla - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-18
**Phase:** 03-volumen-y-mezcla
**Areas discussed:** Arquitectura/gain-staging + ringbuf, Fades y suavizado, Normalización, Replay Gain, Eliminar silencios, Widgets globales, Defaults, Bugs del motor

---

## Indicaciones iniciales del usuario (fuera de cuestionario)

- Los pre-amplificadores nuevos deben ser **independientes entre sí** y del preamp del EQ (que permanece acoplado al EQ); cada uno se activa con su función. Se pidió recomendación de posición en la cadena → respuesta: suma única en dB al frente, antes del DSP, con limiter como red de seguridad (D-01..D-03).
- Pregunta directa: ¿ringbuf en f64? → Análisis: posible pero no recomendable (f32 ≈ 24 bits ≥ cualquier DAC; el procesamiento ya es f64; CPAL raramente acepta f64). Usuario aceptó mantener f32 (D-04).

---

## Fades y suavizado

| Pregunta | Option | Description | Selected |
|----------|--------|-------------|----------|
| ¿Master del grupo? | Independientes | 3 funciones autónomas | |
| | Es el master del grupo | OFF apaga fade-in/out y los muestra grises | ✓ |
| Tiempo de suavizado | 80 ms | Recomendado (estándar anti-zipper) | |
| | 50 ms / 150 ms | Alternativas | |
| | **500 ms** | Valor libre del usuario | ✓ |
| Fade-out en stop/skip manual | Rampa corta fija ~200 ms | Recomendado | |
| | Siempre el tiempo configurado | | |
| | Corte inmediato | Elegido por el usuario | ✓ |
| Curva de fades | Equal-power | Estándar DAW | ✓ |
| | Lineal en dB / lineal en amplitud | | |
| Fade-in en skip manual | Todo inicio de pista | | |
| | **Fade-in solo natural** | Corrección del usuario: ambos fades solo en transiciones naturales | ✓ |

**Notes:** El usuario definió el patrón completo de transiciones: naturales = fades; manuales (play/skip/stop/pausa) = inmediato.

---

## Normalización

| Pregunta | Option | Description | Selected |
|----------|--------|-------------|----------|
| Enfoque | RMS tiempo real | Ventana ~400 ms, rampas lentas, sin retraso de inicio | ✓ |
| | Por picos | Simple pero no iguala volumen percibido | |
| | Pre-escaneo | Exacto pero retrasa inicio + pico CPU | |
| ¿Qué es el "Umbral superior"? | Cap de boost máximo | Nunca subir más de +X dB | ✓ |
| | Gate de activación / Boost fijo +X | | |
| Defaults target/cap | -14 dB / +6 dB | Referencia tipo streaming | ✓ |
| | -18 dB / +6 dB (RG clásico) / -14 / +12 | | |
| Limiter | Auto-on con normalización, restaura al apagar | Techo -1 dBTP | ✓ |
| | Independiente | | |
| Interacción con RG | Cooperan, RG primero | Normalización mide post-RG y afina | ✓ |
| | RG tiene prioridad / mutuamente excluyentes | | |
| Velocidad de ajuste (#16) | 4000/1000 ms (recomendado) | Subida lenta inaudible | |
| | 800/800 ms (usuario) | Primera elección del usuario | |
| | **3000/1000 ms** | Compromiso final del usuario | ✓ |

**Notes:** El usuario pidió opciones para el "Umbral superior de pre-amplificación" y eligió el cap de boost. Tras pedir la recomendación real para #16, fijó 3000/1000 ms.

---

## Replay Gain

| Pregunta | Option | Description | Selected |
|----------|--------|-------------|----------|
| ¿Dónde van los ±dB? | Offsets en etapa RG (dominio dB, aplicación única) | Referencia: "Preamp" de foobar2000 | ✓ |
| | Preamps separados post-RG | Más puntos de ganancia = más riesgo de clip | |
| Album + canción activos | Se suman | Comportamiento actual del engine | ✓ |
| | Canción prioriza / Album prioriza | | |
| Análisis en tiempo real | Fallback sin etiquetas | Etiqueta manda si existe | ✓ |
| | Siempre suma | Doble corrección (descartado) | |
| Target del análisis RT | Mismo que Normalización (-14 dB) | Un solo objetivo coherente | ✓ |
| | Referencia RG clásica -18 | | |

**Notes:** Tope +12 dB existente se mantiene como red de seguridad. Los flags RG ya existen en `AudioState` (default true) — la UI solo los expone.

---

## Eliminar silencios

| Pregunta | Option | Description | Selected |
|----------|--------|-------------|----------|
| Mecanismo de salto | Descarte en decoder | Sin seeks, posición salta sola | ✓ |
| | Basado en seeks / Pre-escaneo | | |
| Métrica de detección | Peak por frame | Conservador, barato | ✓ |
| | RMS ventana corta | | |
| Defaults duración/umbral | 1000 ms / -48 dB (propuesto) | | |
| | **1000 ms / -50 dB** | Ajuste libre del usuario | ✓ |
| Umbral de bordes | Fijo -50 dB interno, sin duración mínima | | ✓ |
| | Adaptativo (noise-floor) | | |

**Notes:** Se acordó que el borde final marca `effective_end` (EOF temprano + alimenta el trigger del fade-out). Riesgo comunicado: pasajes silenciosos intencionales pueden saltarse si superan la duración configurada.

---

## Widgets globales

| Pregunta | Option | Description | Selected |
|----------|--------|-------------|----------|
| Fondo checkbox OFF | COLOR_BG (negro) | Estado apagado claramente distinguible | ✓ |
| | COLOR_ACCENT (rojo, literal del spec) | Off/on casi idénticos | |
| Delete/Supr/Backspace | Edición estándar | Backspace izquierda, Delete derecha del cursor | ✓ |
| | Borran todo el valor | | |
| Geometría stepper | 72px todo dentro / 60px + unidad fuera / 60px iconos 10px | | |
| | **64px, iconos 14px** | Decisión libre del usuario; ajustará tipografía/geometría después si hace falta | ✓ |
| Rangos/defaults | Aprobar tabla propuesta | | |
| | **Listar todo para ajustar** | El usuario pidió la tabla completa (1-20) y luego ajustó: fade-in 1000 ms, fade-out 1000 ms, #16 800/800 → luego 3000/1000 | ✓ |

**Notes:** Mejoras aceptadas sobre el CustomSlider: filtro de caracteres, tecla Delete, sufijo excluido del parseo. Input manual nunca se redondea al paso (requisito explícito).

---

## Bugs del motor (decididos tras reporte)

| Bug | Option | Selected |
|-----|--------|----------|
| B1: dropout de efectos por `try_write()` (`decoder.rs:820-840`) | Arreglar ahora / Diferir | ✓ Arreglar ahora |
| B2: sufijo rompe parseo en CustomSlider | Arreglar slider también / Solo widget nuevo | ✓ Arreglar slider también |
| B3: volumen no persiste (reset a 0.3) | Persistir volumen / Diferir | ✓ Persistir |

---

## Claude's Discretion

- Nombres de mensajes/claves/funciones nuevas; formato de sufijo con recorte de ceros.
- Histéresis de silencio (+3 dB), rampa anti-click de offsets (~100 ms), reset de contadores en seek/pausa.
- Un solo controlador de sonoridad compartido (Normalización + Análisis RT no corren lazos duales).
- Detección de silencio medida PRE-fade (evita que el fade-out dispare el salto).
- Mecanismo concreto del fix B1 (bloqueo breve vs snapshot) — decidir en planificación.
- Master ON restaura estados individuales guardados de sub-funciones (no fuerza todo a on).

## Deferred Ideas

- Crossfade entre pistas (reutiliza el motor de envolventes).
- Pre-cálculo de ReplayGain en el scanner (background → DB).
- Memoria de ganancia por pista (aplicación instantánea al re-escuchar).
- LUFS K-weighted real en vez de RMS plano.
- Migración de ringbuf a f64 — evaluada y descartada (f32 ≥ resolución de cualquier DAC real).
