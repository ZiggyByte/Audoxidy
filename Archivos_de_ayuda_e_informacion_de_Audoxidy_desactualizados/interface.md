Esta es una descripción técnica y visual, detallada.


### Diseño de Interfaz de Usuario predeterminada para el Reproductor de Música Audoxidy  - Especificación Completa

- Estética General y Paleta de Colores:
  - No se usara el diseño de ventanas por defecto del sistema operativo.
  - Diseño sin bordes
  - El diseño debe seguir un estilo "Dark Mode" moderno, minimalista y plano (Flat Design).
  - Color Principal del Reproductor: (Color #000000).
  - Color de Contraste del Reproductor: (Color: #111111), utilizado para elementos inactivos.
  - Color de Acento del Reproductor: Casi rojo (Color: #FF003D) utilizado para elementos activos, selecciones, logotipos y barras de progreso.
  - Tipografía de Logo: El Nombre de Audoxidy tendra el "Color de Texto Principal" y se mostrara solo en donde se indique y se escribira así: "AuDoxiDY" usando unicamente la fuente personalizada "Stage Wander" que se encuentra en la carpeta assets/fonts, y cuando el puntero del mouse se coloque sobre el nombre, las letras "A, D, Y" se deberan cambiar al "color de acento" del reproductor pero con una animacion letra por letra de las 3 letras que te acabo de indicar.
  - Tipografia de Textos: Noto Sans que se encuentra en la carpeta assets/fonts:
  Color Principal de Textos:  #c6c6c6 para textos principales.
  Color Secundario de Textos: #717171 para textos secundarios.
- Estructura: Modular y flexible, permitiendo que los paneles se acoplen en una sola entre si o en una ventana grande y se puedan desacoplar (undock) en ventanas flotantes independientes.
-Iconos personalizados par usar en el reproductor se encuentran en assets/icons, estos iconos estan en formato svg, por lo que se pueden redimensionar, y cambiar el color del icono, tambien es importante mencionar que hay varios diseños para un icono, usaremos los iconos que tengan en su nombre "outlined, fill y rounded", los iconos outlined son iconos que solo tiene contorno y son los que se mostraran siempre y usaremos los icnos fill, cuando se posicione (hover) el puntero del mouse sobre el icono o se de clic en el, tambien hay algunos iconos que no tienen otros estilos extras, pero aun asi los vamos a usar para las funciones que vayamos implementando, escanea la carpeta, para que localices los iconos que iras usando conforme vayamos creando el reproductor.

### Módulo 1: El Reproductor (Ventana Principal / Panel Lateral / Ventana Flotante)
- Este es el gestor de contenido principal.
- Este módulo actúa como el controlador principal y puede estar anclado a la izquierda de la biblioteca o flotar independientemente.
-Cabecera Visual (Now Playing): Aqui se muestra la carátula del álbum actual en grande, con un tamaño proporcional de 1:1, este modulo siempre debe mantener el mismo aspecto de ratio 1:1. Sobre ella funciona el gestor principal de reproduccion de audio con el siguiente diseño y funciones:
Importante. El area de la caratula del album es por capas.
1. Capa 1: Esta area es el primer espacio que tu asignas dentro de la UI que va a ocupar la caratula del album, esta es como la capa madre de todas las demas, asi que es importante que siempre debe mantener un aspecto de ratio de 1:1 y el contenido de todas las demas capas dentro de esta deben adaptarse al tamaño de esta capa madre.
2. Capa 2: En esta capa se debe incrustar la caratula del album de la cancion que se este reproduciendo.
3. Capa 3: En esta capa debe ir una opacidad del 30% (Esta opcion despues la vamos a habilitar para que el usuario cambie el porcentaje de opacidad a su gusto en el panel de personalizacion que vamos a crear despues).
4. Capa 4: El tamaño total que ocupa el area de la caratula del album se divide en 10 partes (10/10) horizontales y funcionará de la siguiente manera;
Contando de arriba a bajo:
 - En la primer division 1/10: Alineado a la izquierdo y alineado al centro de forma vertical dentro su division irá un icono de 3 rayas horizontales (el llamado icono menu hamburguesa) que se encuentra en /assets/icons y el icono se llama "menu.svg" en este icono al dar clic sera el menu principal y se mostrara en forma de lista desplegable; en el centro ira informacion de los canales que tiene la cancion que se esta reproduciendo y solo se mostrara cuando la cancion que se esta reproduciendo tiene 1 canal (se debera mostrar "Mono"), 2.1, 4.0, 5.1, 7.1, etc. y solo se deberan mostrar asi como te lo escribi, cuando la cancion solo tiene canales estereo (2.0) no se mostrara nada y cuando se muestren los canales de la cancion se debera mostrar una ligera opacidad de fondo abarcando el tamaño total del texto; alineados a la derecha iran los iconos del control de la ventana: minimizar (icono "minimize.svg"), maximizar (icono "maximixe.svg") y cerrar reproductor (icono "close-big.svg"), Todo el contenido de esta seccion tendra el mismo "Color principal de la tipografia". Todos estos iconos deberan tener un tamaño de 32px y cuando se situe el puntero sobre alguno de esos iconos, esa area de ese icono tendra una (hover) opacidad del 70% en forma de circulo perfecto.
 - En la division 8/10: tendra un tamaño fijo de 40px de alto y aqui se mostrara el nombre de la cancion alineado a la izquierda y alineado al centro de forma vertical dentro su divisioncon una tipografia en un tamaño mas grande y en el "Color principal de la tipografia" ya asignado anteriormente y tendra un efecto de deslazamiento de marquesina hacia la izquierda que se activara solo cuando el nombre de la cancion sea mas largo que el tamaño del area de la caratula del album, (Estas opciones despues se van a poder ajustar desde el panel de personalizacion que vamos a crear despues, donde el usuario va a a poder elejir la alineacion a la izquierda, centro o derecha, activar o desactivar el efecto marquesina).
 - En la division 9/10:  tendra un tamaño fijo de 30px de alto y aqui se mostrara el nombre del artista del album alineado a la izquierda y alineado al centro de forma vertical dentro su division con una tipografia un poco mas pequeña y en el "Color principal de la tipografia" ya asignado anteriormente y tendra un efecto de deslazamiento de marquesina hacia la izquierda que se activara solo cuando el nombre del artista sea mas largo que el tamaño del area de la caratula del album; alineado del lado derecho y alineado al centro de forma vertical dentro su division se va a mostrar el contador de progreso del tiempo de la cancion en formato minutos:segundos [M:SS] y en el "Color principal de la tipografia" ya asignado anteriormente y este tiempo de progreso solo debe aparecer cuando se esta reproduciendo una cancion play/pause y desaparecer cuando se detuvo la cancion (STOP) o cuando no hay mas canciones que reproducir y debe estar sincronizada con la barra de progreso de la cancion, (Estas opciones despues se van a poder ajustar desde el panel de personalizacion que vamos a crear despues, donde el usuario va a a poder elejir la alineacion a la izquierda, centro o derecha, activar o desactivar el efecto marquesina).
 - En la division 10/10: Tendra un tamaño fijo de 40px de alto y aqui se mostrara Alineado al centro de forma vertical dentro su division irá la barra progreso de la cancion y busqueda seek, debe funcionar tambien para saltar inmediatamente sin latencia a un segundo en especifico que el usuario elija dentro de la cancion cuando de clic sobre la barra de progreso  y debe estar sincronizada con el contador de progreso del tiempo de la cancion, el fondo de esta division tendra un fondo de desenfoque (blur), la barra de progreso sera del "Color de Contraste del Reproductor" ya asignado anteriormente e ira cambiando al "Color Principal de la Tipografia" tambien ya asignado anteriormente conforme vaya avanzando la cancion, (Estas opciones despues se van a poder ajustar desde el panel de personalizacion que vamos a crear despues, donde el usuario va a poder elejir entre mostrar esta barra de progreso o cambiarla por una barra de progreso y busqueda en forma de onda completa, activar o desactivar el fondo desenfocado (blur), los colores de la barra de progreso y su fondo).
5. Capa 5: En esta capa es donde iran los controles de audio y tomando la division de 10/10 horizontales que definimos anteriormente, solamente vamos a tomar las divisiones de 2/10 a 9/10 (contando de arriba a abajo) y la vamos a dividir en 3/3 verticales y aqui vamos a colocar los controles de audio:
 - En la division izquierda ira el control de cancion anterior, al dar clic con el clic primario del mouse en esa division se cambiara a la cancion anterior.
 - En la division del centro ira el control de play/pause, al dar un clic con el clic primario del mouse funciona como si fuera el boton de play/pause, los botones deben saber si se esta reproduciendo una cancion o no, para que al dar un clic se realice la accion correcta y se muestre el icono correcto dependiendo con el estado de reproductor.
 - En la division derecha ira el control de siguiente cancion, al dar clic con el clic primario del mouse en esa division se cambiara a la cancion anterior.
- Ademas, en toda esta seccion que abarca las divisiones de 1/10 a 9/10 del area del caratula del album, funcionara como un control de volumen; con la rueda del mouse al subirla o bajarla sirve como un control de volumen, subiendo y bajando el volumen, la sensibilidad del mouse debe ser de 60% de 100% y mostrando brevemente en el centro de la seccion un icono acorde a esta funcion y tambien el nivel de volumen en el que se encuentra en ese momento y que abarcara del 0 a 100, el volumen por default cuando se instale el reproductor sera del 30.

- Todos estos controles e iconos no se deben mostrar siempre, solo se mostraran cuando se da clic en alguna de estas secciones, cancion anterior (izquierda), play/pause, stop (centro), cancion siguiente (derecha) o cuando se usa la rueda para cambiar el volumen y en sus respectivas divisiones y sin cambiar la opacidad que tenga configurada el album y todos con un desvanecimiento de salida, los iconos tambien deberan ir alineados al centro horizontalmente de cada division y al centro verticalmente y contando desde las divisiones 1/10 a 9/10 del area del caratula del album.
- Importante. Dado que en esta sección no se muestran los controles, se debera mostrar brevemente un icono corespondiente a cada una de las acciones a la que se haya dado clic y cuando sea la accion de volumen se debera mostrar brevemente el icono correspondiente a este, ademas de mostra el nivel de volumen al que se encuentra que va de 0 a 100 y todos con un desvanecimiento de salida.

### Modulo 2: Listas de Reproducción
- Este es el gestor de listas de reproduccion principal que se puede acoplar y desacoplar al modulo principal que es donde se encuentra la caratula del album, esta seccion se puede acoplar a la seccion principal y adaptar el tamaño del ancho al tamaño que tenga la seccion principal de la caratula de album si asi lo requiera el usuario, su posicion por defaul es abajo del "modulo 1: Caratula del album".
- Estas modulos mantienen la consistencia estética de colores principal, sin bordes y tiene los siguientes diseños y funciones:
  - Barra superior: de este modulo solo se muestran el nombre de las listas de reproduccion en forma de pestañas y tendra un ancho de 35px de alto, el tamaño de cada pestaña es:
    - Las pestañas adaptan su tamaño al nombre de la lista de reproduccion (manteniendo la consistencia de colores del reproductor con el mismo color de fondo, y solo cuando se selecciona una lista esta cambia a un tipoo de letra en negrita y el fondo de toda la pestaña cambia a un color un poco mas claro para identificarla mejor).
    - Las pestañas se pueden cambiar de posicion solo arrastrandolas dentro de esta area de pestañas, y solo cuando el tamaño del modulo es pequeño y la cantidad de listas de reproduccion es mayor de las que se pueden mostrar debido al tamaño del modulo que tenga en ese momento, aparecera un elemento interactivo una "Flecha de Despliegue" que es el icono "arrow-down-chevron.svg" que se encuentra en la carpeta assets/icons este es un icono de flecha hacia abajo (chevron) alineado en el lado derecho, al final de todas las listas de reproduccion que se muestran en ese momento y al hacer clic en la "Flecha de Despliegue" se despliega una pequeña lista en forma de submenu con todas las demas listas de reproduccion que hay, dentro este submenu en forma de lista que muestra las demas listas de reproduccion que hay, tambien se puede arrastrar una lista de reproduccion para cambiar su posicion dentro de todas las listas que se hayan creado y no solo para moverla de posicion dentro del submenu desplegable, si no que tambien moverla hacia las listas que estan las pestañas.
  - Lista de Reproducción Actual (Playlist): Debajo de las pestañas se muestra una lista vertical compacta de las canciones en cola ("Now Playing") que tiene la lista de reproduccion seleccionada.
    - Empezando por una division primaria que puede mostrar el nombre de album, carpeta, artista, duracion total de esa seccion, o demas opciones que se puedan elegir (esta inforqamcion se distingue por un tipo de letra un poco mas grande y en negrita), y ademas se pueden combinar todos estos datos a mostrar configurable por el usuario (todo esto configurable desde el panel de personalizacion), para asi poder separar mejor la informacion de canciones que hay en la lista de reproduccion, dependiendo de las opciones que el usuario elija y en la forma en que quiere separar y distinguir mas facil todas las canciones.
    - La lista de canciones que hay en la opcion elegida o configurada previamente tiene 2 filas: La primera fila (tipografia en color blanco) contiene el Número de pista y Título y duracion; La Sefunda fila (tipografia en color gris) contiene el nombre del artista, album  y año, (toda la informacion que pueden contener estas filas se puede cambiar y reordenar a gusto del usuario y se pueden agregar mas opciones todo configurable desde el panel de personalizacion).
    - La canción activa reproduciendose actualmente está resaltada con el texto con el color de acento explicado anteriormente y en negrita, el usuario se puede mover entre las canciones, al selecccionar una cancion esta estara resaltada con un fondo en color gris claro manteniendo el color y tamaño de de la tipografia) y cuando se da doble clic en ella se reproducira automaticamente y cambiara su resaltado cambiando el color de la tipografia al color de acento y en negrita automaticamente.
  - Barra Inferior: Muestra con iconos minimalistas y tendra un tamaño de 35 px de alto.
    - Búsqueda rápida (Solo para buscar dentro de la lista de reproduccion actual)
    - Botones de control: Anterior, Play/Pausa, Siguiente, Shuffle (aleatorio), Repeat (repetir), ecualizador, temporizador de apagado.
    _ Barra de volumen.
    - Estos botones tambien se pueden insertan en otros modulos y ademas se pueden agregar o quitar que botones mostrar, cada uno de estos botones se puede insertar en varios paneles a la vez y elegir la posicion en la que se muestran dentro de los paneles y con iconos y colores personalizados (todo configurable desde el panel de personalizacion). 

### Modulo 3: Filtros de la Biblioteca (Filtros / Lista)
Este es el gestor de contenido y filtrado principal para la "Biblioteca musical", que se puede acoplar y desacoplar, su posicion por default es a la izquierda del modulo de la "Biblioteca musical" .

Panel de Navegación: Que contiene un árbol de categorías: Género, Artista, Álbum, Año, etc. Los iconos que se usara cuando el arbol este cerrado es "arrow-right-chevron.svg" y cuando el arbol este abierto es "arrow-down-chevron.svg".

Barra de Filtros Superior: Una fila horizontal con menús desplegables para filtrar por: Título, Género, Año, Artista, Duración, Formato y Tasa de bits y alineado a la derecha ira un icono que se usara cuando el menu desplegable este cerrado es "arrow-right-chevron.svg" y cuando el menu desplegable este abierto es "arrow-down-chevron.svg", los iconos se encuentran la carpeta assets/icons.

- Barra Inferior: tendra un tamaño de 35 px de alto y se mostrara una barra de Búsqueda rápida (Solo para buscar dentro de seccion de filtros) en color de contraste principal del reproductor, tal como ocurre en el modulo de lista de rerproduccion

### Módulo 4: Biblioteca Musical (Ventana Principal / Grid)
Este es el gestor principal donde se encuentra toda la biblioteca musical.
Área de Contenido (Grid de Álbumes):

Disposición: Los álbumes se muestran en una cuadrícula (grid) regular de tarjetas por default, tambien se podra elegir mostrar la biblioteca musical en forma de lista, en forma de lista con la caratula del album a la izquierrda y a un costado toda la lista de canciones que tiene ese album, en forma de  lista con la caratula del album a la izquierda en cada cancion y en forma de lista simple.

Tarjeta de Álbum en modo cuadricula: Cada tarjeta contiene la imagen de portada. Debajo de la portada, hay información de texto alineada a la izquierda:
Nombre del Artista
Título del Álbum
Genero
Año
Toda esta informacion va ir en el "color de texto secundario" y alineado a la derecha se mostrara el icono de flecha de despliegue "arrow-down-chevron.svg" y el icono "arrow-up-chevron.svg", los iconos se encuentran la carpeta assets/icons y su comportamiento es el siguiente:

Elemento Interactivo "Flecha de Despliegue": Estara Alineado del lado derecho y al centro verticalmente del tamaño de la información de cada tarjeta de álbum, aqui es donde se mostrara siemprre el icono de flecha hacia abajo "arrow-down-chevron.svg".

Comportamiento de Expansión (Inline Expansion):

Al hacer clic en la "Flecha de Despliegue" de un álbum específico, la cuadrícula se "rompe" visualmente y el icono cambia a "arrow-up-chevron.svg" y sucede lo siguiente:

Se genera un espacio horizontal completo justo debajo de la fila donde se encuentra el álbum seleccionado, empujando los albumes que hay a la derecha hacia abajo y en filas inferiores hacia abajo.

Contenido Expandido: En este nuevo espacio, aparece la lista detallada de canciones (tracklist) de ese álbum específico en un "color de texto secundario" y con los metadatos:

Columnas de la lista: # (Número), Título, Género, Año, Artista, Duración, Formato (ej. FLAC, mp3, wav, ogg, etc) y Tasa de bits (ej. 1058 kbps, etc).

Integración: La estructura general de la biblioteca no cambia de ventana; todo ocurre dentro del mismo contenedor, manteniendo la visualización de los otros álbumes arriba y abajo.

-Barra Inferior: tendra un tamaño de 35 px de alto y se mostrara una barra de Búsqueda rápida alineada a la izquierda (Solo para buscar dentro de seccion de biblioteca musical) en color de contraste principal del reproductor, tal como ocurre en el modulo de lista de rerproduccion, al costado derecho de la barra de busqueda se mostrara la informacion completa del total de canciones que hay en la biblioteca musical, asi como el total de albumes agregadas y el tiempo total que suman todas las canciones de la biblioteca musical en "color de texto secundario", alineado totalmente a la derecha se mostrara unicamente el icono que corresponda la vista actual que tiene la biblioteca musical y los iconos son los siguientes:
Grid: "view-grid-outlined.svg"
Lista con cuadricula a la izquierda:"view-list-thumbnail-fill.svg"
Lista con Thumbnail: "view-list-thumbnail-outlined.svg"
Lista simple: "view-list.svg"
Cuando se de clic en este icono se mostrara hacia arriba un pequeño submenu desplegable que contendra todas estas vistas para que se puedan seleccionar y el icono debera cambiar a la vista que se haya elegido.
A un costado de ese icono se mostrara el icono de agregar "more-small.svg" que al presionar este icono se desplegara un submenu desplegable que tendra las opciones: "Agregar archivo" y "agregar carpeta" para que se abra el explorador de archivos y poder agregar canciones o carpetas a la biblioteca de musica.
A un costado estara el boton de reproducir "play-rounded.outlined.svg" funcionara asi: cuando se selecciona una cancion en la biblioteca de musica y se presiona este boton entonces se debera reproducir unicamente esa cancion, cuando se selecciona un album y se presiona este boton entonces se deberan reproducir todas las canciones de ese album.

Vamos a rehacer nuestra base de datos con los metadatos que se deben extraer de las canciones y otros datos que nuestro reproductor podra generar y agregar posteriormente pero que es importante que los espacios para toda esa informacion ya este en la base de datos, asi que elimina todos los datos que se esten guardando actualmente en nuestra base de datos y vamos a cambiarla por los siguientes, en la base de datos usa los nombres de las tablas y columnas en ingles de las siguientes metadatos e informacion:
Caratula del Album (incrustada o dentro de la carpeta en la que esta la cancion o album)
Ruta raiz de la caratula del album original
Ruta raiz de la caratula del album comprimida y cacheada por el reproductor
Numero de Cancion / Numero total de canciones del album al que pertenece
Numero de Disco / Numero de Discos Totales de la coleccion
Titulo de la cancion
Artista
Album
Genero
Año de lanzamiento
Artista del Album
Letra
Track Gain
Album Gain
Notas o comentarios
URL
Copyright
Editor
Compositor
Letrista
Director
Codificado por
Catalogo
ISRC
Key
BPM
Formato
Tamaño
Frecuencia de Muestreo
Canales
Nombre del archivo
Nombre del directorio Raiz
Ruta completa al directorio raiz del archivo
Contador de reproduccion
Fecha de ultima reproduccion
Hora de ultima reproduccion
Calificacion automatica de la cancion
Calificacion personal

Por ahora los metadatos que se extraigan de las canciones seran solo de lectura, posteriormente crearemos un editor de metadatos para editar todos los metadatos de las canciones y poderlos guardar directamente en las canciones.


#### Modulos Auxiliares y Popups
Estas ventanas mantienen la consistencia estética (fondo gris oscuro, con acentos finos en color rojo).

- Popup de Letras (Lyrics): Una ventana flotante simple pero que tambien se podra acoplar a algun otro modulo del reproductor, sin bordes, el color del fondo es consistente con los colores principales ya seleccionados anteriormente pero tambien se pueden cambiar individialmente desde el panel de peronalizacion.
  - Aqui se muestra la letra de la canción sincronizada, resaltando la tipografia de la línea actual en blanco y las futuras ypasadas lineas con el color de la tipografia en color gris.

- Opciones/Configuración principales del reproductor: Ventana con un menú de árbol en una barra lateral la izquierda (Reproducción, Sistema, Interfaz, complementos, etc.) y demas opciones que sean necesarias para configurar). A la derecha, los controles son casillas de verificación (checkboxes) y menús desplegables (dropdowns) con estilo nativo pero oscurecidos. Muestra detalles técnicos como "32 Bit (Float)", "96000 Hz" y tamaño de caché, dependiendo de cada seccion de del submenu elegido en la barra lateral.

 - Popup "Centro de Audio Avanzado":
 Ventana flotante de tamaño fijo y el color de fondo y tipografias es consistente con los colores del reproductor ya seleccionados anteriormente, en este popup habra varias pestañas dentro de ella, la primer pestaña se llama "Configuracion de Audio", la Segunda se llama "Ecualizador", la tercera se llama "Efectos de Audio", asi que dentro de este popup vamos a crear varias pestañas que poco a poco les vamos agregar funciones e iremos agregando mas pestañas conforme las vayamos necesitando y el popup tendra un tamaño de fijo que no se podra cambiar deslizando los bordes del popup, el tamaño del popup es de 864px de ancho fijos y de 474px de alto fijos, la parte superior del popup sera la barra de titulo y tendra un tamaño de 34px de alto y se llamara "Centro de Audio Avanzado", el nombre estara centrado al ancho del tamaño del popup y tambien al centro del tamaño verticalmente del area de titulo, en esta misma barra de titulo estara alineado a la derecha el icono de cerrar popup; despues todo el area restante del popup tendra un padding interno de 15px; debajo de la barra de titulo se mostraran las pestañas, sin bordes y las cuales tendran tambien un padding interno de 10px y cuando este seleccionada una pestaña el color de fondo d ela pestaña cambiara al "Color de Acento" del repoductor, debajo de las pestañas habra un borde delgado del "color de contraste" del reproductor ya definido anteriormente para dividir las pestañas con el contenido de cada pestaña, el contenido de las pestañas es el siguiente: 
  - Pestaña "Configuracion de Audio":
  Aqui se mostraran las configuraciones de calidad de audio, dividiendo el contenido de esta pestaña en dos columnas, izquierda y derecha:
  Columna Izquierda: sera de ajuste de la calidad de audio y las opciones se listaran en un menu desplegable para cada una de las siguientes opciónes:
  - Servidor de Audio: Pipewire (Default), Pulse Audio, Alsa.
  - Dispositivos de Salida: Listar los dispositivos de salida que hay en el equipo.
  - Frecuencia de Muestreo: 44.1kHz, 48kHz, 88.2kHz, 96kHz, 176.4kHz, 192kHz.
  - Profundidad de Bits: 16 bits, 24 bits, 32 bits Float (Default).
  - Canales de salida Automatico: por medio de un togle switch (activar/desactivar), cuando este activado el toggle switch, el reproductor enviara el audio en los canales de salida que tenga el archivo de audio que se esta reproduciendo (mono, stereo, 4.0, etc.) y no permitira ajustar manualmente la opcion de la salida de canales de audio, mostrandola como desactivado, no seleccionable o impedir que se pueda ajustar.
  - Canales de salida: 1.0 Mono, 2.0 Stereo, 2.1 Stereo, 4.0 Quad, 5.1 Surround, 7.1 Surround.
  - Motor HI-FI (Modo Exclusivo): Usando un reglon y usando el color de acento del reproductor dira "Motor Hi-Fi (Motor exclusivo y disponible solo en Linux)" y a derecha el icono de "rocket-launch-outlined.svg" usando el color de acento que tenga el reproductor y cuando este activado el Motor Hi-Fi usar el icono "rocket-launch-fill.svg" usando el color de acento que tenga el reproductor y abajo en otro renglon dira "Bit-Perfect" y tendra un togle switch que activara este modo Hi-Fi; cuando se cambie el togle switch a la posicion de activar, este ajuste no se debera activar inmediatamente, debe haber una segunda aceptacion de confirmacion para activarlo y sera de la siguiente manera: cuando el togle switch de Bit-Perfect cambie a activado, inmediata se abrira un popup, del lado izquierdo se mostrara un icono de advertencia y del lado derecho tendra el siguiente mensaje:
  "Esta apunto de activar el Motor Hi-Fi, el reproductor tendrá el control total de su salida de audio, por lo que no escuchara otros audios o videos en su sistema operativo, tampoco no podrá controlar el volumen del audio, aplicar efectos dsp, compresiones, etc. Esto es para garantizar que la señal de audio se envié a la máxima calidad total a su dispositivo de salida.
  Se recomienda que antes de activar el Motor Hi-Fi baje el volumen de su dispositivo receptor de audio."
  En la parte de abajo de este popup habra dos botones, uno de "Aceptar" y otro de "Cancelar"": Cuando se de clic en el boton de Aceptar, puedes aplicar algun comando inmediatamente para garantizar que la salida de audio solo la controle nuestro reproductor y se active el Motor Hi-Fi correctamente y se cerrara inmediatamente el popup y el togle switch de Bit-Perfect se quedara activado y en el ajuste de "Dispositivos de Salida" cambiara automaticamente a Alsa. Y cuando de clic en el boton de cancelar, no se va a activar el Motor Hi-Fi y se cerrara el popup y el togle switch de Bit-Perfect se quedara desactivado y los demas ajustes como estaban.
  Importante: Cuando el Motor Hi-Fi este activado, en automatico no se permitiran ajustar manualmente las demas opciones de ajustes de la pestaña "Configuracion de audio", deberan mostrarse como desactivadas, no seleccionables o impedir que se pueda ajustar, solo se podran ajustar los "Dispositivos de Salida" y "Servidor de Salida" y tambien en las pestañas de "Ecualizador" y "Efectos" se deberan mostrar todos los ajustes como desactivadas, no seleccionables o impedir que se puedan ajustar, y alineado en el centro del contenido de cada pestaña debe haber un mensaje en dos renglones usando el color de acento del reproductor que diga:
  "Motor Hi-Fi Activado
  No se pueden hacer ajustes"
  Todos los togle switch deben usar el color de acento del reproductor cuando esten activados.
  
  Columna Derecha: se mostrara informacion de la calidad de audio en la que se esta reproduciendo la cancion actual, como por ejemplo:
  Servidor de audio que esta usando.
  Dispositivo de salida que esta reproduciendo.
  Frecuencia de Muestreo.
  Profundidad de bits.
  Canales.
  Quantum.
  Bufer.
  Calidad de input y output
  y Toda la informacion que creas relevante.
  
  Debajo de estas dos columnas y alineado a la izquierda estara el siguiente mensaje informativo usando una tipografia mas pequeña y con el color secundario de textos: "El motor Hi-Fi envia la señal de audio directa para maximizar la calidad de audio".
  Debajo de este mensaje informativo pero alineados a la derecha habra tres botones:
   - El primer boton será "Reiniciar Servicio de Audio", al presionar este boton el reproductor reiniciara el servicio de audio por medio de los comandos correspondientes dependiendo de los servicios de audio que este usando el sistema operativo, nuestro reproductor debe identificar en automatico que servicios de audios esta usando el sistema operativo y aplicar los comandos correctos para reiniciar esos servicios de audio que tiene el sistema operativo.
   - El segundo boton será "Predeterminado", al presionar este boton, todos los ajustes de esta misma pestaña "Configuracion de Audio" volveran a sus ajustes por default.
   - El tercer boton será "Aplicar", al presionar este boton se aplicaran inmediatamente todos los cambios en los ajustes que se hayan selecionado dentro de esta misma pestaña "Configuracion de Audio" y sin latencia, si es necesario que para que se aplique inmediatamente el cambio en los ajustes, puedes aplicar una funcion para que se pause y se reproduzca de nuevo la cancion en automatico en el mismo segundo en el que se quedo la reproducion. o puedes aplicar una funcion para que se detenga por completo la cancion y volver a reproducirla en automatico en el mismo segundo en el que se quedo la reproducion.
    
  - Pestaña "Ecualizador":
  Dentro del area del contenido que tiene la pestaña Ecualizador, primeramente habra una fila que contendra lo siguiente:
   - Alineado a la Izquierda estara el ajuste de activacion el Ecualizador y dira "Activar Ecualizador" y a un costado del lado derecho habra un toggle switch de activar/desactivar el ecualizador, (debe funcionar realmente).
   Al Centro estara el ajuste para seleccionar cuantas Bandas del Ecualizador mostrar y dirá "Bandas:" y a un costado del lado derecho habra dos round-button, el primero dira "31" y a la derecha estara el primer round-button y despues dira "20" y a la derecha se colocara el segundo round-button, asi que cuando se elija alguna de las dos opciones se debera mostrar el ecualizador usando las bandas seleccioandas y esta seccion de seleccion de Bandas debera estar alineado totalmente al centro midiendo todo el ancho que tiene el contenido de la pestaña, (Estas opciones debe funcionar realmente).
   - Alineado a la Derecha estara primero un boton que diga "Predeterminado", al presionar este boton se regresaran a su posicion por default los sliders del pre-amplificador y el ecualizador, a un costado habra otro boton que dirá "Presets" y al presionarlo se desplegaran una lista de opciones abajo de este boton, y la opciones que dira seran: "Cargar", "Guardar": 
    - Cuando se presione la opcion "Cargar" se debera mostrar un popup pequeño que muestre del lado izquiero en forma de lista todos los presets que puede elegir el usuario y ademas se mostraran los presets que guarde el usuario, debe haber un slider para poder desplazarse entre todos los presets que haya, todo ordenados alfabeticamente y del lado derecho de este popup, habra 6 botones en vertical que diran "Aceptar", "Eliminar", "Cancelar", "Importar", "Exportar" y "Predeterminado"; importante: cuando el usuario presione alguno los presets de ecualizacion, ese preset al que se haya dado clic, se debera aplicar inmediatamente el ajuste temporalmente como si fuese una muestra previa del sonido que tendra si activa ese preset que selecciono y si le da clic despues en el boton de "Aceptar" entonces se aplicara el ajuste ya ahora si por completo en el ecualizador y ya no solo de forma temporal y se cerrara el popup, si el usuario selecciona un presets y despues da clic en el boton de "Eliminar" entonces ese preset se va a eliminar y ya no se mostrara en la lista de presets, si el usuario seleciona un preset y despues da clic en el boton "Cancelar" entonces el preset que selecciono no se aplicara en el ecualizador y regresara a los ajustes que tenia el ecualizador antes de previsualizar el preset y se cerrara el popup, cuando el usuario presione el boton "Exportar" entonces se exportaran los ajustes completos de ese preset en un archivo en el formato que tu quieras y que tu creas que es la mejor forma que puedas identificar y cargar correctamente el preset de ecualizacion, cuando el usuario de clic en el boton "Importar" entonces se abrira una ventana de exploracion de archivos para cargar el preset de ecualizacion que quiera cargar el usuario en el formato que elegiste previamente que se exportaria, cuando el usuario presione el boton "Predeterminado" entonces todos los presets con los que cuente el reproductor de forma predeterminada volveran a mostrarse en al lista de presets pero se eliminaran los presets personalizados que haya guardado el usuario, asi que abajo de este boton habra un mensaje usando una tipografia del tamaño de 10px y en el "color de texto secundario" que diga "Advertencia: Esta operación restablecerá los presets predeterminados y se eliminarán los presets personalizados previamente guardados", (Estas opciones debe funcionar realmente).
    - Cuando se presione la opcion "Guardar" se abrira un pequeño ya sea input o popup en donde el usuario podra ingresar un nombre para guardar ese ajuste de ecualizacion que tenga el ecualizador y tendra un boton de guardar y se cerrara el input o popup, aqui elige cual seria la mejor forma para que el usuario pueda ingresar el nombre y guardarlo de forma correcta, (Estas opciones debe funcionar realmente).
    
 - Ecualizador y Pre-Amplificador:
 Debajo del contenido anterior, se mostraran los ajustes de pre-amplificacion y las bandas de ecualizacion distribuidas en una forma de tabla de 3 columnas de la siguiente manera:
 
  - Columna Izquierda irá el Pre-Amplificador: Esta columna tendra un ancho de 50px, el slider estara en vertical y los niveles de pre-amplificacion iran de los -9db a los 9db, el contenido que tendra esta columna es, primero dira "Pre-Amp", debajo ira el slider que la barra del slider debera tener un tamaño total de alto o largo de 160px, debajo del slider se mostrara el nivel en el que se encuentra el slider mostrando solo el numero sin la abreviacion de decibeles ni la palabra completa y cuando el slider este en el nivel 0.0 has que solo se muestre el "0" y de igual forma sin abreviaciones de decibeles, todo el contenido que este dentro de esta columna estara alineado al centro del ancho total de la columna, el valor por defecto del slider del pre-amplificador es "0", el slider se debera deslizar en numero decimales de la siguiente forma: 0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1, 1.1 y asi sucesivamente hasta llegar a 9db y de igual forma para los numeros negativos hasta llegar a -9db, es importante que el slider se mueva por cada numero decimal y se debe escuchar inmediatamente sin ninguna latencia realmente la diferencia auditivamente cuando se va moviendo decimal por decimal.
  
  - Columna Central ira la Guía de Niveles verticales: Esta columna tendra un ancho de 20px y sera una Guia de Niveles Verticales que servira como una guia visual para el slider del pre-amplificador y los sliders del ecualizador en donde se mostra de forma vertical una guia que solo tendra (+9, 0, -9) usando el "color secundario de textos" y un tamaño de tipografia un poco menor, primero se debera colocar un espacio para poder igualar el nivel que tenga el slider del pre-amplificador y despues iran los numeros +9 0 -9 alineandolos correctamente de forma vertical al mismo nivel en el que inicia y termina el slider del pre-amplificador y los sliders del ecualizador, para asi mantener la coherencia y servir como una verdadera guia, esto quiere decir que debes dejar un espaicio arriba y abajo de la guia de niveles para que se centre correctamente con los deslizadores, todo el contenido que este dentro de esta columna estara alineado al centro del ancho total de la columna.
    
  - Columna Derecha ira el Ecualizador: Esta columna tendra un ancho de 722px, cada barra del slider vertical de las bandas del ecualizador deberan tener un tamaño total de alto o largo de 160px, aqui mostraremos dos ecualizadores uno de 20 bandas y otro de 31 bandas, pero por debajo realmente sera solo el ecualizador de 31 bandas profesional siguiendo el standard IEC para ecualizadores 31 bandas de 1/3 octava usando un factor Q constante de 4.4 para limitar la interacción entre bandas adyacentes y cuando el usuario elija usar el ecualizador de 20 bandas por debajo estara enlazado al ecualizador de 31 bandas, los niveles de los ecualizadores iran de los -9db a los 9db, este ecualizador de 31 bandas seguira el standar profesional IEC para ecualizadores profesionales abarcando las frecuencias de los 20Hz a los 20kHz con las siguientes frecuencias:
  
  # Frecuencias del ecualizador de 31 Bandas:
  20Hz, 25Hz, 31.5Hz, 40Hz, 50Hz, 63Hz, 80Hz, 100Hz, 125Hz, 160Hz, 200Hz, 250Hz, 315Hz, 400Hz, 500Hz, 630Hz, 800Hz, 1kHz, 1.25kHz, 1.6kHz, 2kHz, 2.5kHz, 3.15kHz, 4kHz, 5kHz, 6.3kHz, 8kHz, 10kHz, 12.5kHz, 16kHz, 20kHz.
  
  Este ecualizador de 31 bandas profesional de 1/3 de octava será el principal que podra ver y usar el usuario, usando los estandares IEC para ecualizadores de 1/3 de octava de espaciado, por lo que los deslizadores se moveran usando este estandard de 1/3 de octava y cada slider de este ecualizador de 31 bandas tendra un espacio entre cada slider de 10px, el ancho que ocupe todo este ecualizador debe estar centrado horizontalmente en base al ancho de la columna.
  
  Tambien vamos a usar un ecualizador de 20 bandas usando los estandares IEC para ecualizadores de 1/2 de octava de espaciado, este ecualizador cuando este activado y se mueva cada deslizador de frecuencia del ecualizador se movera usando los estandares IEC para ecualizadores de 20 bandas de 1/2 octava usando un factor Q constante de 2.87 para limitar la interacción entre bandas adyacentes, los deslizadores se moveran usando este estandar de 1/2 de octava, pero cada cambio que se haga tambien se aplicara al ecualizador de 31 bandas, asi que este ecualizador funcionara como una capa sobre el ecualizador de 31 bandas, ya que el usuario solo podra ver las bandas correspondientes el ecualizador de 20 bandas, pero en el fondo se estara moviendo el ecualizador de 31 bandas pero con el estandar IEC de 1/2 de octava para ecualizadores, asi que cuando el usuario despues active usar el ecualizador de 31 bandas podra ver los cambios que se hicieron en el ecualizador de 20 bandas y este ecualizador de 20 bandas abarcara las frecuencias de los 22.4Hz a los 16kHz con las siguientes frecuencias:
  
  # Frecuencias del ecualizador de 20 Bandas:
  22.4Hz, 31.5Hz, 45Hz, 63Hz, 90Hz, 125Hz, 180Hz, 250Hz, 355Hz, 500Hz, 710Hz, 1kHz, 1.4kHz, 2kHz, 2.8kHz, 4kHz, 5.6kHz, 8kHz, 11.2kHz, 16kHz.
  
  Y cada slider de este ecualizador de 20 bandas tendra un espacio entre cada slider de 20px, el ancho que ocupe todo este ecualizador debe estar centrado horizontalmente en base al ancho de la columna.
  
  Todos los numeros de las frecuencias de sliders del ecualizador se mostraran arriba de los sliders con un tamaño de tipografia un poco pequeño y sin las abreviaciones "Hz" ni "kHz", solo mostrando el numero de frecuencia abreviado asi: 20, 31.5, 90, 800, 1, 1.25, 2.5, 3.15, 8, etc. y el nivel en decibeles en el que se encuentra una frecuencia ira abajo del slider mostrando solo el numero sin la abreviacion de decibeles ni la palabra completa con un tamaño de tipografia un poco pequeño al igual que el de las frecuencias del equalizador y cuando el slider este en el nivel 0.0 has que solo se muestre el "0" y de igual forma sin abreviaciones de decibeles.
  
  Importante implementar tambien la siguiente funcion: cuando se de clic con el boton secundario del mouse sobre un slider ya sea del ecualizador o del pre-amplificador este slider se debe resetear y regresar a su posicion por defaul que es "0", asi que implementa esta funcion.
  
  Tambien, cuando el ecualizador este activado y algunos sliders de las frecuencias del ecualizador esten en "0", entonces esas frecuencias no se procesaran, apareceran como si estuvieran desactivados o apagados, asi que no se deben procesar esas frecuencias, solo se deben procesar las frecuencias que se haya ajustado el usuario o movido de su pocision por default que es "0", para asi ahorrar procesamiento en el procesador y sistema operativo.
  
  !MUY IMPORTANTE!: cuando se mueva un slider del pre-amplificador y del ecualizador no debe haber latencia, el cambio de los niveles de los decibeles se debe hacer de inmediato y se debe escuchar tambien de inmediato sin ninguna latencia.
  
  Agrega tambien un tootltip que siga el movimiento del puntero del mouse cuando se coloca sobre uno de los deslizadores ya sea el de pre-amplificador o ecualizador, que se muestre de forma inmediata sin latencia y debera mostrar el nivel en el que se encuentra el deslizador mostrando solo numeros sin abreviaciones de los decibeles y la informacion del tooltip se debe mostrar en horizontal sin hacer saltos de linea cuando se mueva hacia niveles negativos o mayores niveles y tambien cuando un slider este en el nivel 0.0 has que solo se muestre el "0" y de igual forma sin abreviaciones de decibeles.
  
Ademas, quiero que agregues una leyenda en la parte inferior izquierda fuera de las tablas del pre-amplificador y ecualizador y que diga "*Puede restablecer el valor por defecto haciendo clic derecho sobre un deslizador" usando el "color de texto secundario".

IMPORTANTE.
Puedes usar los crates que ya tenemos en nuestro stack o usar otros para que no haya latencia, ademas revisa si se puede usar el crate rustfft con los dems crrates que vamos a usar para optimizar y mejorar la estabilidad de nuestro ecualizador y lee, revisa y comprende toda la documentacion completa de todos los crates que vamos a usar para que estas funciones que vamos a implementar funcionen de la forma mas perfecta, segura y rapida en nuestro reproductor, te dejo todas las documentaciones de las ultimas versiones de algunos de los crates que vamos a usar para estas implementanciones, asi que revisalas por completo y analiza las nuevas funciones que tienen, sus nuevas optimizaciones y como usarlas de la forma correcta para aprovechar todas sus mejoras:
  cpal, version: 0,17,1 (DOCUMENTACION: https://docs.rs/cpal/latest/cpal/ )
  symphonia, version: 0.5.5 (DOCUMENTACION: https://docs.rs/symphonia/latest/symphonia/ )
  rubato, version, 1.0.1 (DOCUMENTACION: https://docs.rs/rubato/latest/rubato/ )
  audioadapter, version, 2.0.0 (DOCUMENTACION: https://docs.rs/audioadapter/latest/audioadapter/ )
  rustfft, version, 6.4.1 (DOCUMENTACION: https://docs.rs/rustfft/latest/rustfft/ )
  hrtf, version, 0.8.1 (DOCUMENTACION: https://docs.rs/hrtf/latest/hrtf/ )
  dasp , version, 0.11.0 (DOCUMENTACION: https://docs.rs/dasp/latest/dasp/ )

Tambien te voy a agregar el codigo de demo de ejemplo que tiene la propia documentacion de egui referente a los sliders para que puedas activar y configurar la funcion "Logarithmic" que es la que creo que sirve para que los sliders se muevan decimal por decimal de la forma en la que te lo explique anteriormente, revisa el codigo y la documentacion para corroborar que esa sea la funcion o alguna otra funcion que este en el codigo demo que proporciona la documentacion de egui directamente y que es el siguiente:

use egui::{Slider, SliderClamping, SliderOrientation, Ui, style::HandleShape};

/// Showcase sliders
#[derive(PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Sliders {
    pub min: f64,
    pub max: f64,
    pub logarithmic: bool,
    pub clamping: SliderClamping,
    pub smart_aim: bool,
    pub step: f64,
    pub use_steps: bool,
    pub integer: bool,
    pub vertical: bool,
    pub value: f64,
    pub trailing_fill: bool,
    pub handle_shape: HandleShape,
}

impl Default for Sliders {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 10000.0,
            logarithmic: true,
            clamping: SliderClamping::Always,
            smart_aim: true,
            step: 10.0,
            use_steps: false,
            integer: false,
            vertical: false,
            value: 10.0,
            trailing_fill: false,
            handle_shape: HandleShape::Circle,
        }
    }
}

impl crate::Demo for Sliders {
    fn name(&self) -> &'static str {
        "⬌ Sliders"
    }

    fn show(&mut self, ui: &mut egui::Ui, open: &mut bool) {
        egui::Window::new(self.name())
            .open(open)
            .resizable(false)
            .constrain_to(ui.available_rect_before_wrap())
            .show(ui, |ui| {
                use crate::View as _;
                self.ui(ui);
            });
    }
}

impl crate::View for Sliders {
    fn ui(&mut self, ui: &mut Ui) {
        let Self {
            min,
            max,
            logarithmic,
            clamping,
            smart_aim,
            step,
            use_steps,
            integer,
            vertical,
            value,
            trailing_fill,
            handle_shape,
        } = self;

        ui.label("You can click a slider value to edit it with the keyboard.");

        let (type_min, type_max) = if *integer {
            ((i32::MIN as f64), (i32::MAX as f64))
        } else if *logarithmic {
            (-f64::INFINITY, f64::INFINITY)
        } else {
            (-1e5, 1e5) // linear sliders make little sense with huge numbers
        };

        *min = min.clamp(type_min, type_max);
        *max = max.clamp(type_min, type_max);

        let orientation = if *vertical {
            SliderOrientation::Vertical
        } else {
            SliderOrientation::Horizontal
        };

        let istep = if *use_steps { *step } else { 0.0 };
        if *integer {
            let mut value_i32 = *value as i32;
            ui.add(
                Slider::new(&mut value_i32, (*min as i32)..=(*max as i32))
                    .logarithmic(*logarithmic)
                    .clamping(*clamping)
                    .smart_aim(*smart_aim)
                    .orientation(orientation)
                    .text("i32 demo slider")
                    .step_by(istep)
                    .trailing_fill(*trailing_fill)
                    .handle_shape(*handle_shape),
            );
            *value = value_i32 as f64;
        } else {
            ui.add(
                Slider::new(value, (*min)..=(*max))
                    .logarithmic(*logarithmic)
                    .clamping(*clamping)
                    .smart_aim(*smart_aim)
                    .orientation(orientation)
                    .text("f64 demo slider")
                    .step_by(istep)
                    .trailing_fill(*trailing_fill)
                    .handle_shape(*handle_shape),
            );

            ui.label(
                "Sliders will intelligently pick how many decimals to show. \
                You can always see the full precision value by hovering the value.",
            );

            if ui.button("Assign PI").clicked() {
                self.value = std::f64::consts::PI;
            }
        }

        ui.separator();

        ui.label("Slider range:");
        ui.add(
            Slider::new(min, type_min..=type_max)
                .logarithmic(true)
                .smart_aim(*smart_aim)
                .text("left")
                .trailing_fill(*trailing_fill)
                .handle_shape(*handle_shape),
        );
        ui.add(
            Slider::new(max, type_min..=type_max)
                .logarithmic(true)
                .smart_aim(*smart_aim)
                .text("right")
                .trailing_fill(*trailing_fill)
                .handle_shape(*handle_shape),
        );

        ui.separator();

        ui.checkbox(trailing_fill, "Toggle trailing color");
        ui.label("When enabled, trailing color will be painted up until the handle.");

        ui.separator();

        handle_shape.ui(ui);

        ui.separator();

        ui.checkbox(use_steps, "Use steps");
        ui.label("When enabled, the minimal value change would be restricted to a given step.");
        if *use_steps {
            ui.add(egui::DragValue::new(step).speed(1.0));
        }

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Slider type:");
            ui.radio_value(integer, true, "i32");
            ui.radio_value(integer, false, "f64");
        })
        .response
        .on_hover_text("All numeric types (f32, usize, …) are supported.");

        ui.horizontal(|ui| {
            ui.label("Slider orientation:");
            ui.radio_value(vertical, false, "Horizontal");
            ui.radio_value(vertical, true, "Vertical");
        });
        ui.add_space(8.0);

        ui.checkbox(logarithmic, "Logarithmic");
        ui.label("Logarithmic sliders are great for when you want to span a huge range, i.e. from zero to a million.");
        ui.label("Logarithmic sliders can include infinity and zero.");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Clamping:");
            ui.selectable_value(clamping, SliderClamping::Never, "Never");
            ui.selectable_value(clamping, SliderClamping::Edits, "Edits");
            ui.selectable_value(clamping, SliderClamping::Always, "Always");
        });
        ui.label("If true, the slider will clamp incoming and outgoing values to the given range.");
        ui.label("If false, the slider can show values outside its range, and you cannot enter new values outside the range.");
        ui.add_space(8.0);

        ui.checkbox(smart_aim, "Smart Aim");
        ui.label("Smart Aim will guide you towards round values when you drag the slider so you you are more likely to hit 250 than 247.23");
        ui.add_space(8.0);

        ui.vertical_centered(|ui| {
            egui::reset_button(ui, self, "Reset");
            ui.add(crate::egui_github_link_file!());
        });
    }
}


