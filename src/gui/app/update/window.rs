//! Window message handlers extracted from the update dispatcher.

use crate::gui::app::persistence::log_persist;
use crate::gui::app::{AppFocus, AudoxidyApp, Message};
use crate::gui::dimensions::{ALBUM_SCROLLBAR_RAIL, LIBRARY_CONTENT_X_ORIGIN};
use crate::utils::settings_keys::KEY_PLAYER_VOLUME;
use iced::Task;

/// Squared distance (in px²) the pointer must travel while pressed on a bar
/// before the window drag starts.
const WINDOW_DRAG_THRESHOLD_SQ: f32 = 16.0;

/// Whether the pointer moved far enough from the press origin to start dragging.
///
/// A short click stays below the threshold, so it never moves the window; the
/// comparison is strict, so travelling exactly the threshold distance is not
/// enough yet.
fn exceeds_drag_threshold(origin: iced::Point, position: iced::Point) -> bool {
    let dx = position.x - origin.x;
    let dy = position.y - origin.y;
    dx * dx + dy * dy > WINDOW_DRAG_THRESHOLD_SQ
}

/// Whether the window rectangle lies entirely outside the monitor.
///
/// A window merely overlapping an edge is restored as-is; only a rectangle with
/// no visible area in the monitor triggers the relocation fallback.
fn window_fully_off_screen(position: iced::Point, size: (u32, u32), monitor: iced::Size) -> bool {
    position.x >= monitor.width
        || position.y >= monitor.height
        || position.x + size.0 as f32 <= 0.0
        || position.y + size.1 as f32 <= 0.0
}

