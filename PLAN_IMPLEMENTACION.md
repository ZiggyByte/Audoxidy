# Plan de Implementación: Optimización, Pruebas y Mejoras de Audoxidy

## Fase 1: Corrección de Errores y Estabilidad

### 1.1 Arreglar el sistema de pruebas unitarias
- **Problema**: `cargo test` falla con 14 errores porque el módulo de pruebas no puede acceder a funciones privadas del motor de audio
- **Solución**:
  - Mover las funciones necesarias para pruebas a `pub(crate)` o usar `#[cfg(test)]` para exponerlas solo en modo prueba
  - Reestructurar `src/audio/tests.rs` para probar a través de la API pública de `AudioManager`
  - Agregar pruebas unitarias para `DspChain`, `ChannelMap`, y utilidades de `utils/`

### 1.2 Corregir warnings de compilación
- **Problema**: 28 warnings en `cargo check`, incluyendo variables no usadas y asignaciones sin lectura
- **Solución**:
  - Renombrar variables no usadas con prefijo `_` (ej: `_has_expanded_here`)
  - Eliminar código muerto o comentado sin uso
  - Ejecutar `cargo clippy -- -D warnings` para asegurar limpieza total

### 1.3 Mejorar manejo de errores en el motor de audio
- **Problema**: Uso excesivo de `String` para errores en lugar de tipos estructurados
- **Solución**:
  - Crear un enum `AudioError` con variantes específicas (`DeviceNotFound`, `StreamError`, `DecodeError`, etc.)
  - Implementar `std::error::Error` y `Display` para mejor debugging
  - Reemplazar `Result<T, String>` por `Result<T, AudioError>` en `engine.rs` y `manager.rs`

---

## Fase 2: Optimización de Rendimiento

### 2.1 Optimizar el pipeline de audio
- **Objetivo**: Reducir latencia y mejorar throughput del motor de audio
- **Pasos**:
  1. Perfilar el ciclo de decodificación con `cargo flamegraph`
  2. Evaluar si el ringbuffer de 8 MiB es adecuado o si necesita ajuste dinámico según sample_rate/canales
  3. Optimizar la ruta de datos en `write_data()` para evitar copias innecesarias
  4. Considerar usar `memmap` para archivos de audio grandes en lugar de lectura completa a memoria

### 2.2 Optimizar la cadena DSP
- **Objetivo**: Reducir overhead de procesamiento DSP en tiempo real
- **Pasos**:
  1. Perfilar cada módulo DSP (EQ, Reverb, Compressor, etc.) individualmente
  2. Implementar bypass eficiente para módulos deshabilitados (evitar llamadas a `process()` si `enabled == false`)
  3. Considerar usar SIMD explícito para operaciones de frame-by-frame en EQ y mezcla de canales
  4. Evaluar si `rubato` puede configurarse con parámetros más eficientes para resampleo

### 2.3 Optimizar la gestión de memoria
- **Objetivo**: Reducir asignaciones y mejorar el GC automático
- **Pasos**:
  1. Revisar `memory_manager.rs` y optimizar los intervalos de purga basados en uso real de RAM
  2. Evaluar si `interner.rs` necesita límite de tamaño para evitar crecimiento ilimitado
  3. Optimizar el cache de carátulas (`covers.rs`) con políticas de eviction más agresivas en modo low-resource
  4. Considerar usar `bumpalo` o `typed-arena` para asignaciones temporales en el pipeline de audio

### 2.4 Optimizar la interfaz gráfica (iced)
- **Objetivo**: Mejorar fluidez de la UI y reducir redraws innecesarios
- **Pasos**:
  1. Identificar widgets que causan redraws frecuentes (scrollables, listas grandes)
  2. Implementar virtualización para listas de biblioteca con miles de canciones
  3. Optimizar el rendering de carátulas con `fast_image_resize` en background thread
  4. Evaluar si `iced_aw` puede reemplazarse con componentes custom más ligeros

---

## Fase 3: Pruebas Unitarias y de Integración

### 3.1 Pruebas del motor de audio
- **Cobertura objetivo**: 80%+ de `engine.rs`, `manager.rs`, `dsp.rs`
- **Pruebas a implementar**:
  - `test_audio_state_transitions` (play → pause → stop → play)
  - `test_channel_map_stereo_to_51` y otras configuraciones multicanal
  - `test_replay_gain_application` con valores positivos/negativos
  - `test_seek_boundaries` (seek al inicio, final, fuera de rango)
  - `test_ringbuffer_overflow_underflow` con datos simulados

### 3.2 Pruebas de la cadena DSP
- **Pruebas a implementar**:
  - `test_eq_frequency_response` con señales de prueba conocidas
  - `test_compressor_threshold_ratio` con señales de entrada variables
  - `test_reverb_dry_wet_mix` con valores 0.0, 0.5, 1.0
  - `test_stereo_expander_bounds` con señales mono y estéreo
  - `test_limiter_prevents_clipping` con señales de alta amplitud