Tambien para cambiar el tamaño de los sliders para que cubran los 160px de alto o largo que te especifique anteriormente y si no encuentras una manera correcta de que modifiquen, aqui encontre un enlace en el que explican 2 soluciones con ejemplos de codigo que puedes verificar y ver si es compatible con nuestra version de egui o ver si la puedes adaptar para que los sliders tengan el tamaño que te pedi, este es el enlace del foro de discucion:

https://users.rust-lang.org/t/how-to-resize-egui-sliders-or-any-widgets-really/79823


Con todo esto ya puedes activar para que funcionen perfectamente las funciones anteriores que implementamos en la pestaña del ecualizador, que son activar el toggle switch de activar/desactivar ecualizador, activa tambien la funcion de que bandas de equalizador mostrar 20 bands o 31 bandas, tambien termina de activar las funciones en el apartado de Presets para que se guarden, importen y exporten ahora si los ajustes reales que se tengan en el ecualizador y pre-amplificador y tambien que funcione correctamente el boton de reset para que restablesca todos los valores del ecualizador y pre-amplificador a "0."  
  

 

en la pestaña de ecualziador se mostrara un gráfico de curva en la parte superior y deslizadores verticales (faders) en la parte inferior para cada frecuencia. Incluye un deslizador de preamplificación a la izquierda. Los niveles activos se iluminan en el color de acento del reproductor. Una barra inferior para mostrar botones y desplegables para resetear el ecualizador, guardar y cargar presets, tambien desplegable para elegir la cantidad de bandas del ecualizador para configurar (se debe poder elegir entre 10, 15, 18, 20, 25 y 30 bandas). 



### Funcionalidad de Ventanas (Docking/Undocking):
El sistema debe permitir que el "Módulo 1 (Reproductor)" y el "Módulo 2 (Listas de Reproduccion)",el "Módulo 3 (Filtros de la Biblioteca)" y el "Modulo 4 (Biblioteca)" funcionen unidos como una sola ventana grande (como se ve en la imagen donde están integrados) o separarse. Al separarse, cada ventana mantiene sus controles de ventana propios (cerrar, maximizar/minimizar) en la esquina superior derecha. Y cuando estan acoplados que estos botones de control de ventanas solo aparescan en el modulo principal "Módulo 1 (Reproductor)".
Tambien se debe poder elegir siempre que paneles mostrar y ocultar, seleccionandolos desde el menu principal que estara ubicado en el Módulo 1 (Reproductor)" por medio del icono de menu principal ubicado en la parte superior izquierda dentro de este modulo, tal como ya se habia descrito anteriormente.

### Detalles Adicionales de UI:

Las barras de desplazamiento (sliders) deben ser finas y del "color de contraste" del reproductor.

Los menús contextuales y botones deberan mantener el estilo y colores principales o personalizados y configurados previamente, al pasar el mouse (hover), el borde o fondo se torna en el color de acento seleccionado.
