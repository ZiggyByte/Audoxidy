# Detalles Técnicos - Cambios a CustomSlider Widget

## Archivo: src/gui/widgets.rs

### Sección 1: CustomSliderOptions - Agregar tooltip_font_size

**Líneas: ~1695-1705**

```rust
pub struct CustomSliderOptions {
    pub enable_colored_track: bool,
    pub enable_keyboard_input: bool,
    pub enable_arrow_keys: bool,
    pub show_tooltip: bool,
    pub step_size: f32,
    pub tooltip_font_size: f32,  // ← NUEVO CAMPO
}

impl Default for CustomSliderOptions {
    fn default() -> Self {
        Self {
            enable_colored_track: false,
            enable_keyboard_input: false,
            enable_arrow_keys: false,
            show_tooltip: true,
            step_size: 0.1,
            tooltip_font_size: 13.0,  // ← NUEVO DEFAULT
        }
    }
}
```

**Cambio:** Se agregó campo `tooltip_font_size: f32` con default 13.0 para permitir configuración de tamaño de fuente del tooltip.

---

### Sección 2: CustomSliderState - Nuevo Struct para State

**Líneas: ~1707-1710**

```rust
/// Estado interno del slider (mantiene si está siendo arrastrado)
#[derive(Debug, Clone, Default)]
struct CustomSliderState {
    is_dragging: bool,
}
```

**Cambio:** Nuevo struct para mantener estado persistente del widget. Usado con el Tree de Iced para tracking del arrastre del mouse.

---

### Sección 3: CustomSlider struct - SIN CAMBIOS

El struct CustomSlider<'a, Message> permanece igual, solo se agregó el state de CustomSliderState en la implementación.

---

### Sección 4: Builder Methods - Agregar show_tooltip y tooltip_font_size

**Líneas: ~1790-1810 (En el impl block de métodos builder)**

```rust
/// Habilitar o deshabilitar navegación con teclado (flechas)
pub fn with_arrow_keys(mut self, enabled: bool) -> Self {
    self.options.enable_arrow_keys = enabled;
    self
}

/// Habilitar o deshabilitar input de texto
pub fn with_keyboard_input(mut self, enabled: bool) -> Self {
    self.options.enable_keyboard_input = enabled;
    self
}

/// Establecer función de formato personalizado para el tooltip
pub fn format_value(mut self, f: impl Fn(f32) -> String + 'a) -> Self {
    self.format_fn = Some(Box::new(f));
    self
}

/// Habilitar o deshabilitar tooltip
pub fn show_tooltip(mut self, enabled: bool) -> Self {
    self.options.show_tooltip = enabled;
    self
}

/// Establecer tamaño de fuente del tooltip
pub fn tooltip_font_size(mut self, size: f32) -> Self {
    self.options.tooltip_font_size = size;
    self
}
```

**Cambios:**
- Agregado método `show_tooltip()`
- Agregado método `tooltip_font_size()`
- Ambos siguen el patrón builder para permitir encadenamiento

---

### Sección 5: Widget Trait Implementation - Agregar state()

**Líneas: ~1843-1855**

```rust
impl<'a, Message: 'a> iced::advanced::Widget<Message, Theme, iced::Renderer> for CustomSlider<'a, Message> {
    fn size(&self) -> iced::Size<Length> {
        iced::Size {
            width: self.width,
            height: self.height,
        }
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(CustomSliderState::default())
    }

    fn layout(
        &mut self,
        _tree: &mut iced::advanced::widget::Tree,
        _renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let size = limits.resolve(self.width, self.height, iced::Size::ZERO);
        iced::advanced::layout::Node::new(size)
    }
```

**Cambio:** Agregado método `state()` para inicializar el tree state con CustomSliderState. Esto permite mantener estado persistente entre renders.

---

### Sección 6: update() Method - Reescrito Completamente

**Líneas: ~1861-1964**

**Antes (Incorrecto):**
```rust
fn update(&mut self, _tree: &mut ..., event: &Event, ...) {
    // - No usa tree para state
    // - No diferencia entre click y drag
    // - Keyboard afecta todos los sliders
}
```

