# Audoxidy

Reproductor de audio en alta definicion, alto rendimiento y personalizable para Linux, Windows y MacOs, multi-idioma e inspirado en AIMP v5.40. Implementado íntegramente en Rust con enfoque en baja latencia, seguridad de memoria, eficiencia de recursos y experiencia de usuario coherente con entornos Linux modernos con un diseño modular.

 - En este archivo README.md encontraras parte de los requisitos que queremos para crear nuestro reproductor de musica en alta fidelidad, ademas en la misma carpeta en la que se encuentra este mismo archivo README.md encontraras el archivo INTERFACE.md en ese archivo se detalla parte del diseño d ela interfaz, ademas tambien se anexan imagenes para que las analices por completo, las comprendas y sepas como debe quedar la interfaz ya terminada, las imagenes anexadas so los archivos: "diseño.png", "modular.png", "equalizador.png y "opciones.png". 
 Siempre regresa a este archivo readme las veces que sea necesario para que revises la documentacion de las librerias oficiales, sepas los requisitos sobre las funciones del reproductor y su diseño final. (Importante. Siempre debes responder todo en español)

## Funcionalidades Principales

### Motor de Audio
- IMPORTANTE. Puedes optimizar el funcionamiento del uso del reproductor para que use hasta 1/3 de los hilos totales que tenga el procesador dependiendo de lo que requiera hacer el reproductor en ciertos momentos.
- Reproducción multiformato: MP3, FLAC, OGG, WAV, APE, WavPack, AAC, Opus, ALAC.
- Formatos AC3, DSD y DTS: Para estos formatos, ofrecer el soporte solo si el usuario tiene instalado ffmpeg en su sistema operativo.
- Soporte para frecuencias de muestreo: 44.1 kHz, 48 kHz, 96 kHz, 192 kHz
- Canales: Mono, Estéreo (2.0, 2.1), 5.1, 7.1
- Profundidad de bits: 16/24/32-bit y 32-bit float (esta opcion debe ser facilmente seleccionable por el usuario final).
- Baja latencia En Rust: usa cpal con backend ALSA directo, buffer de 64–256 frames, y procesamiento sin heap; cpal + ALSA + hilo dedicado + buffers pequeños = solución óptima en Rust.
- Ecualizador gráfico seleccionable por el usuario final a 20 y 31 bandas por canal y global (hasta 180 filtros en 5.1).
- Ecualizador Paramétrico (Parametric EQ).
- Linear Phase EQ
- Resampling de alta gama con múltiples algoritmos, usando rubato (https://crates.io/crates/rubato) o algun otro que recomiendes que sea el mas optimo y mejor para nuestro reproductor.
- Implementar los siguientes efectos de audio:
  -Aumento de graves
  -Aumento de volumen
  -Expansor Estéreo
  -Balance Estéreo
  -Fader
  -ReplayGain
  -Preamp/Gain
  -Crossfeed
  -HRTF
  -Reducción de ruido
  -Reverberación
  -Compresor/Limitador
  -Delay
  -Distorsion
  -Chorus
  -Flanger
  -Phaser
  -DSP en Cadena Personalizable (Component-based DSP): La capacidad de ordenar efectos en una cadena arbitraria (EQ → Crossfeed → Reverb → Limiter).
- Soporte ReplayGain (lectura automática de tags y análisis en segundo plano).
- Análisis de correlación estéreo (visual).
Todos estos efectos de audio no deben estar activos siempre para no empeorar el rendimiento del sistema operativo, solo se iran activando cuando el usuario los vaya a usar.
- Grabación de streams de radio (guardado en MP3, OGG, FLAC).
- Conversión entre formatos (MP3 requiere binario externo `lame` o algun otro que tu recomiendes).

### FRECUENCIAS DEL ECUALIZADOR PROFESIONAL BASADO EN EL ESTANDAR IEC.
# Frecuencias del ecualizador de 31 Bandas 1/3 octava:
 20Hz, 25Hz, 31.5Hz, 40Hz, 50Hz, 63Hz, 80Hz, 100Hz, 125Hz, 160Hz, 200Hz, 250Hz, 315Hz, 400Hz, 500Hz, 630Hz, 800Hz, 1kHz, 1.25kHz, 1.6kHz, 2kHz, 2.5kHz, 3.15kHz, 4kHz, 5kHz, 6.3kHz, 8kHz, 10kHz, 12.5kHz, 16kHz, 20kHz

# Frecuencias del ecualizador de 20 Bandas:
22.4Hz, 31.5Hz, 45Hz, 63Hz, 90Hz, 125Hz, 180Hz, 250Hz, 355Hz, 500Hz, 710Hz, 1kHz, 1.4kHz, 2kHz, 2.8kHz, 4kHz, 5.6kHz, 8kHz, 11.2kHz, 16kHz

# Frecuencias del ecualizador de 15 Bandas:
25Hz, 40Hz, 63Hz, 100Hz, 160Hz, 250Hz, 400Hz, 630Hz, 1kHz, 1.6kHz 2.5kHz, 4kHz, 6.3kHz, 10kHz, 16kHz

# Frecuencias del ecualizador de 10 Bandas:
31.5Hz, 63Hz, 125Hz, 250Hz, 500Hz, 1kHz, 2kHz, 4kHz, 8kHz, 16kHz

### Gestión de Metadatos
- Lectura y escritura de metadatos completa usando lofty: ID3v1/v2, Vorbis, APE, MP4.
- Extracción automática de portadas embebidas.
- Descarga inteligente automática de metadatos y portadas mediante APIs:
  - MusicBrainz y otras
  - Cover Art Archive y otras.
- Caché persistente local mediante base de datos SQLite para bibliotecas de +50,000 canciones.
- Escaneo paralelo de bibliotecas grandes de +50,000 archivos con prioridad media sin impacto en reproducción.

### Conectividad
- Acceso a fuentes remotas:
  - NAS: SMB/CIFS (smbclient)
  - WebDAV
  - FTP
  - HTTP/HTTPS
- Integración con servicios en la nube: Nextcloud, Plex (vía WebDAV) y más.
- Soporte para streams de radio por Internet (ICY/HTTP streams, HLS básico).
- Conexión e Integración con plataformas de streaming cuando el usuario tiene credenciales válidas:
  - Spotify (API oficial + OAuth2)
  - YouTube Music (streams públicos compatibles)
  - SoundCloud (API pública)
  - Tidal (API)
  - Dezeer (API)
  - y más plataformas de streaming.

### Interfaz de Usuario Modular
- Arquitectura basada en paneles dockables y flotantes.
- Paneles independientes:
  - Biblioteca musical
  - Listas de reproducciones
  - Lista de reproducción activa
  - Lista personalizable con datos de Genero, Artista, Album, Año y más.
  - Visualizadores (espectro, forma de onda)
  - Letras (lyrics)
  - Carátula del álbum
  - Ecualizador
  - Panel de Ecualizador y efectos
  - Panel de visualizacion y edicion de metadatos completa
  - Panel de conversión/grabación
- Comportamiento de docking:
  - Desacoplamiento total: Los paneles pueden desacoplarse y moverse libremente, incluso a monitores secundarios.
  - Ajuste automático al acercar paneles: Detección de proximidad para acoplamiento automático, permite agrupar o separar.
  - Grupos de paneles: Al acoplar se mueven como unidad cuando están vinculados.
  - Tamaño fijo o adaptativo por panel (configurable por el usuario)
- Sin ventanas adicionales en la barra de tareas: Todos los paneles se renderizan dentro de una única ventana nativa → no generan entradas adicionales en la barra de tareas.

### Personalización Visual Avanzada
- Sistema de skins basado en archivos RON.
- Control total sobre:
  - Colores de fondo, texto, bordes y elementos interactivos (por sección y global).
  - Tipografía: fuente, tamaño y estilo por componente UI, (biblioteca, lista, EQ, etc.).
  - Imágenes de fondo por panel y global, con opciones de posición y escala.
  - Modo dinámico: usar carátula del álbum actual como fondo, con opcion de activar/desactivar este modo dinamico.
  - Extracción automática de colores dominantes de la carátula para generar paleta de UI dinamica y automatica cuando el usuario elige esta opción..
- Efectos visuales:
  - Transparencia por panel.
  - Desenfoque de fondo mediante shaders WGPU.
- Gestión de iconos:
  - Detección automática del tema de iconos del sistema (escaneo de `$XDG_DATA_DIRS/icons`).
  - Soporte para múltiples paquetes de iconos instalados.
  - Carga de iconos personalizados para Audoxidy.
  - Recoloreo automático de iconos del sistema solo si el icono es monocromáticos según modo claro/oscuro.
  - El usuario puede definir color de iconos independientemente del modo.
  - El usuario puede elegir color de iconos (blanco en oscuro, negro en claro, o personalizado)

### Gestión de Estilos y Skins
  - Guardado, exportación e importación de configuraciones completas de UI
  - Incluye: layout, colores, fuentes, iconos, fondos, transparencias, etc.
  - Múltiples skins guardados con cambio instantáneo desde menú
  - Formato de skin: **RON** (legible y eficiente).

### Integración con el Sistema
- Soporte MPRIS: control desde barra de tareas, teclas multimedia, widgets y más, para control desde el sistema operativo.
- Notificaciones del sistema al cambiar de pista, cambiar el volumen, etc. ( opcion de activar/desactivar por el usuario), y tambien crear notificaciones personalizadas propias del con el estilo visual del reproductor (opcion de activar/desactivar por el usuario).
- Atajos de teclado globales (X11/Wayland).
- Detección automática del entorno de escritorio (GNOME, KDE, Cinnamon, MATE, XFCE, XQT, etc.).
- Adaptación automática al esquema de color del sistema (claro/oscuro) y aplicar skin compatible automaticamente.

## Dependencias del Sistema (Linux)
- libasound2 (ALSA)
- libxcb
- libdbus-1
- libsmbclient (opcional, para NAS)
- openssl
- libnotify (opcional, para notificaciones)
- libxkbcommon (opcional, para atajos de teclado) 
- ffmpeg (opcional, para formatos DTS, DSD, AC3, etc.)

## Crates de Rust Utilizados
- cpal
- symphonia
- lofty
- rusqlite
- egui
- eframe
- egui_dock
- egui_extras
- reqwest
- tokio
- smbclient
- webdav-client
- async-ftp
- image
- usvg
- resvg
- rustfft
- rubato
- hrtf
- dasp
- zbus
- color-thief
- kmeans-colors
- serde
- ron
- rayon
- tracing
- tracing-subscriber
- parking_lot
- libloading (para plugins futuros)

## Dependencias de Rust actualizadas a su ultima version estable (Usa solamente las ultimas versiones estables de todos las librerias, a continaucion te pongo los enlaces a la Documentación oficial de cada una de ellas:)

Version actual de Rust estable e instalada en el sistema: 1.92.0 (BUSCA LA DOCUMENATCION DE ESTA ULTIMA VERSION PARA QUE LA ANALICES, ENTIENDAS Y COMPRENDAS LOS CAMBIOS Y MEJORAR QUE SE IMPLEMENTARON )

# Audio
cpal, version: 0,17,1 (DOCUMENTACION: https://docs.rs/cpal/latest/cpal/ )
symphonia, version: 0.5.5 (DOCUMENTACION: https://docs.rs/symphonia/latest/symphonia/ )
rubato, version, 1.0.1 (DOCUMENTACION: https://docs.rs/rubato/latest/rubato/ )
audioadapter, version, 2.0.0 (DOCUMENTACION: https://docs.rs/audioadapter/latest/audioadapter/ )
audioadapter-buffers, version, 2.0.0 (DOCUMENTACION: https://docs.rs/audioadapter/latest/audioadapter/ )
audioadapter_sample, version, 2.0.0 (DOCUMENTACION: https://docs.rs/audioadapter-sample/latest/audioadapter_sample/)
rustfft, version, 6.4.1 (DOCUMENTACION: https://docs.rs/rustfft/latest/rustfft/ )
hrtf, version, 0.8.1 (DOCUMENTACION: https://docs.rs/hrtf/latest/hrtf/ )
dasp , version, 0.11.0 (DOCUMENTACION: https://docs.rs/dasp/latest/dasp/ )
ringbuf, version, 0.4.8 (DOCUMENTACION: https://docs.rs/ringbuf/latest/ringbuf/ )

# UI
egui, version, 0.33.3 (DOCUMENTACION: https://docs.rs/egui/latest/egui/ )
eframe, version, 0.33.3 (DOCUMENTACION: https://docs.rs/eframe/latest/eframe/ )
egui_dock, version, 0.18.0 (DOCUMENTACION: https://docs.rs/egui_dock/latest/egui_dock/ )
egui_extras, version, 0.33.3 (DOCUMENTACION: https://docs.rs/egui_extras/latest/egui_extras/ )
image, version, 0.25.9 (DOCUMENTACION: https://docs.rs/image/latest/image/ )
resvg, version, 0.46.0 (DOCUMENTACION: https://docs.rs/resvg/latest/resvg/ )
usvg, version, 0.46.0 (DOCUMENTACION: https://docs.rs/usvg/latest/usvg/ )

# Async & Network
tokio, version, 1.49.0 (DOCUMENTACION: https://docs.rs/tokio/latest/tokio/ )
reqwest, version, 0.13.1 (DOCUMENTACION: https://docs.rs/reqwest/latest/reqwest/ )
async_ftp, version, 6.0.0 (DOCUMENTACION: https://docs.rs/async_ftp/latest/async_ftp/ )

# Database & Metadata
rusqlite, version, 0.38.0 (DOCUMENTACION: https://docs.rs/rusqlite/latest/rusqlite/ )
lofty, version, 0.22.4 (DOCUMENTACION: https://docs.rs/lofty/latest/lofty/ )
walkdir, version 2.5.0 (DOCUMENTACION: https://docs.rs/walkdir/latest/walkdir/ )

# Serialization & Config
serde, version, 1.0.228 (DOCUMENTACION: https://docs.rs/serde/latest/serde/ )
ron, version, 0.12.0 (DOCUMENTACION: https://docs.rs/ron/latest/ron/ )

# Utils
tracing, version, 0.1.44 (DOCUMENTACION: https://docs.rs/tracing/latest/tracing/ )
tracing-subscriber, version, 0.3.22 (DOCUMENTACION: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/ )
parking_lot, version, 0.12.5 (DOCUMENTACION: https://docs.rs/parking_lot/latest/parking_lot/ )
rayon, version, 1.11.0 (DOCUMENTACION: https://docs.rs/rayon/latest/rayon/ )
color-thief, version, 0.2.2 (DOCUMENTACION: https://docs.rs/color-thief/latest/color_thief/ )
kmeans_colorsversion, 0.7.1 (DOCUMENTACION: https://docs.rs/kmeans_colors/latest/kmeans_colors/)
zbus, version, 5.13.2 (DOCUMENTACION: https://docs.rs/zbus/latest/zbus/ )
base64, version, 0.22.1 (DOCUMENTACION: https://docs.rs/base64/latest/base64/ )
regex, version 1.12.3 (DOCUMENTACION: https://docs.rs/regex/latest/regex/ )
rfd, version, 0.17.2 (DOCUMENTACION: https://docs.rs/rfd/latest/rfd/ )
libloading, 0.9.0 (DOCUMENTACION: https://docs.rs/libloading/latest/libloading/ )
rand, 0.9.2 (DOCUMENTACION: https://docs.rs/rand/latest/rand/ )



# NOTA MUY IMPORTANTE. REVISA LA DOCUMENTACION DE TODOS LOS PAQUETES PARA APLICAR TODAS SUS FUNCIONALIDADES CORRECTAMENTE, PRINCIPALMENTE LOS PAQUETES QUE TIENEN QUE VER CON TODO EL AUDIO, DSP y GUI, PARA QUE NUESTRO CODIGO ESTE ADAPTADO A LAS NUEVAS VERSIONES DE LOS PAQUETES Y ASI APROVECHAR SUS GRANDES MEJORAS Y SU FUNCIONAMIENTO SEA EL CORRECTO, SEGURO Y OPTIMIZADO, NO TRATES DE ADAPTAR EL CODIGO DE LOS PAQUETES A NUESTRA CODIGO YA QUE PODEMOS PROVOCAR MUCHOS ERRORES, NOSOTROS DEBEMOS ADAPTAR EL CODIGO AL FUNCIONAMIENTO DE LOS PAQUETES ACTUALIZADOS A SU ULTIMA VERSION ESTABLE, TAMBIEN PUEDES AGREGAR NUEVOS PAQUETES SI CREES QUE HAY MEJORES SOLUCIONES QUE NOS AYUDEN CON ALGUNAS FUNCIONES PARA NUEVAS IMPLEMENTACIONES. TAMBIEN ES IMPORTANTE QUE LOS ARCHIVOS DE AYUDA POR EJEMPLO: TASK, IMPLEMENTATION PLAN Y WALKTHROUGH DEBEN ESTAR SIEMPRE EN ESPAÑOL Y NO DEBES AGREGAR CODIGOS O CAMBIOS EN EL CODIGO A ESTOS ARCHIVOS DE AYUDA Y PLANEACION


Puedes descargar, instalar y configurar mas librerias y paquetes si llega a ser necesario y siempre dbsucar yd escargar sus ultimas versiones estables.
Tambien puedes agregar recomendaciones para ciertas configuraciones, optimizaciones y demas recomendaciones que vayas encontrando.
