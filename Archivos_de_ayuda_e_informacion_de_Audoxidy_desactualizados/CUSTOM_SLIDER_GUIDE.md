# Guía de Uso: Widget Global CustomSlider

## Descripción General

`CustomSlider` es un widget personalizado universal de Audoxidy ubicado en `src/gui/widgets.rs`. Proporciona un slider reutilizable con soporte para múltiples orientaciones, interacciones avanzadas, colores configurables y una API limpia basada en el patrón builder.

## Características Principales

- ✓ **Orientaciones Dual**: Vertical y Horizontal
- ✓ **Reset por Clic Secundario**: Restaura el valor por defecto
- ✓ **Arrastre Continuo Mejorado**: Movimiento suave del mouse
- ✓ **Navegación por Teclado**: Flechas arriba/abajo/izquierda/derecha
- ✓ **Input de Texto Avanzado**: Edición directa de valores con cursor navegable (posicionable con clic y flechas)
- ✓ **Track Coloreado**: Visualización visual hasta la posición del handle
- ✓ **Tooltip Dinámico**: Muestra valor actual junto al cursor
- ✓ **Estados Separados**: Focus (mouse over, visual) y Hover (clic activo, visual + teclado) con colores independientes
- ✓ **Snap a Grid**: Alineación automática al step_size al soltar el arrastre
- ✓ **AppFocus Routing**: Prioridad de teclado entre módulos (AudioCenter vs Library vs Playlist)
- ✓ **Colores Configurables**: Track, handle normal, focus, hover, bordes — todo personalizable
- ✓ **Rendimiento Optimizado**: Cálculos eficientes, sin re-procesamiento extra

## Estados del Slider

El slider tiene 3 estados visuales independientes:

| Estado | Activación | Visual | Teclado |
|--------|-----------|--------|---------|
| **Normal** | Por defecto | `handle_color` | No |
| **Focus** | Mouse sobre el slider (sin clic) | `handle_focus_color` | No |
| **Hover** | Clic en el slider | `handle_hover_color` + borde | Sí |

- **Focus** es puramente visual. Se activa al pasar el mouse, se pierde al salir.
- **Hover** requiere clic. Activa teclado. Se pierde al hacer clic en otro slider o espacio vacío.
- El borde en hover es **inset** (interno): el handle se reduce para mostrar el borde por dentro.

## Ejemplo de Uso Básico

### Slider Vertical Simple

```rust
use crate::gui::widgets::{CustomSlider, SliderOrientation};

let slider = CustomSlider::new(
    5.0,                            // Valor inicial
    0.0..=10.0,                     // Rango de valores
    |v| Message::SliderChanged(v),  // Callback en cambio
    || Message::SliderReset,        // Callback en clic derecho
)
.orientation(SliderOrientation::Vertical)
.width(Length::Fixed(24.0))
.height(Length::Fixed(240.0));
```

### Slider Horizontal con Track Coloreado

```rust
let slider = CustomSlider::new(
    50.0,
    0.0..=100.0,
    |v| Message::VolumeChanged(v),
    || Message::VolumeMuted,
)
.orientation(SliderOrientation::Horizontal)
.with_colored_track(true)
.width(Length::Fill)
.height(Length::Fixed(20.0))
.format_value(|v| format!("{}%", v as i32));
```

### Slider con Estados Focus/Hover Personalizados

```rust
let slider = CustomSlider::new(
    -3.0,
    -12.0..=12.0,
    |v| Message::EqBandChanged(v),
    || Message::EqBandReset,
)
.orientation(SliderOrientation::Vertical)
.width(Length::Fixed(24.0))
.height(Length::Fixed(240.0))
// Track
.track_color(Color::from_rgb(0.2, 0.2, 0.2))
.active_track_color(Color::from_rgb(0.0, 0.8, 0.0))
.with_colored_track(true)
// Handle - Normal
.handle_color(Color::from_rgb(0.0, 0.6, 0.0))
// Handle - Focus (mouse over, visual)
.handle_focus_color(Color::from_rgb(0.0, 1.0, 0.0))
// Handle - Hover (clic activo, visual + teclado)
.handle_hover_color(Color::from_rgb(0.2, 0.8, 0.2))
.border_hover(2.0, Color::from_rgb(0.0, 0.6, 0.0))
// Tooltip
.show_tooltip(true)
.format_value(|v| format!("{:.1} dB", v));
```

