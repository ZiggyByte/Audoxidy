# Análisis Detallado del Módulo de Biblioteca de Audio

Este documento proporciona una visión técnica profunda del funcionamiento de la biblioteca en Audoxidy.

---

## 1. Arquitectura Técnica y Funcionamiento General
La biblioteca de audio está gestionada principalmente por el componente `LibraryManager` en `src/gui/library.rs`. Su arquitectura sigue un patrón de **Gestión de Estado Centralizada** y **Virtualización de Vistas**.

### Especificaciones Técnicas Clave:
- **Modelo de Datos**: Utiliza `SongData` y `ArtistGroup` para estructurar la información.
- **Caché Proactiva**: Al iniciar, el sistema carga todas las canciones de la base de datos en `cached_all_songs`. Esto permite que búsquedas y filtros se realicen en memoria (`O(n)` en memoria en lugar de lentas consultas SQL constantes).
- **Virtualización**: Tanto la cuadrícula como las listas utilizan un sistema que solo renderiza los elementos visibles. Esto permite que la interfaz permanezca fluida incluso con colecciones de más de 100,000 canciones.

---

## 2. El Valor de 300px x 600px (Lazy Margin)
Se ha identificado que estos valores (300.0 y 600.0) no corresponden a dimensiones fijas de una función, sino a un **margen de renderización perezosa (lazy rendering margin)** dentro de la lógica del Grid Virtualizado.

- **Ubicación**: `src/gui/library.rs`, dentro del widget `responsive`.
- **Funcionamiento**: Define una zona de "amortiguación" por encima y por debajo del área visible del scroll.
- **Diferenciación**:
    - **600px**: Es el estándar para hardware normal. Asegura que cuando haces scroll hacia abajo, los siguientes álbumes ya estén renderizados y sus carátulas cargadas antes de que entren en tu vista, proporcionando suavidad total.
    - **300px**: Se activa automáticamente si el sistema detecta pocos recursos (`is_low_resource()`), reduciendo la carga de memoria a la mitad.

---

## 3. Ordenamiento y Visualización
La biblioteca organiza los elementos de forma diferente según la vista seleccionada para maximizar la legibilidad.

### Vista de Cuadrícula (Grid)
- **Criterio de Ordenamiento**: Es un ordenamiento canónico de tres niveles: **Artista -> Año -> Nombre del Álbum**.
- **Lógica de Visualización**: Los álbumes se agrupan en filas (`chunks`) dinámicas de entre 2 y 10 columnas según el tamaño de la ventana.
- **Navegación**: Permite expandir un álbum ("Toggle Expansion") para mostrar sus canciones directamente debajo, manteniendo el contexto del Grid.

### Vistas de Lista (Simple, Detallada, Miniaturas)
- **Criterio de Ordenamiento**: Sigue una jerarquía técnica: **Álbum Artista -> Año -> Álbum -> Disco -> Número de Pista**.
- **Lógica de Visualización**: Utiliza el widget `universal_song_list`. Las canciones se agrupan por Artista. Si una canción no tiene artista definido, aparece en "Artista Desconocido".
- **Cabeceras**: En las listas, al hacer scroll, el nombre del artista actual se queda "pegado" en la parte superior (sticky header) hasta que llega el siguiente artista.

---

## 4. Funcionamiento de los Filtros
El sistema de filtrado es **Global y Sincronizado**. Se procesa en el método `apply_filter()`.

1. **Entrada**: Recibe la búsqueda del cuadro de texto (`search_query`) y los filtros seleccionados en el panel lateral (Carpeta, Artista, Álbum, Género, Año).
2. **Filtrado de Canciones**: Crea una lista `filtered_songs` que cumple todos los criterios.
3. **Sincronización del Grid**: El Grid se actualiza para mostrar únicamente los álbumes que contienen las canciones del paso anterior.
4. **Búsqueda de Discografía**: Si buscas un nombre de álbum directamente, el sistema lo muestra incluso si el filtro de artista es diferente, permitiendo encontrar álbumes por nombre rápidamente.

---

## 5. Formas de Refrescar la Biblioteca
Existen tres mecanismos para que la vista se actualice:

1. **Auto-Refresco por Scanner (Automático)**: El Scanner monitoriza los archivos en disco en segundo plano. Si detecta cambios, marca la base de datos como "sucia". El loop principal de la app (`Tick`) detecta esto y recarga los datos de la biblioteca automáticamente sin intervención del usuario.
2. **Refresco por Interacción (Reactivo)**: Escribir en la búsqueda o cambiar un filtro dispara instantáneamente una llamada a `apply_filter()`, actualizando la UI en menos de 1ms gracias a la memoria caché.
3. **Refresco de Origen (Source Change)**: Cambiar entre "Local", "Spotify", etc., limpia el caché actual y carga los datos correspondientes al origen seleccionado.
