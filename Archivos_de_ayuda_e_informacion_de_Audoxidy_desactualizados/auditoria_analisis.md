# Reporte de Auditoría: Audoxidy (Parte 1 - Análisis Técnico)

Este documento detalla los hallazgos del análisis exhaustivo realizado sobre la arquitectura actual de Audoxidy. El objetivo es identificar cuellos de botella y áreas críticas que afecten la escalabilidad para gestionar bibliotecas de más de 200,000 canciones y 40,000 álbumes con bajo consumo de CPU y RAM.

## 1. Módulo Principal (`app.rs` y Ciclo de Vida de UI)
- **Ciclo Update Centralizado:** La aplicación depende de mensajes periódicos (`Tick` a 500ms / 5000ms y `UITick`). Modificar el estado global de la aplicación (como los textos de la marquesina) en este nivel superior provoca que **Iced repinte y recalcule el diseño de todo el árbol de widgets** de la aplicación, lo que dispara el uso de CPU.
- **Bloqueos Síncronos:** El uso de `self.database.lock()` dentro del hilo principal de Iced (`update`) es peligroso. Si el `scanner` está realizando una transacción de 500 inserciones, la UI se congelará por completo al intentar leer la base de datos.
- **Gestión de Memoria en Estado Global:** Se mantienen en memoria copias masivas de las estructuras de datos (listas de canciones completas).

## 2. Acceso a Datos y Base de Datos (`database.rs`)
- **Aciertos:** El esquema SQLite está bien normalizado y el uso de `WAL` (Write-Ahead Logging), índices `FTS5` y `mmap` es la decisión correcta para rendimiento puro.
- **Cuellos de Botella (Carga en Memoria):** Consultas como `get_all_songs` y `get_grid_items_by_artist` recuperan el 100% de las filas de golpe. A nivel de +200k canciones, cargar esto en un `Vec<SongData>` generará latencias notables y consumos severos de RAM en cada "refresco" de la librería.
- **Concurrencia Estricta:** La base de datos está encapsulada en un único `Arc<Mutex<Database>>`. A pesar de tener `WAL` (que permite 1 escritor y N lectores en SQLite), este Mutex fuerza a que la lectura y escritura ocurran de manera secuencial y bloqueante a nivel de la aplicación Rust.

## 3. Escáner de Archivos (`scanner.rs`)
- **Procesamiento de Etiquetas:** La recolección de metadatos usa `lofty` muy bien, con una resolución de prioridades (Vorbis > Id3v2 > Id3v1) excelente. 
- **Transacciones y Batching:** Se hace un commit cada 500 archivos, lo que es eficiente. Sin embargo, las consultas individuales preparadas se ejecutan una por una dentro del bucle en vez de aprovechar operaciones *Bulk* optimizadas, lo que retrasa los escaneos gigantes.
- **Generación de Carátulas Simultánea:** El descubrimiento de carátulas envía tareas en paralelo a `covers.rs`, lo que es eficiente, pero si la velocidad de disco en lectura de archivos supera a la velocidad de codificación AVIF, se puede encolar un exceso de mensajes y saturar la RAM con las imágenes crudas extraídas.

## 4. Biblioteca y Virtualización (`library.rs` y `library_filters.rs`)
- **Aciertos:** La idea de crear un `FilterIndex` en `library_filters.rs` pre-calculado es una optimización brillante para búsquedas $O(1)$. La virtualización en listas (solo renderizar lo que cabe en pantalla calculando offsets `top_space` / `bottom_space`) es la única forma de escalar en Iced.
- **Recálculo Pesado:** La función `get_view_structure` y el agrupamiento por artistas/álbumes a nivel de aplicación (iterando todo el vector de memoria) es $O(N)$ o $O(N \log N)$ cuando hay ordenamientos. Con 200,000 elementos, iterar y mapear todo el arreglo cada vez que se teclea una letra en la búsqueda causará *stuttering* y caídas graves de FPS en la interfaz.

## 5. Listas de Reproducción (`playlist.rs`)
- La forma de manejar "linear_idx" mapeando grupos, carpetas y canciones planas es funcional, pero a escalas masivas puede volverse compleja. Al aplanar la lista constantemente al realizar acciones, los tiempos de procesamiento escalan linealmente. 
- La persistencia de la posición de la playlist graba a base de datos muy frecuentemente (incluso en eventos como `SeekTo`).

## 6. Motor Gráfico y Componentes (`widgets.rs` y `utils/covers.rs`)
- **Fuga Lógica en Caché de Iced:** En `covers.rs`, Audoxidy carga imágenes usando `Handle::from_path` o `Handle::from_bytes`. Iced por defecto *cachea internamente* estas imágenes decodificadas en la GPU/RAM. Si un usuario hace un scroll rápido a través de 40,000 álbumes en modo Grid, Iced retendrá decenas de miles de texturas, lo que terminará provocando que el sistema operativo mate el proceso por exceso de memoria (OOM). No existe actualmente una estrategia proactiva de descarte de imágenes fuera de pantalla.
- **Truncado de Texto (`smart_truncate_text`):** El procesamiento de textos y fuentes a nivel de Rust antes de enviar al motor de renderizado puede ser pesado si se repite constantemente durante el scroll.

---
*Este archivo detalla el estado actual. Las propuestas y arquitecturas para solucionar estos cuellos de botella se encuentran en la Fase 2.*