**Después (Correcto):**
```rust
fn update(
    &mut self,
    tree: &mut iced::advanced::widget::Tree,
    event: &iced::Event,
    layout: iced::advanced::Layout<'_>,
    cursor: iced::advanced::mouse::Cursor,
    _renderer: &iced::Renderer,
    _clipboard: &mut dyn iced::advanced::Clipboard,
    shell: &mut iced::advanced::Shell<'_, Message>,
    _viewport: &iced::Rectangle,
) {
    let bounds = layout.bounds();
    let Some(cursor_pos) = cursor.position() else {
        tree.state.downcast_mut::<CustomSliderState>().is_dragging = false;
        return;
    };

    let is_hovered = bounds.contains(cursor_pos);
    let state = tree.state.downcast_mut::<CustomSliderState>();

    match event {
        // ButtonPressed: Iniciar arrastre
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
            if is_hovered {
                state.is_dragging = true;  // ← NUEVO: Set dragging flag
                // ... actualizar valor ...
            }
        }

        // ButtonReleased: Detener arrastre
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
            state.is_dragging = false;  // ← NUEVO: Clear dragging flag
        }

        // CursorMoved: Solo actualizar si está siendo arrastrado
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. }) => {
            if state.is_dragging && bounds.contains(cursor_pos) {  // ← NUEVO: Check is_dragging
                // ... actualizar valor ...
            }
        }

        // Keyboard: SOLO SI ESTÁ HOVERED
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. })
            if self.options.enable_arrow_keys && is_hovered =>  // ← NUEVO: && is_hovered
        {
            // ... procesar flechas ...
        }

        _ => {}
    }
}
```

**Cambios Principales:**
1. **Drag State Tracking:** Ahora usa `tree.state` para tracking de `is_dragging`
2. **ButtonReleased Handler:** Nuevo evento para detener arrastre
3. **CursorMoved Logic:** Solo actualiza si `state.is_dragging == true`
4. **Keyboard Scoping:** Agregada condición `&& is_hovered` al pattern match

---

### Sección 7: draw() Method - Pasar cursor_pos a draw_tooltip()

**Líneas: ~1966-2004**

```rust
fn draw(
    &self,
    _tree: &iced::advanced::widget::Tree,
    renderer: &mut iced::Renderer,
    _theme: &Theme,
    _style: &iced::advanced::renderer::Style,
    layout: iced::advanced::Layout<'_>,
    cursor: iced::advanced::mouse::Cursor,  // ← Se usa ahora
    _viewport: &iced::Rectangle,
) {
    let bounds = layout.bounds();
    let is_hovered = cursor.position().map(|p| bounds.contains(p)).unwrap_or(false);
    let percent = self.calculate_percent();

    match self.orientation {
        SliderOrientation::Vertical => {
            self.draw_vertical_slider(renderer, bounds, is_hovered, percent);
        }
        SliderOrientation::Horizontal => {
            self.draw_horizontal_slider(renderer, bounds, is_hovered, percent);
        }
    }

    // Renderizar tooltip si está habilitado y el slider está enfocado
    if is_hovered && self.options.show_tooltip {
        if let Some(cursor_pos) = cursor.position() {
            self.draw_tooltip(renderer, bounds, percent, cursor_pos);  // ← NUEVO: cursor_pos
        }
    }
}
```

**Cambio:** Se extrae `cursor_pos` de `cursor.position()` y se pasa a `draw_tooltip()` para posicionamiento relativo al cursor.

---

### Sección 8: draw_tooltip() Method - Reescrito Completamente

**Líneas: ~2180-2231**

**Antes (Incorrecto):**
```rust
fn draw_tooltip(&self, renderer, bounds, percent) {
    // - Posición basada en slider bounds (no cursor)
    // - Colores incorrectos
    // - Font size hardcodeado en 10.0
    // - Sin borde
}
```

