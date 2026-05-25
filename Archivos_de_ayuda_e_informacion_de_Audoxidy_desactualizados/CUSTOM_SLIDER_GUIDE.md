# Guía de Uso: Widget Global CustomSlider

## Descripción General

`CustomSlider` es un widget personalizado universal de Audoxidy ubicado en `src/gui/widgets.rs`. Proporciona un slider reutilizable con soporte para múltiples orientaciones, interacciones avanzadas y una API limpia basada en el patrón builder.

## Características Principales

- ✓ **Orientaciones Dual**: Vertical y Horizontal
- ✓ **Reset por Clic Secundario**: Restaura el valor por defecto
- ✓ **Arrastre Continuo Mejorado**: Movimiento suave del mouse
- ✓ **Navegación por Teclado**: Flechas arriba/abajo/izquierda/derecha
- ✓ **Input de Texto**: Edición directa de valores (opcional)
- ✓ **Track Coloreado**: Visualización visual hasta la posición del handle
- ✓ **Tooltip Dinámico**: Muestra valor actual con formato personalizable
- ✓ **Rendimiento Optimizado**: Cálculos eficientes, bajo overhead

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
.with_colored_track(true)  // Activa visualización del progreso
.width(Length::Fill)
.height(Length::Fixed(20.0))
.format_value(|v| format!("{}%", v as i32));  // Formato personalizado
```

### Slider con Todas las Opciones

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
.with_colored_track(true)        // Mostrar progreso
.with_arrow_keys(true)           // Permitir flechas
.with_keyboard_input(false)      // No mostrar input (por defecto)
.format_value(|v| format!("{:.1} dB", v))  // Formato en dB
.track_width(8.0)                // Ancho de la pista
.handle_size(16.0);              // Tamaño del controlador
```

## Enumeraciones y Tipos

### SliderOrientation

```rust
pub enum SliderOrientation {
    Vertical,    // Desplazamiento vertical (arriba/abajo)
    Horizontal,  // Desplazamiento horizontal (izquierda/derecha)
}
```

### CustomSliderOptions

```rust
pub struct CustomSliderOptions {
    pub enable_colored_track: bool,  // Colorear track hasta handle
    pub enable_keyboard_input: bool, // Campo de entrada de texto
    pub enable_arrow_keys: bool,     // Navegar con flechas
    pub show_tooltip: bool,          // Mostrar tooltip en hover
    pub step_size: f32,              // Incremento por pulsación de flecha
}
```

## Métodos del Builder

### Dimensiones

```rust
.width(Length::Fixed(24.0))      // Ancho fijo
.height(Length::Fixed(240.0))    // Alto fijo
```

### Orientación y Aparencia

```rust
.orientation(SliderOrientation::Vertical)   // Orientación
.track_width(8.0)                           // Ancho de la pista
.handle_size(16.0)                          // Tamaño del handle
```

### Funcionalidades

```rust
.with_colored_track(true)       // Habilitar track coloreado
.with_arrow_keys(true)          // Permitir navegación con flechas
.with_keyboard_input(true)      // Permitir edición por texto
```

### Formato Personalizado

```rust
.format_value(|v| format!("{:.1} dB", v))  // Para ecualizador
.format_value(|v| format!("{}%", v as i32)) // Para volumen
.format_value(|v| format!("{}ms", v as i32)) // Para delays
```

## Integración en Mensajes

### Definir Mensajes de Control

```rust
pub enum Message {
    // Slider messages
    SliderChanged(f32),           // Cuando el slider cambia
    SliderReset,                  // Cuando se hace clic derecho
    
    // Específico para ecualizador
    EqBandChanged(usize, f32),   // Banda y valor
    EqBandReset(usize),          // Reset de una banda
}
```

### Manejar en Update

```rust
match message {
    Message::SliderChanged(value) => {
        // Actualizar estado
        self.slider_value = value;
        // Aplicar cambio a backend
        audio_manager.set_value(value);
    }
    Message::SliderReset => {
        self.slider_value = 0.0;  // O valor por defecto
        audio_manager.reset();
    }
}
```

## Casos de Uso Reales

### 1. Control de Volumen

```rust
CustomSlider::new(
    current_volume,
    0.0..=1.0,
    |v| Message::VolumeChanged(v),
    || Message::VolumeReset,
)
.orientation(SliderOrientation::Horizontal)
.with_colored_track(true)
.format_value(|v| format!("{:.0}%", v * 100.0))
.width(Length::Fill)
.height(Length::Fixed(16.0))
```

### 2. Control de Ecualizador (Vertical)

