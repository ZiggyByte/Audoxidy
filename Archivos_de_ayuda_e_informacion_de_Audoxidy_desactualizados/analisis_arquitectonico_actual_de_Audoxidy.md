Perfecto. Ahora tengo un entendimiento completo de toda la arquitectura. Vamos a crear el análisis exhaustivo.

📋 ANÁLISIS ARQUITECTÓNICO COMPLETO — MÓDULO DE LISTAS DE REPRODUCCIÓN
1. Estado Actual de la Arquitectura
Base de Datos (SQLite + rusqlite)
Tabla	Propósito
SONGS	Datos esenciales: file_path, title, track_num, duration, album_id, artist_id, etc.
ALBUMS	Identidad del álbum con hash_id, title, artist_id, year, genre, cover_path
ARTISTS	Identidad global del artista con hash_id deduplicado
FOLDERS	Rutas de directorios escaneados
SONGS_FTS	Full-text search (FTS5) con title, artist_name, album_title denormalizados
SONG_METADATA	Metadata extendida: lyrics, comments, composer, gain, etc.
SONG_TAG_ITEMS	Datos crudos de tags (ID3v2, VorbisComments, etc.)
Observación crítica: No existe ninguna tabla de playlists actualmente.

Motor de Audio
cpal para output + Symphonia para decoding
Thread dedicado de decodificación con ring buffer lock-free
AudioState compartido via Arc<RwLock<>>
EOF detection → auto-advance via play_next()
PlaylistManager Actual
Estructura: Vec<(String, Vec<PlaylistItem>)> — completamente en memoria
PlaylistItem duplica metadata (title, artist, album, duration, path) que ya existe en la BD
Shuffle: fastrand::usize(0..len) — repite canciones antes de agotar todas
Repeat: 3 estados (Off, All, One) — funcional
Sin historial de shuffle, sin persistencia, sin skip/disable tracks
Sin drag-and-drop
GUI (Iced 0.14)
Layout: player_view (400px) + filters_view (202px) + library_view (flex)
playlist_view (400px) apilado debajo de player_view
Virtualización implementada en library pero NO en playlist
Navegación por teclado implementada en library (flechas + enter)
3 search boxes existentes (playlist, library, filters)
Sin drag-and-drop nativo en Iced
2. Análisis de Drag-and-Drop: iced_drop vs Custom
iced_drop v0.2.23
Aspecto	Evaluación
Compatibilidad	✅ Compatible con iced 0.14
API	droppable(widget).on_drop(Message) + zones_on_point()
Mecánica	Escanea zonas por ID de widget en punto de release
Ejemplos	Color blocks, Todo board (Kanban-style)
Reordering	No tiene API dedicada; requiere lógica custom
Limitación principal	Usa iced::advanced::widget::Id — incómodo para listas dinámicas con miles de items
Performance	Escanea TODAS las zonas en cada drop → O(n) por drop. Con miles de canciones, será lento
Drop entre contenedores	✅ Funciona (Todo example lo demuestra)
Drag desde OS	❌ No soportado
Madurez	Sin releases oficiales oficiales, author planea mejoras futuras
Implementación Custom desde Cero
Aspecto	Evaluación
Control total	✅ Puedes optimizar para listas virtuales
Drag desde OS	✅ Necesitas iced::event::listen() para Event::PlatformSpecific
Complejidad	Alta: necesitas gestionar estado de drag, hit testing, visual feedback
Reutilización	Necesitas un sistema genérico para múltiples lugares
⚠️ Recomendación: Enfoque Híbrido
Para Fases 1-2 (actual): No implementar drag-and-drop todavía. Primero construir la arquitectura de datos, la UI base, shuffle robusto, skip/disable tracks, y persistencia. El drag-and-drop se implementa en Fase 3 cuando el resto esté sólido.

Para Fase 3 (futuro): Implementar un sistema custom de drag-and-drop no usar iced_drop para las listas de canciones, porque:

El escaneo O(n) de zonas no escala a miles de canciones
El sistema de widget::Id es incómodo para items dinámicos
Necesitas drag desde el explorador de archivos del OS (que iced_drop no soporta)
El sistema custom debe usar:

Estado global de drag en AudoxidyApp (dragging_source, drag_type, drag_data)
mouse_area con on_drag_start, on_drag, on_release
Hit-testing manual basado en coordenadas de scroll viewport
Para drop desde OS: iced::event::listen_with() capturando eventos DroppedFile
3. Arquitectura de Base de Datos Recomendada
Problema Actual a Resolver
El PlaylistItem actual duplica metadata (title, artist, album, duration, path) que ya existe en la BD. Si el usuario tiene 10 playlists con 1000 canciones cada una, hay 10,000 copias de metadata que pueden quedar desincronizadas.