### Slider con Input de Texto (pestaña efectos)

```rust
let slider = CustomSlider::new(
    val,
    range,
    on_change,
    on_reset,
)
.orientation(SliderOrientation::Horizontal)
.with_arrow_keys(true)
.with_keyboard_input(true)
.input_width_fixed(40.0)
.input_height_fixed(18.0)
.input_align(InputAlign::Center)
.on_hover_state_change(|active| Message::AudioCenterMsg(
    AudioCenterMessage::SliderHoverActive(active)
));
```

## Enumeraciones y Tipos

### SliderOrientation

```rust
pub enum SliderOrientation {
    Vertical,
    Horizontal,
}
```

### InputPosition

```rust
pub enum InputPosition {
    Auto,    // Horizontal→Right, Vertical→Bottom
    Top,
    Bottom,
    Left,
    Right,
}
```

### InputWidth

```rust
pub enum InputWidth {
    Auto,           // Ancho calculado según contenido
    Fixed(f32),     // Ancho fijo en píxeles
}
```

### InputAlign

```rust
pub enum InputAlign {
    Left,
    Center,
    Right,
}
```

### CustomSliderOptions (Estructura Completa)

```rust
pub struct CustomSliderOptions {
    // Track
    pub enable_colored_track: bool,
    pub track_color: Option<Color>,
    pub active_track_color: Option<Color>,

    // Handle - Normal
    pub handle_color: Option<Color>,

    // Handle - Focus (mouse sobre slider, sin clic)
    pub handle_focus_color: Option<Color>,

    // Handle - Hover (clic activo en slider)
    pub handle_hover_color: Option<Color>,

    // Borde normal
    pub border_color: Option<Color>,
    pub border_width: f32,

    // Borde focus (mouse sobre slider, sin clic)
    pub border_focus_color: Option<Color>,
    pub border_focus_width: f32,

    // Borde hover (clic activo en slider)
    pub border_hover_color: Option<Color>,
    pub border_hover_width: f32,

    // Tooltip
    pub show_tooltip: bool,
    pub tooltip_font_size: f32,

    // Interacción
    pub enable_keyboard_input: bool,
    pub enable_arrow_keys: bool,
    pub step_size: f32,

    // Input de teclado
    pub input_position: InputPosition,
    pub input_gap: f32,
    pub input_width: InputWidth,
    pub input_bg_color: Option<Color>,
    pub input_border_color: Option<Color>,
    pub input_border_focus_color: Option<Color>,
    pub input_border_width: f32,
    pub input_border_radius: f32,
    pub input_font_size: f32,
    pub input_font_color: Option<Color>,
    pub input_align: InputAlign,
    pub input_padding_h: f32,
    pub input_padding_v: f32,
    pub input_fixed_height: Option<f32>,
}
```

## Métodos del Builder

### Dimensiones

```rust
.width(Length::Fixed(24.0))      // Ancho
.height(Length::Fixed(240.0))    // Alto
```

### Orientación y Apariencia

```rust
.orientation(SliderOrientation::Vertical)   // Orientación
.track_width(f32)                           // Ancho de pista (defecto: 8.0)
.handle_size(f32)                           // Tamaño del handle (defecto: 16.0)
```

### Colores del Track

```rust
.track_color(Color)                // Fondo del track (None = COLOR_CONTRAST)
.active_track_color(Color)         // Progreso (None = COLOR_ACCENT)
```

### Colores del Handle por Estado

```rust
// Normal (ninguna interacción)
.handle_color(Color)               // None = COLOR_ACCENT

// Focus (mouse sobre slider, sin clic)
.handle_focus_color(Color)         // None = COLOR_TEXT_PRIMARY

// Hover (clic activo, teclado habilitado)
.handle_hover_color(Color)         // None = COLOR_TEXT_SECONDARY
```

### Bordes por Estado

```rust
// Borde normal
.border(f32, Color)                // width + color (None = transparente, 0.0)

// Borde focus (mouse sobre slider)
.border_focus(f32, Color)          // width + color

// Borde hover (clic activo)
.border_hover(f32, Color)          // width + color
```

### Tooltip

