use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer,
    widget::{Operation, Tree},
};
use iced::{
    Alignment, Color, Element, Length, Padding, Theme,
    widget::{
        Responsive, Space, button, container, mouse_area, pick_list, radio, row, scrollable, svg,
        text, text_input, toggler,
    },
};
use iced::{Event, Rectangle, Size, Vector};

use iced::color;

use crate::gui::theme::{
    COLOR_ACCENT, COLOR_BG, COLOR_CONTRAST, COLOR_TEXT_PRIMARY, COLOR_TEXT_SECONDARY,
    FONT_INTER_SANS_MEDIUM, FONT_INTER_SANS_NORMAL,
};
use crate::utils::{SortColumn, format_duration, truncate_text};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI8, Ordering};

mod buttons;
mod headers;
mod inputs;
#[path = "layout.rs"]
mod layout_ui;
mod meter;
mod overlays;
#[path = "text.rs"]
mod text_ui;

pub use buttons::{
    action_icon_button, chevron_btn, icon_button, standard_pick_list, standard_radio,
    standard_toggler, standard_toggler_full,
};
pub use headers::{album_header_widget, artist_header_widget, build_sort_bar};
pub use inputs::{
    CustomSlider, CustomSliderOptions, GLOBAL_SLIDER_SELECTED, GLOBAL_STEPPER_HOLD, InputAlign,
    InputPosition, InputWidth, NumberStepper, SliderOrientation, StandardCheckbox, StepperUnit,
    VolumeScrollArea, standard_search_input,
};
#[cfg(test)]
pub(crate) use inputs::{format_stepper_value, strip_suffix};
pub use layout_ui::{custom_scrollbar_style, standard_scrollable, standard_scrollbar};
pub use meter::{METER_MIN_DB, VuMeterWidget, meter_popup_view};
pub use overlays::{ContextMenuEntry, build_context_menu_content, modal_text, standard_modal};
pub use text_ui::{
    apply_marquee, smart_truncate_text, smart_truncate_text_advanced,
    smart_truncate_text_with_suffix,
};