✅ Recomendación: Tablas Normalizadas con Foreign Keys
-- Tabla principal de playlists
CREATE TABLE playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    is_system INTEGER NOT NULL DEFAULT 0,  -- 1 para "Archivos locales" y "Default"
    created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    UNIQUE(name)
);

-- Tabla de items de playlist (solo referencias + estado)
CREATE TABLE playlist_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL,
    song_id INTEGER NOT NULL,
    sequence_order REAL NOT NULL,  -- REAL para facilitar inserciones intermedias sin reordenar todo
    enabled INTEGER NOT NULL DEFAULT 1,  -- 1 = se reproduce, 0 = skip automático
    added_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
    FOREIGN KEY (song_id) REFERENCES SONGS(id) ON DELETE CASCADE,
    UNIQUE(playlist_id, song_id)  -- UNA canción solo una vez por playlist
);

-- Índice para ordenamiento rápido
CREATE INDEX idx_playlist_items_order ON playlist_items(playlist_id, sequence_order);
-- Índice para búsqueda inversa (qué playlists contienen esta canción)
CREATE INDEX idx_playlist_items_song ON playlist_items(song_id);
¿Por qué sequence_order como REAL?
Cuando el usuario reordena canciones via drag-and-drop entre la posición 5 y 6, no necesitas renumerar TODAS las canciones. Simplemente asignas 5.5. Solo cuando los gaps se vuelven muy pequeños haces un REINDEX批量. Esto es el patrón usado por sistemas profesionales.

¿Por qué UNIQUE(playlist_id, song_id)?
Evita duplicados accidentales. Si el usuario quiere la misma canción dos veces, se maneja con sequence_order fraccional diferente, pero en la práctica una canción una vez por playlist es suficiente. Si necesitas duplicados reales, quita este UNIQUE.

Persistencia del estado Shuffle
-- Historial de reproducción shuffle por playlist
CREATE TABLE playlist_shuffle_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL,
    session_id TEXT NOT NULL,  -- UUID generado cuando se activa shuffle
    song_id INTEGER NOT NULL,
    play_order INTEGER NOT NULL,  -- 0, 1, 2, 3... orden en que se reprodujeron
    played_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
    FOREIGN KEY (song_id) REFERENCES SONGS(id) ON DELETE CASCADE
);

CREATE INDEX idx_shuffle_session ON playlist_shuffle_history(playlist_id, session_id, play_order);
Optimización de Consultas
Para mostrar una playlist de 5000 canciones sin lag:

-- Consulta paginada: solo obtener las 100 canciones visibles en pantalla
SELECT s.id, s.title, s.track_num, s.duration, s.file_path,
       a.title as album_title, ar.name as artist_name, a.year
FROM playlist_items pi
JOIN SONGS s ON s.id = pi.song_id
JOIN ALBUMS a ON a.id = s.album_id
JOIN ARTISTS ar ON ar.id = s.artist_id
WHERE pi.playlist_id = ? AND pi.enabled = 1
ORDER BY pi.sequence_order
LIMIT 100 OFFSET ?
Clave: La UI solo renderiza lo visible. Con virtualización, necesitas calcular alturas totales pero solo query los items visibles.

4. Shuffle Robusto: Algoritmo de Fisher-Yates con Historial
Problema Actual
fastrand::usize(0..len) repite canciones aleatoriamente sin control.

Solución: Fisher-Yates Shuffle + Historial
// Cuando el usuario activa shuffle:
fn activate_shuffle(&mut self, playlist_id: i64, db: &Database) {
    // 1. Obtener todas las canciones enabled de la playlist
    let songs = db.get_playlist_songs(playlist_id, true);
    
    // 2. Fisher-Yates shuffle para orden aleatorio sin repetición
    let mut indices: Vec<usize> = (0..songs.len()).collect();
    for i in (1..indices.len()).rev() {
        let j = fastrand::usize(0..=i);
        indices.swap(i, j);
    }
    
    // 3. Guardar como orden de reproducción
    self.shuffle_order = indices;
    self.shuffle_position = 0;
    self.shuffle_history = vec![];  // Para backward navigation
}

fn play_next_shuffle(&mut self) -> Option<usize> {
    if self.shuffle_position < self.shuffle_order.len() {
        let idx = self.shuffle_order[self.shuffle_position];
        self.shuffle_history.push(idx);
        self.shuffle_position += 1;
        Some(idx)
    } else if self.repeat_mode == 1 {
        // Repeat All: reshuffle y empieza de nuevo
        self.activate_shuffle(self.active_playlist_id, db);
        self.play_next_shuffle()
    } else {
        None  // Fin de playlist
    }
}