```rust
.show_tooltip(bool)                // Habilitar tooltip (defecto: false)
.tooltip_font_size(f32)            // Tamaño fuente tooltip (defecto: 13.0)
```

### Funcionalidades

```rust
.with_colored_track(true)          // Track coloreado hasta handle
.with_arrow_keys(true)             // Navegación con flechas
.with_keyboard_input(true)         // Input de texto visible junto al slider
```

### Input de Texto (cuando with_keyboard_input = true)

```rust
// Tamaño y posición
.input_width_fixed(40.0)           // Ancho fijo en píxeles
.input_height_fixed(18.0)          // Alto fijo en píxeles
.input_position(InputPosition::Right) // Posición relativa al slider
.input_gap(10.0)                   // Espacio entre slider e input

// Alineación del texto
.input_align(InputAlign::Center)   // Left | Center | Right

// Estilo (colores y bordes)
.input_style(                       // Todos los estilos en un solo builder
    bg, border, border_focus,
    border_width, border_radius,
    font_size, font_color,
)
```

### Callback de Hover

```rust
.on_hover_state_change(|active| {
    // active = true: slider recibió clic (hover activado, teclado listo)
    // active = false: slider perdió hover (clic fuera)
    Message::MyMessage(active)
})
```

### Formato Personalizado

```rust
.format_value(|v| format!("{:.1} dB", v))
.format_value(|v| format!("{}%", v as i32))
```

## Referencia Completa de Métodos

| Método | Parámetro | Defecto | Descripción |
|--------|-----------|---------|-------------|
| `new()` | (value, range, on_change, on_reset) | - | Crear nuevo slider |
| `width()` | Length | Fixed(20.0) | Ancho del widget |
| `height()` | Length | Fill | Alto del widget |
| `orientation()` | SliderOrientation | Vertical | Orientación |
| `track_width()` | f32 | 8.0 | Ancho de la pista |
| `handle_size()` | f32 | 16.0 | Tamaño del handle |
| `with_colored_track()` | bool | false | Colorear track |
| `track_color()` | Color | COLOR_CONTRAST | Fondo del track |
| `active_track_color()` | Color | COLOR_ACCENT | Track activo |
| `handle_color()` | Color | COLOR_ACCENT | Handle normal |
| `handle_focus_color()` | Color | COLOR_TEXT_PRIMARY | Handle en focus (mouse over) |
| `handle_hover_color()` | Color | COLOR_TEXT_SECONDARY | Handle en hover (clic activo) |
| `border()` | (f32, Color) | (0.0, transparent) | Borde normal |
| `border_focus()` | (f32, Color) | (0.0, transparent) | Borde focus |
| `border_hover()` | (f32, Color) | (0.0, COLOR_ACCENT) | Borde hover (inset) |
| `show_tooltip()` | bool | false | Tooltip en hover/drag |
| `tooltip_font_size()` | f32 | 13.0 | Fuente del tooltip |
| `with_arrow_keys()` | bool | false | Flechas de teclado |
| `with_keyboard_input()` | bool | false | Input visible junto al slider |
| `input_position()` | InputPosition | Auto | Posición del input |
| `input_gap()` | f32 | 10.0 | Gap slider-input |
| `input_width_fixed()` | f32 | Auto | Ancho fijo del input |
| `input_height_fixed()` | f32 | font+padding | Alto fijo del input |
| `input_align()` | InputAlign | Left | Alineación texto en input |
| `input_style()` | (bg, border, focus, w, r, sz, fc) | ver tabla | Estilo completo input |
| `on_hover_state_change()` | `Fn(bool)->Message` | None | Callback hover activo |
| `format_value()` | `Fn(f32)->String` | `{:.1}` | Formato del valor |
| `options()` | CustomSliderOptions | default | Setear opciones directo |

## Valores por Defecto del Input

