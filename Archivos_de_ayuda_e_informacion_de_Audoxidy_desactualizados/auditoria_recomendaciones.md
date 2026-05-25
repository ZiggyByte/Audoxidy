# Reporte de Auditoría: Audoxidy (Parte 2 - Recomendaciones y Optimizaciones)

A partir de los hallazgos en el análisis de escalabilidad para manejar +200,000 canciones y +40,000 álbumes, a continuación se recomiendan las siguientes mejoras estructuradas por orden de impacto y prioridad.

## 1. Concurrencia de Base de Datos y Supresión de Bloqueos (Crítico) [✔️ COMPLETADO]
**Problema:** `Arc<Mutex<Database>>` paraliza la UI y las lecturas mientras el escáner escribe.
**Solución:** 
- **Pool de Conexiones (`r2d2` o `deadpool_sqlite`):** Ya que se usa el modo `WAL`, implementar un pool de conexiones permitirá que el hilo de Iced lea desde la base de datos de manera simultánea mientras el hilo del escáner (`scanner.rs`) realiza transacciones pesadas de escritura.
- Alternativamente, tener **dos conexiones directas separadas**: Una `Connection` de solo lectura (RO) embebida en el estado de Iced, y otra `Connection` de lectura/escritura (RW) entregada exclusivamente al `Scanner`.

## 2. Refactor del Ticker y la Marquesina (Rendimiento UI) [POSTERGADO]
**Problema:** Iced repinta todo el árbol a 2 FPS (500ms) por la marquesina global en `app.rs`.
**Solución:**
- Eliminar el `UITick` de 500ms del `update` principal de `app.rs`. 
- *(Postergado para Migración a Canvas)* Crear un **Widget Personalizado tipo Canvas en Iced** para la marquesina de texto, la barra de progreso (Seek Bar) y los contadores de tiempo. Al aislar todo el panel inferior de reproducción en un Canvas, se elimina la necesidad del `Tick` de 500ms en el árbol principal de Iced. El Canvas se encargará de dibujar animaciones a 60 FPS (deslizamiento de texto y progreso de barra) y manejar eventos del ratón (clic en la barra para adelantar), lo que reducirá el uso de CPU de la UI a 0% mientras se reproduce música, ya que Iced dejará de recalcular las listas de miles de álbumes repetidamente.

## 3. Paginación y Virtualización "Lazy" desde SQLite [NO ES NECESARIA]
### ⚠️ NO ES NECESARIA ESTA IMPLEMENTACION DADO QUE YA TENEMOS PLICADA LA DEDUPLCIACION DE DATOS MEDIANTE EL CACHE DE STRINGS (INTERNER)
**Problema:** Cargar un `Vec<SongData>` de 200,000 ítems en la RAM colapsará el uso de memoria de la app.
**Solución:**
- En lugar de mantener `cached_all_songs`, la UI debe consultar a la base de datos bajo demanda usando `LIMIT` y `OFFSET`.
- En `library.rs`, el `LibraryManager` solo debe saber **cuántas** canciones/álbumes hay (`COUNT(*)`) para pintar la barra de scroll falsa, y al mover el scroll, pedir a la base de datos la "página" correspondiente (ej. 100 ítems) de manera asíncrona para pintar las filas visibles.
- Mover toda la lógica de ordenamiento (`sort_column`) y agrupado (`get_view_structure`) a la base de datos. SQLite usando índices es órdenes de magnitud más rápido y eficiente en memoria que iterar y ordenar en Rust.

## 4. Gestor Global de Memoria y Caché de Carátulas (Prevención de Fugas OOM) [✔️ COMPLETADO]
**Problema:** Iced decodifica y destruye imágenes continuamente durante el scroll rápido (provocando tirones y alto I/O de disco) y, cuando la aplicación se minimiza o se deja en reposo, mantiene recursos innecesarios en la memoria RAM y VRAM.
**Solución (Modo Fantasma):**
- **Caché LRU (Least Recently Used) en `covers.rs`:** Implementar un caché manual con un límite exacto de **150 carátulas**. Al mantener vivos los `Handles` en Rust, obligamos a la GPU a retener las texturas visibles, permitiendo un scroll inverso instantáneo sin volver a leer el disco.
- **Módulo `memory_manager.rs` (Garbage Collector):** Crear un reloj de inactividad que se reinicia con cualquier movimiento del ratón o acción. Si el usuario no interactúa durante **5 minutos**:
  1. Expulsará las carátulas más antiguas del Caché LRU de `covers.rs`.
  2. Llamará explícitamente a `iced::widget::image::clear_cache()` para forzar la liberación de VRAM en el motor gráfico.
  3. Vaciará búsquedas, filtros y cachés de listas almacenados en `library.rs` y `app.rs`, dejando la RAM y VRAM de la aplicación al mínimo indispensable mientras la música sigue sonando en segundo plano.