fn play_prev_shuffle(&mut self) -> Option<usize> {
    if !self.shuffle_history.is_empty() {
        self.shuffle_history.pop();  // Quitar actual
        if let Some(&idx) = self.shuffle_history.last() {
            self.shuffle_position = self.shuffle_history.len();
            Some(idx)
        } else {
            None
        }
    } else {
        None
    }
}
Ventaja: Nunca se repite una canción hasta que TODAS se han reproducido. El historial permite retroceder exactamente al orden previo.

5. Sistema de Skip/Disable Tracks (Indicador “•”)
Lógica
Cada playlist_item tiene enabled: bool
El “•” indicador se muestra:
Color secundario → enabled (se reproduce en auto-advance)
Transparente/oculto → disabled (se salta en auto-advance)
Siempre se puede reproducir manualmente (click/enter) sin importar el estado
Implementación
// En PlaylistManager
pub fn toggle_song_enabled(&mut self, song_idx: usize) {
    if let Some(item) = self.active_list.get_mut(song_idx) {
        item.enabled = !item.enabled;
    }
}

pub fn toggle_album_enabled(&mut self, album_folder: &str) {
    // Toggle todas las canciones del mismo álbum
    for item in self.active_list.iter_mut() {
        if item.album_folder == album_folder {
            item.enabled = !item.enabled;
        }
    }
}

// En play_next() - auto-advance
fn play_next_auto(&mut self) {
    loop {
        // next_idx calculado por shuffle o sequential
        let next_idx = self.calculate_next();
        if let Some(item) = self.active_list.get(next_idx) {
            if item.enabled {
                self.playing_song_idx = Some(next_idx);
                self.load_and_play(&item.path);
                return;
            }
        }
        // Si está disabled, saltar al siguiente
        self.advance_index();
        if self.has_wrapped_around() {
            return; // Todas las canciones están disabled
        }
    }
}
Agrupación por álbum en la playlist
Cuando se agregan canciones, agruparlas por folder_path (carpeta del álbum):

struct PlaylistGroup {
    folder_name: String,      // Nombre de la carpeta
    songs: Vec<PlaylistItem>, // Canciones de ese álbum
    all_enabled: bool,        // Para toggle rápido del grupo
}
6. Optimización de UI con Virtualización
Problema
Con 5,000 canciones en una playlist, renderizar todo causa lag.

Solución: Virtualización (ya implementada en library)
Reutilizar el patrón de universal_song_list() en widgets.rs:

// Calcular espacio total
let total_height: f32 = grouped_items.iter()
    .map(|item| item.height())
    .sum();

// Determinar rango visible desde scroll viewport
let visible_start = viewport.y;
let visible_end = viewport.y + viewport.height;
let lazy_margin = 300.0;

// Solo renderizar items visibles + margen
let mut visible_rows = column![];
let mut accumulated = 0.0;
for item in grouped_items {
    if accumulated + item.height() > visible_start - lazy_margin
       && accumulated < visible_end + lazy_margin {
        visible_rows.push(item.render());
    }
    accumulated += item.height();
}
Carga diferida de metadata
-- NO hacer JOIN en cada render
-- En su lugar, obtener datos en batch para el rango visible:
SELECT s.title, s.track_num, s.duration,
       a.title, ar.name, a.year
FROM playlist_items pi
JOIN SONGS s ON s.id = pi.song_id
JOIN ALBUMS a ON a.id = s.album_id
JOIN ARTISTS ar ON ar.id = s.artist_id
WHERE pi.playlist_id = ?
  AND pi.sequence_order BETWEEN ? AND ?
ORDER BY pi.sequence_order;
7. Pestañas de Playlist con Scroll
Arquitectura de la barra superior (40px alto)
[Archivos locales] [Default] [Mi Playlist] [▼]
                                    ↑
                            Flecha desplegable
Implementación:

Medir ancho total de todas las pestañas renderizadas
Si total_width > available_width:
Mostrar pestañas que caben + icono arrow-down-chevron.svg (28px)
Las pestañas ocultas van en un menú desplegable vertical
Las pestañas fijas (“Archivos locales”, “Default”) nunca se mueven al menú
El menú desplegable es un container con column de botones
Drag-and-drop de pestañas (Fase 3)
Cada pestaña es un mouse_area con:

on_press → seleccionar playlist
on_drag_start → iniciar drag con playlist_id
on_release → calcular drop target por coordenadas
8. Barra Inferior (40px alto)
Layout
[🔍 Buscar... (170px)]          [🎵] [🔁] [🔀] [📝]
 ← align left            align right →
Iconos (de derecha a izquierda):
Icono	Archivo	Estado	Función
Lyrics	lyrics-outlined.svg	Secondary	Abrir letras (Futura)
Shuffle	shuffle-rounded-outlined.svg	Accent si activo	Reproducción aleatoria robusta
Repeat	repeat-rounded-outlined.svg / repeat-one-rounded-outlined.svg	Accent si activo	Repeat Off/All/One
Ecualizador	equalizer-rounded.svg	Secondary	Abrir tab Ecualizador
Hover: Cambiar a COLOR_TEXT_PRIMARY via mouse_area con on_enter/on_exit