| Propiedad | Default |
|-----------|---------|
| `input_bg_color` | COLOR_BG (#000000) |
| `input_border_color` | COLOR_TEXT_SECONDARY (#5B5B5B) |
| `input_border_focus_color` | COLOR_ACCENT (#FF003D, solo al escribir) |
| `input_border_width` | 1.0 px |
| `input_border_radius` | 4.0 px |
| `input_font_size` | 11.0 px |
| `input_font_color` | COLOR_TEXT_PRIMARY (#AFAFAF) |
| `input_padding_h` | 4.0 px |
| `input_padding_v` | 2.0 px |
| `input_align` | Left |
| `input_position` | Auto (Right horizontal, Bottom vertical) |
| `input_gap` | 10.0 px |
| `input_width` | Auto (calculado del contenido) |
| `input_fixed_height` | None (calculado de font + padding) |

## Comportamiento de Entrada

### Mouse
- **Clic Izquierdo en slider**: Activa hover + arrastre continuo
- **Clic Izquierdo en input**: Activa edición de texto con cursor posicionable
- **Arrastre**: Movimiento continuo, snap a grid al soltar
- **Clic Derecho**: Reset al valor por defecto

### Teclado (si `.with_arrow_keys(true)`)
- **Arriba/Abajo** (Vertical) o **Derecha/Izquierda** (Horizontal): Cambian valor
- **Flecha dirección incorrecta**: Ignorada (no pierde foco)
- **Otra tecla**: Desactiva foco de teclado del slider

### Input de Texto (si `.with_keyboard_input(true)`)
- **Clic en input**: Activa edición, cursor en posición del clic
- **Enter**: Confirma valor (válido) o rechaza (inválido)
- **Escape**: Cancela edición
- **Flechas izquierda/derecha**: Mueven cursor entre dígitos
- **Backspace**: Elimina dígito antes del cursor
- **Caracteres**: Se insertan en posición del cursor

### AppFocus Routing
- Al hacer clic en un slider: `AppFocus::AudioCenter` → flechas no afectan library/playlist
- Al hacer clic en espacio vacío o en otro slider: se restaura focus anterior
- Al hacer clic en input: hover se desactiva pero AppFocus permanece AudioCenter hasta Enter/Escape

## Snap a Grid

Siempre activo, sin flag. Al soltar el arrastre, el valor se redondea al múltiplo de `step_size` más cercano:

| step_size | Snap a |
|-----------|--------|
| 0.1 | 0.1 más cercano |
| 0.01 | 0.01 más cercano |
| 1.0 | Entero más cercano |

## Troubleshooting

### El slider no responde al mouse
- Verifica que `.width()` y `.height()` sean > 0
- Asegúrate de que los bounds del widget no estén ocultos

### Arrow keys no funcionan
- Usa `.with_arrow_keys(true)` para habilitarlas
- Debes hacer **clic en el slider** primero (hover visual no activa teclado)
- El focus visual (mouse over) NO activa teclado, solo el hover (clic)

### Input de texto no aparece
- Usa `.with_keyboard_input(true)` para habilitarlo
- El input aparece junto al slider según `input_position` (defecto: derecha para horizontal, abajo para vertical)
- Si el slider usa `Length::Fill`, el layout automáticamente reserva espacio para el input

### El input se sale de la tarjeta
- Al usar `input_position` Right/Left con slider Fill, el layout reserva espacio automáticamente
- Para inputs laterales en sliders con Fill, usa `input_width_fixed()` para controlar el ancho

### Performance lenta en input
- No uses `shell.capture_event()` — causa re-procesamiento completo del árbol de widgets
- El routing vía AppFocus es más eficiente

## Notas Importantes

1. **Focus ≠ Hover**: Son estados separados. Focus es visual (mouse over). Hover requiere clic y activa teclado.
2. **Track sin borde**: `border()`, `border_focus()`, `border_hover()` solo afectan al handle, NO al track.
3. **Callback hover**: `on_hover_state_change` se dispara `true` al recibir clic y `false` al perder hover.
4. **Colores por defecto**: Si no se especifican, usan constantes del tema.
5. **Borde inset**: En hover activo, el borde se dibuja DENTRO del handle.
6. **Layout automático**: Cuando `enable_keyboard_input = true` y el input está a la derecha/izquierda, el layout reduce el ancho del slider para dejar espacio al input.
7. **Snap nativo**: Todos los sliders tienen snap a grid siempre activo.

## Véase También

- `src/gui/widgets.rs` — Implementación del widget
- `src/gui/audio_center.rs` — Ejemplo de uso en ecualizador y efectos
- `src/gui/app.rs` — AppFocus y routing de teclado
- `src/gui/theme.rs` — Colores y temas disponibles