**Después (Correcto):**
```rust
/// Renderizar tooltip con el valor actual junto al cursor
fn draw_tooltip(&self, renderer: &mut iced::Renderer, _bounds: Rectangle, _percent: f32, cursor_pos: iced::Point) {
    use iced::advanced::Renderer as _;
    use iced::advanced::text::Renderer as _;

    let val_display = self.format_display_value(self.value);

    // Calcular tamaño del tooltip basado en el contenido
    let tooltip_padding = 8.0;
    let tooltip_height = self.options.tooltip_font_size + tooltip_padding * 2.0;
    let tooltip_width = 80.0;

    // Posicionar tooltip junto al cursor (offset para que no cubra el cursor)
    let tooltip_x = cursor_pos.x + 10.0;
    let tooltip_y = cursor_pos.y - tooltip_height - 5.0;

    let tooltip_rect = Rectangle {
        x: tooltip_x,
        y: tooltip_y,
        width: tooltip_width,
        height: tooltip_height,
    };

    // Dibujar fondo del tooltip (COLOR_CONTRAST)
    renderer.fill_quad(
        iced::advanced::graphics::core::renderer::Quad {
            bounds: tooltip_rect,
            border: iced::Border {
                radius: 4.0.into(),
                width: 1.0,
                color: COLOR_TEXT_SECONDARY,  // ← NUEVO: Border con COLOR_TEXT_SECONDARY
            },
            ..Default::default()
        },
        COLOR_CONTRAST,  // ← Fondo correcto
    );

    // Dibujar texto del tooltip (COLOR_TEXT_SECONDARY con tamaño configurable)
    renderer.fill_text(
        iced::advanced::text::Text {
            content: val_display,
            bounds: iced::Size::new(tooltip_width - tooltip_padding * 2.0, tooltip_height - tooltip_padding * 2.0),
            size: self.options.tooltip_font_size.into(),  // ← Tamaño configurable
            line_height: iced::advanced::text::LineHeight::default(),
            font: FONT_INTER_SANS_MEDIUM,
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center,
            shaping: iced::advanced::text::Shaping::Basic,
            wrapping: iced::advanced::text::Wrapping::default(),
        },
        iced::Point::new(tooltip_x + tooltip_padding, tooltip_y + tooltip_padding),
        COLOR_TEXT_SECONDARY,  // ← Texto correcto
        tooltip_rect,
    );
}
```

**Cambios Principales:**
1. **Posición:** Ahora basada en `cursor_pos` en lugar de `bounds`
2. **Fondo:** Usa `COLOR_CONTRAST` (era otro color)
3. **Borde:** Agregado `1.0` width con `COLOR_TEXT_SECONDARY`
4. **Texto Color:** Cambio a `COLOR_TEXT_SECONDARY` (era `COLOR_TEXT_PRIMARY`)
5. **Font Size:** Usa `self.options.tooltip_font_size` (era hardcodeado 10.0)
6. **Padding:** Calculado dinámicamente basado en font size

---

## Resumen de Cambios

| Componente | Tipo de Cambio | Líneas | Descripción |
|-----------|-----------------|--------|-------------|
| CustomSliderOptions | Agregar campo | ~1695 | Agregado `tooltip_font_size: f32` |
| CustomSliderState | Nuevo Struct | ~1707 | Estado persistente para is_dragging |
| Builder Methods | Agregar 2 métodos | ~1796-1810 | show_tooltip() y tooltip_font_size() |
| state() | Nuevo método | ~1853 | Inicialización de tree state |
| update() | Reescribir | ~1861-1964 | Drag tracking, ButtonReleased, keyboard scoping |
| draw() | Modificar | ~1995-2002 | Pasar cursor_pos a draw_tooltip() |
| draw_tooltip() | Reescribir | ~2180-2231 | Colores, borde, posición, font size correctos |

---

## Testing Checklist

- [ ] Tooltip aparece junto al cursor
- [ ] Tooltip tiene COLOR_CONTRAST background
- [ ] Tooltip tiene 1px border en COLOR_TEXT_SECONDARY
- [ ] Tooltip texto en COLOR_TEXT_SECONDARY
- [ ] Tooltip font size es 13px (configurable)
- [ ] Drag es fluido y continuo
- [ ] Keyboard solo afecta slider con hover
- [ ] Multiple sliders no interfieren
- [ ] No hay memory leaks de state
- [ ] Performance es aceptable

---

## Backward Compatibility

✓ Todos los cambios son **backwards compatible**:
- Nuevos métodos builder son opcionales
- Campo `tooltip_font_size` tiene default value
- Cambios internos no afectan API publica
- No hay breaking changes