impl crate::gui::app::AudoxidyApp {
    pub(in crate::gui::app) fn handle_window(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::GlobalKeyDown(key, modifiers) => {
                use crate::gui::library::LibraryNavDir;
                use iced::keyboard::Key;
                use iced::keyboard::key::Named;

                match key {
                    Key::Named(Named::ArrowUp) => {
                        if self.focus == AppFocus::Playlist {
                            self.playlist_manager
                                .handle_key_nav(LibraryNavDir::Up, modifiers);
                            if let Some(idx) = self.playlist_manager.focused_idx {
                                return self.execute_playlist_autoscroll(idx);
                            }
                            Task::none()
                        } else if self.focus == AppFocus::AudioCenter {
                            Task::none()
                        } else {
                            self.update(Message::LibraryKeyNav(LibraryNavDir::Up, modifiers))
                        }
                    }
                    Key::Named(Named::ArrowDown) => {
                        if self.focus == AppFocus::Playlist {
                            self.playlist_manager
                                .handle_key_nav(LibraryNavDir::Down, modifiers);
                            if let Some(idx) = self.playlist_manager.focused_idx {
                                return self.execute_playlist_autoscroll(idx);
                            }
                            Task::none()
                        } else if self.focus == AppFocus::AudioCenter {
                            Task::none()
                        } else {
                            self.update(Message::LibraryKeyNav(LibraryNavDir::Down, modifiers))
                        }
                    }
                    Key::Character(ref c) if c.to_lowercase() == "a" && modifiers.command() => {
                        if self.focus == AppFocus::Playlist {
                            self.playlist_manager.select_all();
                            Task::none()
                        } else {
                            self.library_manager.select_all();
                            Task::none()
                        }
                    }
                    Key::Named(Named::ArrowLeft) => {
                        if self.focus == AppFocus::Playlist {
                            self.playlist_manager
                                .handle_key_nav(LibraryNavDir::Left, modifiers);
                            Task::none()
                        } else if self.focus == AppFocus::AudioCenter {
                            Task::none()
                        } else {
                            self.update(Message::LibraryKeyNav(LibraryNavDir::Left, modifiers))
                        }
                    }
                    Key::Named(Named::ArrowRight) => {
                        if self.focus == AppFocus::Playlist {
                            self.playlist_manager
                                .handle_key_nav(LibraryNavDir::Right, modifiers);
                            Task::none()
                        } else if self.focus == AppFocus::AudioCenter {
                            Task::none()
                        } else {
                            self.update(Message::LibraryKeyNav(LibraryNavDir::Right, modifiers))
                        }
                    }
                    Key::Named(Named::Enter) => {
                        if self.focus == AppFocus::Playlist {
                            // Reproducción inmediata para Enter en Playlist
                            // (reinicia la canción actual si ya se está reproduciendo).
                            if let Some(idx) = self.playlist_manager.focused_idx
                                && self
                                    .playlist_manager
                                    .get_song_at_linear_index(idx)
                                    .is_some()
                            {
                                self.play_song_from_playlist(idx);
                            }
                            Task::none()
                        } else {
                            self.update(Message::PlayLibrarySelection)
                        }
                    }
                    Key::Named(Named::Delete) => {
                        if self.focus == AppFocus::Playlist {
                            if let Some(idx) = self.playlist_manager.focused_idx {
                                return self.update(Message::PlaylistDeleteSelection(idx));
                            }
                            Task::none()
                        } else {
                            self.update(Message::LibraryDeleteSelection)
                        }
                    }
                    Key::Named(Named::Space) => {
                        if self.focus == AppFocus::Playlist
                            && let Some(sel) = self.playlist_manager.focused_idx
                        {
                            return self.update(Message::PlaylistToggleEnabled(sel));
                        }
                        Task::none()
                    }
                    _ => Task::none(),
                }
            }
            Message::WindowResized(w, h) => {
                let previous_width = self.window_size.0 as f32;
                self.window_size = (w, h);
                let sidebar_w = 722.0_f32;
                self.library_manager.library_area_width = (w as f32 - sidebar_w).max(202.0);
                self.geometry_dirty_at = Some(std::time::Instant::now());

                // The card geometry follows the window width one-to-one (the
                // panels and insets are fixed), so the next layout can be
                // predicted here and the scroll offset corrected in the same
                // update. Applying it before the layout removes the frame of
                // lag that a deferred correction would show as a jump.
                let anchor = self.reanchor_viewport(w as f32 - previous_width);

                // El runtime no emite un evento de maximizado y la propiedad
                // `_NET_WM_STATE` puede actualizarse después del cambio de
                // tamaño, así que se consulta con un pequeño retardo. El flag
                // evita apilar consultas durante un arrastre de redimensionado.
                let mut tasks = Vec::new();
                if let Some(anchor) = anchor {
                    tasks.push(anchor);
                }

                // Dragging the window between monitors with different scales does
                // not always deliver a rescale event, so the ratio is re-queried
                // here: without it the decode bucket would stay at the previous
                // monitor's size.
                if let Some(window_id) = self.window_id {
                    tasks.push(iced::window::scale_factor(window_id).map(Message::WindowRescaled));
                }

                if !self.maximized_query_pending {
                    self.maximized_query_pending = true;
                    tasks.push(Task::perform(
                        async {
                            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                        },
                        |_| Message::QueryMaximized,
                    ));
                }

                Task::batch(tasks)
            }
            Message::WindowRescaled(scale) => {
                if scale.is_finite() && scale > 0.0 {
                    self.library_manager.scale_factor = scale;
                    // The memoized rows carry the handles of the previous bucket,
                    // so they are invalidated and the window re-requested; the
                    // album snapshot does not change and is not re-read from the
                    // database.
                    self.library_manager.cached_grid_rows.replace(None);
                    self.library_manager.request_cover_window();
                }
                Task::none()
            }
            Message::QueryMaximized => {
                if let Some(window_id) = self.window_id {
                    iced::window::is_maximized(window_id).map(Message::WindowMaximizedState)
                } else {
                    self.maximized_query_pending = false;
                    Task::none()
                }
            }
            Message::WindowMoved(position) => {
                self.window_position = Some(position);
                self.geometry_dirty_at = Some(std::time::Instant::now());
                Task::none()
            }
            Message::WindowMaximizedState(maximized) => {
                self.maximized_query_pending = false;
                if maximized != self.window_maximized {
                    self.window_maximized = maximized;
                    self.geometry_dirty_at = Some(std::time::Instant::now());
                }
                if maximized {
                    // Tamaño de pantalla completa: se guarda para crear la
                    // ventana ya a este tamaño en el próximo arranque y evitar
                    // el parpadeo de abrir pequeño y maximizar después.
                    self.maximized_size = Some(self.window_size);
                } else {
                    // El tamaño actual es el restaurado; es el valor que se
                    // persiste como tamaño normal de la ventana.
                    self.unmaximized_size = self.window_size;
                    self.geometry_dirty_at = Some(std::time::Instant::now());
                }
                Task::none()
            }
            Message::SetWindowId(id) => {
                self.window_id = Some(id);

                // The pixel ratio is queried when the window opens: it is the
                // value that picks the decode bucket of every cover before the
                // first rescale.
                let scale_query = iced::window::scale_factor(id).map(Message::WindowRescaled);

                // El flag `maximized` de `window::Settings` se envía antes de
                // mapear la ventana y algunos gestores de ventanas lo ignoran.
                // Con la ventana ya creada la petición sí se aplica, y ocurre
                // antes del primer render, así que no se ve el cambio de tamaño.
                let cfg = crate::utils::config::load_config();
                if !cfg.ui.window_geometry_initialized || cfg.ui.window_maximized {
                    self.window_maximized = true;
                    return Task::batch(vec![
                        iced::window::maximize(id, true).map(|_: ()| Message::NoOp),
                        scale_query,
                    ]);
                }
                scale_query
            }
            Message::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers;
                Task::none()
            }
            Message::PlayerHoverZone(zone) => {
                self.player_ui_state.hover_zone = zone;
                Task::none()
            }
            Message::PlayerScroll(delta) => {
                let current_vol = self.core.audio_state().volume;
                let direction = if delta > 0.0 { 1.0 } else { -1.0 };
                let vol_percent = current_vol * 100.0;
                let new_vol_percent = (vol_percent + direction * 5.0).clamp(0.0, 100.0);
                self.core.set_volume(new_vol_percent / 100.0);
                // Persist volume immediately
                log_persist(
                    KEY_PLAYER_VOLUME,
                    self.core.persist_volume((new_vol_percent / 100.0) as f64),
                );
                self.player_ui_state.showing_volume = Some(new_vol_percent);
                self.player_ui_state.volume_tick_id =
                    self.player_ui_state.volume_tick_id.wrapping_add(1);
                self.player_ui_state.volume_clearing = false;
                let current_tick = self.player_ui_state.volume_tick_id;

                iced::Task::perform(
                    async { tokio::time::sleep(std::time::Duration::from_millis(1500)).await },
                    move |_| Message::PlayerVolumeTimeout(current_tick),
                )
            }
            Message::PlayerVolumeTimeout(tick) => {
                if self.player_ui_state.volume_tick_id == tick {
                    self.player_ui_state.showing_volume = None;
                    self.player_ui_state.volume_clearing = false;
                }
                Task::none()
            }
            Message::ToggleMenu => {
                self.player_ui_state.is_menu_open = !self.player_ui_state.is_menu_open;
                Task::none()
            }
            Message::GlobalClick => {
                if self.player_ui_state.is_menu_open {
                    self.player_ui_state.is_menu_open = false;
                }
                self.playlist_manager.show_tab_dropdown = false;
                self.filters_manager.menu_open = false;
                self.context_menu = None;
                Task::none()
            }
            Message::WindowDrag => {
                // El press solo arma el arrastre: el arrastre del sistema
                // comienza cuando el puntero supera el umbral, de modo que un
                // clic breve no mueve la ventana.
                self.window_drag_armed = true;
                self.window_drag_origin = self.player_ui_state.mouse_pos;
                Task::none()
            }
            Message::WindowResize(direction) => {
                // Con la ventana maximizada el redimensionado está desactivado:
                // el comportamiento esperado es no poder cambiar el tamaño.
                if self.window_maximized {
                    return Task::none();
                }
                if let Some(window_id) = self.window_id {
                    iced::window::drag_resize(window_id, direction).map(|_: ()| Message::NoOp)
                } else {
                    Task::none()
                }
            }
            Message::PlayerWindowAction(action) => match action {
                crate::gui::player::WindowAction::Minimize => {
                    if let Some(window_id) = self.window_id {
                        iced::window::minimize(window_id, true).map(|_: ()| Message::UiTick)
                    } else {
                        Task::none()
                    }
                }
                crate::gui::player::WindowAction::Maximize => {
                    if let Some(window_id) = self.window_id {
                        // El toggle del botón es la intención explícita; se
                        // refleja de inmediato y la consulta diferida posterior
                        // confirma o corrige el estado real.
                        self.window_maximized = !self.window_maximized;
                        self.geometry_dirty_at = Some(std::time::Instant::now());
                        iced::window::toggle_maximize(window_id).map(|_: ()| Message::NoOp)
                    } else {
                        Task::none()
                    }
                }
                crate::gui::player::WindowAction::Close => {
                    self.persist_geometry();
                    // The exit below skips every destructor, so the planner's
                    // statistics are refreshed here and nothing may follow the
                    // exit call.
                    self.core.optimize_query_planner();
                    std::process::exit(0);
                }
            },
            Message::PlayerMouseMoved(pos) => {
                self.last_mouse_pos = pos;

                // Arrastre de ventana armado: el sistema empieza a mover (y a
                // des-maximizar si procede) solo tras superar el umbral, para
                // que un clic breve no arrastre la ventana.
                if self.window_drag_armed
                    && let Some(origin) = self.window_drag_origin
                    && exceeds_drag_threshold(origin, pos)
                {
                    self.window_drag_armed = false;
                    self.window_drag_origin = None;
                    if let Some(window_id) = self.window_id {
                        return iced::window::drag(window_id).map(|_: ()| Message::NoOp);
                    }
                }

                let is_resizing = self.library_manager.resizing_column.is_some();
                if is_resizing {
                    let col = self.library_manager.resizing_column.unwrap();
                    let start_x = self.library_manager.resizing_start_x;
                    let start_w = self.library_manager.resizing_start_w;

                    let diff = pos.x - start_x;
                    let new_w = (start_w as f32 + diff).max(30.0) as u16;

                    self.library_manager.column_widths.insert(col, new_w);
                }

                if self.library_manager.marquee_start_pos.is_some()
                    && !self.library_manager.is_dragging
                    && let Some(start_pos) = self.library_manager.marquee_start_pos
                {
                    let dx = pos.x - start_pos.x;
                    let dy = pos.y - start_pos.y;
                    let dist_sq = dx * dx + dy * dy;

                    if dist_sq > 9.0 {
                        // Umbral de 3px (3*3=9)
                        self.library_manager.is_dragging = true;
                    }
                }

                if self.library_manager.is_dragging {
                    let lib_x = (pos.x - LIBRARY_CONTENT_X_ORIGIN).max(0.0);
                    let scroll_y = self.library_manager.scroll_offset.y;
                    let lib_y = pos.y - 72.0 + scroll_y;
                    self.library_manager.marquee_end = Some(iced::Point::new(lib_x, lib_y));
                }

                if self.audio_center_manager.is_dragging
                    && let Some(start_pos) = self.audio_center_manager.drag_start
                {
                    let delta_x = pos.x - start_pos.x;
                    let delta_y = pos.y - start_pos.y;

                    let current_pos = self.audio_center_manager.window_pos.unwrap_or_else(|| {
                        let win_w = 940.0;
                        let win_h = 474.0;
                        let x = (self.window_size.0 as f32 - win_w) / 2.0;
                        let y = (self.window_size.1 as f32 - win_h) / 2.0;
                        iced::Point::new(x.max(0.0), y.max(0.0))
                    });

                    let new_x = (current_pos.x + delta_x)
                        .clamp(0.0, (self.window_size.0 as f32 - 100.0).max(0.0));
                    let new_y = (current_pos.y + delta_y)
                        .clamp(0.0, (self.window_size.1 as f32 - 40.0).max(0.0));

                    self.audio_center_manager.window_pos = Some(iced::Point::new(new_x, new_y));
                    self.audio_center_manager.drag_start = Some(pos);
                }

                self.player_ui_state.mouse_pos = Some(pos);
                self.wake_up_controls(true)
            }
            Message::GlobalMouseRelease => {
                // Safety net: clear any lingering stepper hold flag on any primary
                // button release (prevents a stuck hold-to-repeat if a
                // release event is missed by the widget).
                crate::gui::widgets::GLOBAL_STEPPER_HOLD
                    .store(0, std::sync::atomic::Ordering::Relaxed);

                // Un press liberado sin superar el umbral no arrastra nada.
                self.window_drag_armed = false;
                self.window_drag_origin = None;

                if self.library_manager.resizing_column.is_some() {
                    self.library_manager.resizing_column = None;
                }

                if self.library_manager.is_dragging {
                    if let Some(start) = self.library_manager.marquee_start
                        && let Some(pos) = self.player_ui_state.mouse_pos
                    {
                        let lib_x = (pos.x - LIBRARY_CONTENT_X_ORIGIN).max(0.0);
                        let scroll_y = self.library_manager.scroll_offset.y;
                        let lib_y = pos.y - 72.0 + scroll_y;
                        let end = iced::Point::new(lib_x, lib_y);

                        let dx = start.x - end.x;
                        let dy = start.y - end.y;
                        let dist_sq = dx * dx + dy * dy;

                        // Solo procesar como marquee si ha habido un arrastre real (más de 3 píxeles)
                        if dist_sq > 9.0 {
                            let rect_x = start.x.min(end.x);
                            let rect_y = start.y.min(end.y);
                            let rect_w = (start.x - end.x).abs().max(1.0);
                            let rect_h = (start.y - end.y).abs().max(1.0);

                            let rect = iced::Rectangle {
                                x: rect_x,
                                y: rect_y,
                                width: rect_w,
                                height: rect_h,
                            };

                            self.library_manager.select_marquee(rect, self.modifiers);
                        }
                    }
                    self.library_manager.is_dragging = false;
                    self.library_manager.marquee_start = None;
                    self.library_manager.marquee_end = None;
                    self.library_manager.marquee_start_pos = None;
                }

                if self.audio_center_manager.is_dragging {
                    self.audio_center_manager.is_dragging = false;
                    self.audio_center_manager.drag_start = None;
                }

                // Aseguramos que marquee_start_pos se limpie incluso si no hubo dragging real
                self.library_manager.marquee_start_pos = None;
                self.library_manager.is_dragging = false;

                Task::none()
            }
            Message::InitStartup => {
                self.sync_player_art();

                let mut tasks = Vec::new();

                // La geometría inicial (tamaño, posición y maximizado) ya se
                // aplicó al crear la ventana en `main`. Aquí se sincroniza el
                // estado maximizado real y se valida que la ventana no haya
                // quedado fuera del monitor (guard anti fuera-de-pantalla).
                if let Some(window_id) = self.window_id {
                    tasks.push(
                        iced::window::is_maximized(window_id).map(Message::WindowMaximizedState),
                    );
                    tasks.push(
                        iced::window::monitor_size(window_id).map(Message::WindowMonitorSize),
                    );
                }

                // Forzar auto-scroll a la canción que se restauró
                if let Some(idx) = self.playlist_manager.playing_song_idx {
                    tasks.push(self.execute_playlist_autoscroll(idx));
                }

                // Forzar auto-focus a la pestaña de playlist activa
                let active_id = self.playlist_manager.active_playlist_id;
                tasks.push(Task::done(Message::SwitchPlaylist(active_id, false)));

                // Carga inicial de la biblioteca para asegurar que las vistas de lista no estén vacías
                tasks.push(self.trigger_library_search());

                Task::batch(tasks)
            }
            Message::WindowMonitorSize(Some(monitor)) => {
                // Si la ventana restaurada quedó completamente fuera del
                // monitor (por un cambio de resolución o de configuración de
                // pantallas), se recoloca en la esquina superior izquierda.
                if let Some(window_id) = self.window_id {
                    let off_screen = !self.window_maximized
                        && self.window_position.is_some_and(|pos| {
                            window_fully_off_screen(pos, self.unmaximized_size, monitor)
                        });
                    if off_screen {
                        return iced::window::move_to(window_id, iced::Point::new(0.0, 0.0))
                            .map(|_: ()| Message::NoOp);
                    }
                }
                Task::none()
            }
            Message::WindowMonitorSize(None) => Task::none(),
            _ => Task::none(),
        }
    }

    pub(in crate::gui::app) fn wake_up_controls(&mut self, is_mouse_move: bool) -> Task<Message> {
        let new_until = self.player_ui_state.tick_count.wrapping_add(3); // 1.5s a 500ms/tick

        // Optimización: solo actualizar si la diferencia es significativa (> 1 tick) o si estaba inactivo
        let needs_update = !self.player_ui_state.is_active
            || (new_until > self.player_ui_state.active_until_tick
                && new_until - self.player_ui_state.active_until_tick > 1);

        if needs_update {
            self.player_ui_state.is_active = true;
            self.player_ui_state.active_until_tick = new_until;
        } else if !is_mouse_move {
            // Si no es movimiento (ej. click), forzar activación
            self.player_ui_state.is_active = true;
        }

        let mut tasks = vec![];

        if is_mouse_move {
            if self.player_ui_state.showing_volume.is_some()
                && !self.player_ui_state.volume_clearing
            {
                self.player_ui_state.volume_clearing = true;
                let current_vol_tick = self.player_ui_state.volume_tick_id;
                tasks.push(iced::Task::perform(
                    async { tokio::time::sleep(std::time::Duration::from_millis(1000)).await },
                    move |_| Message::PlayerVolumeTimeout(current_vol_tick),
                ));
            }
        } else {
            self.player_ui_state.showing_volume = None;
            self.player_ui_state.volume_clearing = false;
        }

        if tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(tasks)
        }
    }

    pub(in crate::gui::app) fn persist_geometry(&mut self) {
        if self.window_id.is_some() {
            let (w, h) = self.unmaximized_size;
            let mut cfg = crate::utils::config::load_config();
            cfg.ui.window_width = w as f32;
            cfg.ui.window_height = h as f32;
            if let Some(pos) = self.window_position {
                cfg.ui.window_x = Some(pos.x);
                cfg.ui.window_y = Some(pos.y);
            }
            cfg.ui.window_maximized = self.window_maximized;
            if let Some((mw, mh)) = self.maximized_size {
                cfg.ui.window_maximized_width = Some(mw as f32);
                cfg.ui.window_maximized_height = Some(mh as f32);
            }
            cfg.ui.window_geometry_initialized = true;
            if let Err(e) = crate::utils::config::save_config(&cfg) {
                tracing::warn!("Failed to persist window geometry: {e}");
            }
        }
    }
}