9. Recomendaciones Críticas
9.1 Evitar Duplicación de Metadata
NO almacenes title, artist, album, duration en PlaylistItem. En su lugar:

pub struct PlaylistItem {
    pub song_id: i64,         // FK → SONGS(id)
    pub sequence_order: f64,
    pub enabled: bool,
}

// Para display, hacer JOIN en la query:
// SELECT s.title, s.artist, s.album, s.duration
// FROM playlist_items pi JOIN SONGS s ON s.id = pi.song_id
Si necesitas acceso rápido en memoria (cache):

pub struct PlaylistSongCache {
    pub song_id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f32,
    pub track_num: u32,
    pub year: String,
    pub file_path: String,
}
Esta cache se reconstruye al cargar la playlist y se descarta al cerrar. Nunca es la fuente de verdad — la BD siempre manda.

9.2 Persistencia
Guardar playlist: INSERT OR REPLACE INTO playlist_items ... batch al agregar/quitar/reordenar
Guardar estado de reproducción: playing_song_idx, shuffle_history, shuffle_order → tabla separada playlist_state
Auto-save: Al cambiar de playlist o cerrar la app
9.3 Prevención de Race Conditions
El scanner puede actualizar SONGS mientras el usuario ve una playlist. Solución:

-- Cuando el scanner actualiza una canción existente:
-- ON CONFLICT(file_path) DO UPDATE SET ...
-- Esto no afecta playlist_items porque referencia por song_id
9.4 Rendimiento con Playlists de 10,000+ Canciones
Virtualización obligatoria — solo renderizar ~50 items visibles
Índices en sequence_order — el ORDER BY debe ser instantáneo
Carga en batch — obtener 200 canciones por query, no 10,000
Cover cache — precargar covers visibles con el sistema existente de enqueue_cover_job
Búsqueda local — el search box filtra en memoria, no en la BD (ya tienes las canciones cargadas)
9.5 Drag-and-Drop desde Explorador de Archivos del OS
Iced soporta Event::PlatformSpecific(PlatformSpecific::Unix(_)) para file drops en Linux. Necesitas:

// En subscriptions()
iced::event::listen_with(|event, _, _| {
    match event {
        iced::Event::PlatformSpecific(
            iced::event::PlatformSpecific::Unix(
                iced::event::unix::Event::DroppedFile(path)
            )
        ) => Some(Message::FileDroppedFromOS(path)),
        _ => None,
    }
})
10. Plan de Implementación por Fases
FASE 1: Cimientos de Base de Datos
[ ] Crear tablas: playlists, playlist_items, playlist_shuffle_history
[ ] CRUD básico: crear playlist, agregar canciones, eliminar
[ ] Migración: crear “Archivos locales” y “Default” como playlists del sistema
[ ] Funciones DB: get_playlist_songs(), insert_playlist_item(), delete_playlist_item(), update_playlist_item_order()
FASE 2: UI Base del Módulo de Playlist
[ ] Rediseñar barra superior con pestañas dinámicas
[ ] Menú desplegable de pestañas overflow
[ ] Lista de canciones con diseño de 2 filas + separadores de carpeta
[ ] Indicador “•” con toggle (click + espacio)
[ ] Navegación por teclado (flechas + enter)
[ ] Barra inferior con iconos SVG y búsqueda local
FASE 3: Lógica de Reproducción Robusta
[ ] Fisher-Yates shuffle con historial completo
[ ] Repeat Off/All/One con iconos SVG correctos
[ ] Skip automático de canciones disabled en auto-advance
[ ] Toggle enabled/disabled por canción y por álbum
[ ] Persistencia de shuffle history en BD
FASE 4: Persistencia y Optimización
[ ] Auto-save de playlist items a BD
[ ] Virtualización de lista de canciones
[ ] Cache de metadata en memoria (reconstruible)
[ ] Precarga de covers con sistema existente
FASE 5: Drag-and-Drop Custom
[ ] Sistema custom de drag-and-drop (NO iced_drop)
[ ] Reordenar pestañas de playlists
[ ] Reordenar canciones dentro de playlist
[ ] Mover canciones entre playlists
[ ] Drag desde biblioteca de audio a playlist
[ ] Drag desde explorador de archivos del OS
FASE 6: Funciones Avanzadas
[ ] Crear nueva playlist (UI + BD)
[ ] Renombrar playlist
[ ] Eliminar playlist
[ ] Letras (Lyrics) — integrar con módulo futuro
[ ] Ecualizador → abrir tab desde playlist
