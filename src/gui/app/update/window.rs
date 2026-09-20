//! Window message handlers extracted from the update dispatcher.

use crate::gui::app::persistence::log_persist;
use crate::gui::app::{AppFocus, Message};
use iced::Task;

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
                            if let Some(idx) = self.playlist_manager.focused_idx {
                                if self
                                    .playlist_manager
                                    .get_song_at_linear_index(idx)
                                    .is_some()
                                {
                                    self.play_song_from_playlist(idx);
                                }
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
                        if self.focus == AppFocus::Playlist {
                            if let Some(sel) = self.playlist_manager.focused_idx {
                                return self.update(Message::PlaylistToggleEnabled(sel));
                            }
                        }
                        Task::none()
                    }
                    _ => Task::none(),
                }
            }
            Message::WindowResized(w, h) => {
                self.window_size = (w, h);
                // La biblioteca ocupa todo el ancho menos el panel izquierdo (~520px) y filtros (~202px)
                let sidebar_w = 722.0_f32;
                self.library_manager.library_area_width = (w as f32 - sidebar_w).max(202.0);
                Task::none()
            }
            Message::SetWindowId(id) => {
                self.window_id = Some(id);
                Task::none()
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
                    "player_volume",
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
            Message::PlayerWindowAction(action) => match action {
                crate::gui::player::WindowAction::Minimize => {
                    if let Some(id) = self.window_id {
                        iced::window::minimize(id, true).map(|_: ()| Message::UiTick)
                    } else {
                        Task::none()
                    }
                }
                crate::gui::player::WindowAction::Maximize => {
                    if let Some(id) = self.window_id {
                        iced::window::toggle_maximize(id).map(|_: ()| Message::UiTick)
                    } else {
                        Task::none()
                    }
                }
                crate::gui::player::WindowAction::Close => {
                    std::process::exit(0);
                }
            },
            Message::PlayerMouseMoved(pos) => {
                self.last_mouse_pos = pos;
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
                {
                    if let Some(start_pos) = self.library_manager.marquee_start_pos {
                        let dx = pos.x - start_pos.x;
                        let dy = pos.y - start_pos.y;
                        let dist_sq = dx * dx + dy * dy;

                        if dist_sq > 9.0 {
                            // Umbral de 3px (3*3=9)
                            self.library_manager.is_dragging = true;
                        }
                    }
                }

                if self.library_manager.is_dragging {
                    let lib_x = (pos.x - 602.0).max(0.0);
                    let scroll_y = self.library_manager.scroll_offset.y;
                    let lib_y = pos.y - 72.0 + scroll_y;
                    self.library_manager.marquee_end = Some(iced::Point::new(lib_x, lib_y));
                }

                if self.audio_center_manager.is_dragging {
                    if let Some(start_pos) = self.audio_center_manager.drag_start {
                        let delta_x = pos.x - start_pos.x;
                        let delta_y = pos.y - start_pos.y;

                        let current_pos =
                            self.audio_center_manager.window_pos.unwrap_or_else(|| {
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

                if self.library_manager.resizing_column.is_some() {
                    self.library_manager.resizing_column = None;
                }

                if self.library_manager.is_dragging {
                    if let Some(start) = self.library_manager.marquee_start {
                        if let Some(pos) = self.player_ui_state.mouse_pos {
                            let lib_x = (pos.x - 602.0).max(0.0);
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
                // Sincronizar carátula inicial
                self.sync_player_art();

                let mut tasks = Vec::new();

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
}