#[cfg(test)]
#[path = "window_shell_tests.rs"]
mod window_shell_tests;

/// Width the next layout will report for a window width delta.
///
/// Once a layout has run the reported width is exact. Before the first layout the
/// snapshot is the minimum one, so inverting it is the only estimate available.
fn predicted_reported_width(
    measured: f32,
    current: crate::gui::library::grid_metrics::GridMetrics,
    width_delta: f32,
) -> f32 {
    let base = if measured > 0.0 {
        measured
    } else {
        current.columns as f32 * current.card_width + ALBUM_SCROLLBAR_RAIL
    };
    base + width_delta
}

impl AudoxidyApp {
    /// Re-anchors the library viewport for a window width delta.
    ///
    /// The card geometry follows the window width one-to-one because the panels
    /// and insets are fixed, so the next metrics can be predicted before the
    /// layout runs and the scroll offset corrected without a deferred frame.
    fn reanchor_viewport(&mut self, width_delta: f32) -> Option<Task<Message>> {
        if self.library_manager.view_mode != crate::gui::library::LibraryViewMode::Grid {
            return None;
        }
        // The width the last layout reported is exact; only before the first
        // layout does the snapshot have to be inverted, and then its value is the
        // minimum one. Trusting the inversion after a layout was the defect: the
        // predicted width could be ~1000 px when the real content was ~517 px.
        let reported_width = predicted_reported_width(
            self.library_manager.reported_width.get(),
            self.library_manager.grid_metrics.get(),
            width_delta,
        );
        let next = crate::gui::library::grid_metrics::grid_metrics(reported_width);

        self.apply_grid_anchor_to(next, self.library_manager.scroll_offset.y)
    }
}