## 5. Optimizaciones en el Escáner (`scanner.rs`) [✔️ COMPLETADO]
**Problema:** Cuello de botella en inserciones de base de datos grandes.
**Solución:**
- Preparar los `Statements` (`conn.prepare_cached`) una sola vez fuera del bucle de carpetas.
- Para importaciones masivas, configurar temporalmente `PRAGMA synchronous = OFF` o usar parámetros que reduzcan la escritura a disco durante la transacción inicial.
- **Control de Presión en RAM:** Limitar el tamaño de la cola del hilo procesador de imágenes AVIF en `covers.rs`. Si la cola pasa de 1000 imágenes crudas en memoria esperando comprimirse, bloquear el escáner momentáneamente (`yield` o sleep) para darle tiempo al procesador y no exceder la RAM del usuario en modo "Bajos Recursos".

## 6. Gestión de Eventos y Optimización de Filtros 
- **Desacoplar eventos pesados de la UI:** Cuando un usuario escribe en el buscador de la librería, el recálculo (si todavía usa memoria) no debe ejecutarse en cada pulsación de teclado. Implementar un **Debouncer** (ej. de 200ms a 300ms) de modo que si el usuario escribe la palabra "Queen" rápidamente, la búsqueda solo se lance cuando termine de teclear, evitando 5 recálculos pesados.
- **Persistencia de Playlist:** Bajar la frecuencia de grabado. Que la app almacene en RAM el estado `last_pos_sec` y `SeekTo` pero solo realice el `UPDATE` físico en la DB al pausar, al cambiar de canción, o de manera asíncrona cada 30 segundos, reduciendo I/O de disco.

## 7. Optimización de Memoria Avanzada (Estructuras Compactas) [✔️ COMPLETADO]
- **Uso de `Arc<str>` en lugar de `String`:** Modificar la estructura `SongData` para que campos repetitivos como `artist`, `album`, y `genre` utilicen `Arc<str>`. De esta forma, si 500 canciones pertenecen al mismo artista, el string solo existirá una vez en la memoria RAM y las canciones solo guardarán un puntero ligero. Esto reduce drásticamente el consumo global de memoria.
- **Internamiento de Cadenas (String Interning):** Implementar un caché o pool global para reutilizar etiquetas idénticas generadas desde la base de datos.

## 8. Rendimiento y Aislamiento del Motor de Audio
- **Aislamiento Estricto del Hilo de Audio:** Asegurar que el hilo encargado del motor de audio tenga la máxima prioridad del sistema operativo y no se vea bloqueado por tareas de I/O de la interfaz o el escáner. Esto prevendrá "crujidos" o caídas de audio cuando el disco esté bajo alta carga durante un escaneo de una biblioteca masiva.

## 9. Interfaz de Renderizado de Bajo Nivel (Canvas)
- **Renderizado de Filas con Canvas:** Si a pesar de la virtualización la lista de la biblioteca sigue consumiendo recursos al instanciar widgets, explorar la migración del renderizado de las filas a una capa de bajo nivel usando el widget `Canvas` de `iced`. Esto evita la creación de múltiples estructuras de widget y permite dibujar a 60 FPS con un costo de CPU casi nulo.

## 10. Búsqueda Difusa (Fuzzy Search)
- **Búsqueda Avanzada con FTS5:** Aprovechar al máximo la tabla FTS5 de SQLite para implementar una búsqueda difusa. Esto permitirá encontrar canciones incluso con errores tipográficos menores o de coincidencia parcial, mejorando significativamente la experiencia de usuario.

---
*Fin del Reporte.*