```rust
CustomSlider::new(
    eq_band_value,
    -12.0..=12.0,
    move |v| Message::EqBandChanged(band_index, v),
    move || Message::EqBandChanged(band_index, 0.0),
)
.orientation(SliderOrientation::Vertical)
.with_colored_track(true)
.format_value(|v| format!("{:.1} dB", v))
.width(Length::Fixed(24.0))
.height(Length::Fixed(240.0))
```

### 3. Control de Balance Estéreo

```rust
CustomSlider::new(
    balance_value,
    -1.0..=1.0,
    |v| Message::BalanceChanged(v),
    || Message::BalanceReset,
)
.orientation(SliderOrientation::Horizontal)
.with_colored_track(false)  // No se necesita visualización
.format_value(|v| {
    if v > 0.0 { format!("R {:.0}%", v * 100.0) }
    else if v < 0.0 { format!("L {:.0}%", -v * 100.0) }
    else { "Center".to_string() }
})
.width(Length::Fill)
.height(Length::Fixed(20.0))
```

## Optimizaciones de Rendimiento

### Consideraciones

1. **Format Function**: Si no proporcionas `.format_value()`, usa el formato por defecto `{:.1}`
2. **Track Coloreado**: Habilítalo solo cuando sea visualmente necesario
3. **Arrow Keys**: Desactívalo si no necesitas navegación por teclado
4. **Step Size**: Por defecto es 0.1, ajusta según tu rango

### Ejemplo de Optimización

```rust
// Bien: Solo enable lo que necesitas
CustomSlider::new(value, range, on_change, on_reset)
    .with_colored_track(true)  // Solo esto
    .height(Length::Fixed(240.0))

// Evita: Habilitar todo innecesariamente
CustomSlider::new(value, range, on_change, on_reset)
    .with_colored_track(true)
    .with_arrow_keys(true)      // Si no lo usarás
    .with_keyboard_input(true)  // Si no lo usarás
```

## Comportamiento de Entrada

### Mouse

- **Clic Izquierdo**: Cambia el valor según la posición del cursor
- **Arrastre Izquierdo**: Movimiento continuo y suave
- **Hover**: Muestra tooltip con valor actual
- **Clic Derecho**: Reset al valor por defecto

### Teclado (si `.with_arrow_keys(true)`)

- **Arriba/Derecha**: Incrementa el valor
- **Abajo/Izquierda**: Decrementa el valor
- El incremento es de `step_size` (por defecto 0.1)

### Texto (si `.with_keyboard_input(true)`)

- Campo de entrada editable
- Validación automática del rango
- Conversión automática de string a f32

## Troubleshooting

### El slider no responde al mouse

- Verifica que `.width()` y `.height()` sean > 0
- Asegúrate de que los bounds del widget no estén ocultos

### El tooltip no aparece

- Verifica que `.show_tooltip` esté habilitado (por defecto sí)
- Asegúrate de pasar el mouse sobre el slider

### Arrow keys no funcionan

- Usa `.with_arrow_keys(true)` para habilitarlas
- El slider debe tener el foco de la ventana

### Performance lenta

- Reduce el número de sliders en pantalla
- Desactiva track coloreado si no es necesario
- Usa `.format_value()` con formateo simple

## Referencia Completa de Métodos

| Método | Parámetro | Defecto | Descripción |
|--------|-----------|---------|------------|
| `new()` | (value, range, on_change, on_reset) | - | Crear nuevo slider |
| `width()` | Length | Fixed(20.0) | Ancho del widget |
| `height()` | Length | Fill | Alto del widget |
| `orientation()` | SliderOrientation | Vertical | Orientación |
| `with_colored_track()` | bool | false | Colorear track |
| `with_arrow_keys()` | bool | false | Permitir flechas |
| `with_keyboard_input()` | bool | false | Permitir texto |
| `format_value()` | Fn(f32)->String | "{:.1}" | Formato del tooltip |
| `track_width()` | f32 | 8.0 | Ancho de pista |
| `handle_size()` | f32 | 16.0 | Tamaño del handle |

## Notas Importantes

1. **Colores**: Usa las constantes del tema (COLOR_ACCENT, COLOR_TEXT_PRIMARY, etc.)
2. **Rangos**: Siempre especifica un rango válido (start < end)
3. **Callbacks**: Deben retornar el Message deseado
4. **Diseño**: Mantén coherencia con otros sliders del proyecto
5. **Localización**: Usa variables de idioma en format_value

## Véase También

- `src/gui/widgets.rs` - Implementación del widget
- `src/gui/audio_center.rs` - Ejemplo de uso en ecualizador
- `src/gui/theme.rs` - Colores y temas disponibles