### 3.3 Pruebas de utilidades
- **Pruebas a implementar**:
  - `test_format_duration` con valores límite (0, 1 hora, 24 horas)
  - `test_format_size` con bytes negativos, cero, grandes valores
  - `test_intern_string_deduplication` y `test_clear_interner`
  - `test_truncate_text` con límites de caracteres Unicode
  - `test_compare_track_numbers` con casos edge ("1/10", "A1", vacíos)

### 3.4 Pruebas de integración
- **Pruebas a implementar**:
  - `test_load_and_play_flac` con archivo de prueba real
  - `test_database_scan_and_query` con directorio de prueba
  - `test_playlist_crud_operations`
  - `test_media_controls_integration` (simulado, sin hardware real)

---

## Fase 4: Mejoras de Arquitectura

### 4.1 Separar concerns en el motor de audio
- **Objetivo**: Mejorar mantenibilidad y testabilidad
- **Pasos**:
  1. Extraer la lógica de decodificación de `engine.rs` a un módulo `decoder.rs`
  2. Separar la gestión de dispositivos CPAL a `device_manager.rs`
  3. Crear un trait `AudioDecoder` para permitir múltiples backends (symphonia, miniaudio, etc.)
  4. Implementar inyección de dependencias para facilitar pruebas mock

### 4.2 Mejorar el sistema de configuración
- **Objetivo**: Centralizar y validar configuraciones
- **Pasos**:
  1. Crear un struct `AppConfig` con serde para serialización/deserialización
  2. Validar configuraciones al inicio (sample_rate válido, buffer_size > 0, etc.)
  3. Agregar soporte para perfiles de configuración (low-resource, high-fidelity, default)
  4. Implementar hot-reload de configuración sin reiniciar la app

### 4.3 Implementar sistema de logging estructurado
- **Objetivo**: Mejorar debugging y monitoreo
- **Pasos**:
  1. Configurar `tracing-subscriber` con niveles y filtros por módulo
  2. Agregar spans para operaciones críticas (decodificación, DSP, I/O)
  3. Implementar log rotation para evitar archivos gigantes
  4. Agregar métricas de rendimiento (latencia de audio, uso de RAM, FPS de UI)

---

## Fase 5: Optimizaciones Avanzadas

### 5.1 Perfilado y benchmarking
- **Herramientas**: `cargo flamegraph`, `criterion`, `massif` (valgrind)
- **Objetivos**:
  - Identificar hotspots en el pipeline de audio
  - Medir impacto de cada módulo DSP en latencia
  - Perfilar uso de memoria de la UI y cache de carátulas
  - Establecer benchmarks de referencia para futuras optimizaciones

### 5.2 Optimizaciones específicas de plataforma
- **Linux**: Usar ALSA/PipeWire directamente si cpal no es óptimo
- **Windows**: Optimizar WASAPI exclusive mode para baja latencia
- **macOS**: Usar Core Audio directamente si hay overhead en cpal
- **General**: Evaluar si `cpal` necesita reemplazo por `cpal` + `rodio` o solución custom

### 5.3 Optimización de compilación
- **Objetivo**: Reducir tiempos de compilación en desarrollo
- **Pasos**:
  1. Configurar `cargo-llvm-cov` para perfiles de desarrollo más rápidos
  2. Evaluar si algunos crates pueden ser feature-gated para desarrollo
  3. Configurar `sccache` o `cargo-nextest` para compilación incremental más rápida
  4. Considerar dividir en workspace si el tiempo de compilación crece

---

## Fase 6: Documentación y Mantenibilidad

### 6.1 Documentación de API interna
- **Objetivo**: Facilitar contribuciones y mantenimiento
- **Pasos**:
  1. Agregar doc comments a todas las funciones públicas y `pub(crate)`
  2. Documentar invariantes y precondiciones en el motor de audio
  3. Crear diagramas de flujo para el pipeline de audio y DSP
  4. Actualizar README con información correcta (iced, no egui)

### 6.2 Mejorar CI/CD (futuro)
- **Objetivo**: Automatizar verificaciones de calidad
- **Pasos**:
  1. Configurar GitHub Actions para `cargo check`, `cargo test`, `cargo clippy`
  2. Agregar workflow de benchmarking con criterion
  3. Configurar cobertura de pruebas con `cargo-tarpaulin`
  4. Agregar linting de commits y PRs

---

## Priorización Recomendada

1. **Inmediato**: Fase 1 (errores y estabilidad) → Fase 3.1-3.3 (pruebas básicas)
2. **Corto plazo**: Fase 2.1-2.3 (optimizaciones de audio y memoria) → Fase 4.1 (separación de concerns)
3. **Mediano plazo**: Fase 2.4 (UI) → Fase 3.4 (integración) → Fase 5 (perfilado avanzado)
4. **Largo plazo**: Fase 4.2-4.3 (arquitectura) → Fase 6 (documentación y CI)

---

## Métricas de Éxito

- ✅ `cargo test` pasa sin errores
- ✅ `cargo clippy -- -D warnings` sin warnings
- ✅ Latencia de audio < 50ms en configuración default
- ✅ Uso de RAM < 500MB con biblioteca de 10k canciones
- ✅ Cobertura de pruebas > 80% en módulos críticos
- ✅ FPS de UI > 50 en listas grandes con scroll
