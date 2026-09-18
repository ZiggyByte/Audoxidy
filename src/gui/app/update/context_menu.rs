//! Context-menu message handlers extracted from the update dispatcher.

use crate::gui::app::Message;
use iced::Task;

impl crate::gui::app::AudoxidyApp {
    pub(in crate::gui::app) fn handle_context_menu(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenContextMenu(pos, entries) => {
                let wrapped = self.wrap_context_menu(entries);
                self.context_menu = Some((pos, wrapped));
                Task::none()
            }
            Message::RequestContextMenu(entries) => {
                // Auto-seleccionar si el mensaje viene del sistema de playlist
                // (Normalmente esto lo gestionamos filtrando por el tipo de entrada o mensaje)
                // Pero como RequestContextMenu es genérico, nos apoyamos en que si hay un linear_idx
                // en la primera entrada, es de playlist.
                if let Some(first) = entries.first() {
                    match &first.action {
                        Some(Message::PlaylistContextMenuPlay(idx))
                        | Some(Message::PlaylistShowFileLocation(idx))
                        | Some(Message::PlaylistShowInLibrary(idx))
                        | Some(Message::PlaylistDeleteSelection(idx))
                        | Some(Message::PlaylistToggleEnabled(idx))
                        | Some(Message::PlaylistRequestSubMenu(idx)) => {
                            if !self.playlist_manager.selected_idxs.contains(idx) {
                                self.playlist_manager.selected_idxs.clear();
                                self.playlist_manager.selected_idxs.insert(*idx);
                                self.playlist_manager.focused_idx = Some(*idx);
                                self.playlist_manager.selection_pivot = Some(*idx);
                            }
                        }
                        _ => {}
                    }
                }

                let pos = self.last_mouse_pos;
                let wrapped = self.wrap_context_menu(entries);
                self.context_menu = Some((pos, wrapped));
                Task::none()
            }
            Message::CloseContextMenu => {
                self.context_menu = None;
                self.playlist_manager.show_tab_dropdown = false;
                Task::none()
            }
            Message::ContextMenuAction(msg) => {
                self.context_menu = None;
                self.playlist_manager.show_tab_dropdown = false;
                self.update(*msg)
            }
            _ => Task::none(),
        }
    }

    pub(in crate::gui::app) fn wrap_context_menu(
        &self,
        entries: Vec<crate::gui::widgets::ContextMenuEntry<Message>>,
    ) -> Vec<crate::gui::widgets::ContextMenuEntry<Message>> {
        entries
            .into_iter()
            .map(|mut e| {
                if let Some(a) = e.action.take() {
                    // No envolver selectores recursivos o NoOp
                    match &a {
                        Message::ContextMenuAction(_) | Message::NoOp => {
                            e.action = Some(a);
                        }
                        _ => e.action = Some(Message::ContextMenuAction(Box::new(a))),
                    }
                }
                e
            })
            .collect()
    }
}
