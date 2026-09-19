//! Dialogs message handlers extracted from the update dispatcher.

use crate::gui::app::view::dialogs::DIALOG_TEXT_INPUT_ID;
use crate::gui::app::{ActiveDialog, ExportFormat, ExportMode, Message};
use iced::Task;
use iced::widget::operation::focus;

impl crate::gui::app::AudoxidyApp {
    pub(in crate::gui::app) fn handle_dialogs(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenDialog(dialog) => {
                let focus_task = match &dialog {
                    ActiveDialog::CreatePlaylist { .. } | ActiveDialog::RenamePlaylist { .. } => {
                        focus(DIALOG_TEXT_INPUT_ID.clone())
                    }
                    _ => Task::none(),
                };
                self.active_dialog = dialog;
                self.dialog_pos = Some(self.last_mouse_pos);
                focus_task
            }
            Message::CloseDialog => {
                // Restore from preview backup if closing EqPresetLoad dialog
                if let ActiveDialog::EqPresetLoad {
                    preview_backup: Some(backup),
                    ..
                } = &self.active_dialog
                {
                    self.audio_center_manager.preamp_gain = backup.preamp_gain;
                    self.audio_center_manager.eq_band_gains = backup.eq_band_gains.clone();
                    self.audio_center_manager.selected_preset = backup.selected_preset.clone();
                    self.core.set_preamp_gain(backup.preamp_gain);
                    for (i, &val) in backup.eq_band_gains.iter().enumerate() {
                        self.core.set_eq_band_gain(i, val);
                    }
                }
                self.active_dialog = ActiveDialog::None;
                self.dialog_pos = None;
                Task::none()
            }
            Message::UpdateDialogInput(s) => {
                match &mut self.active_dialog {
                    ActiveDialog::CreatePlaylist { name, .. } => *name = s,
                    ActiveDialog::RenamePlaylist { new_name, .. } => *new_name = s,
                    ActiveDialog::EqPresetSave { name_input } => {
                        *name_input = s.clone();
                        // Also forward to AudioCenterManager for the pending_preset_name
                        self.audio_center_manager.update(
                            crate::gui::audio_center::AudioCenterMessage::EqPresetSaveInput(s),
                            self.core.audio_manager(),
                            self.core.database(),
                        );
                    }
                    _ => {}
                }
                Task::none()
            }
            Message::ConfirmDialogAction => {
                let dialog = std::mem::replace(&mut self.active_dialog, ActiveDialog::None);
                self.active_dialog = ActiveDialog::None;
                self.dialog_pos = None;
                let core = self.core.clone();

                match dialog {
                    ActiveDialog::CreatePlaylist {
                        name,
                        pending_add_songs,
                    } => {
                        let final_name = if name.trim().is_empty() {
                            format!("Nueva lista {}", self.playlist_manager.playlists.len() + 1)
                        } else {
                            name
                        };

                        let target_songs = pending_add_songs;
                        return Task::perform(
                            async move {
                                core.create_playlist_with_songs(&final_name, &target_songs)
                                    .ok()
                            },
                            Message::RefreshPlaylists,
                        );
                    }
                    ActiveDialog::RenamePlaylist { id, new_name, .. } => {
                        if !new_name.trim().is_empty() {
                            let is_system = self
                                .playlist_manager
                                .playlists
                                .iter()
                                .find(|p| p.id == id)
                                .map(|p| p.is_system)
                                .unwrap_or(false);

                            if is_system {
                                let final_name = new_name.trim().to_string();
                                let source_id = id;
                                return Task::perform(
                                    async move {
                                        core.rename_system_playlist(source_id, &final_name).ok()
                                    },
                                    Message::RefreshPlaylists,
                                );
                            } else {
                                return Task::perform(
                                    async move { core.rename_playlist(id, &new_name).ok().map(|_| id) },
                                    Message::RefreshPlaylists,
                                );
                            }
                        }
                    }
                    ActiveDialog::DeleteConfirm { id, .. } => {
                        return Task::perform(
                            async move { core.delete_playlist(id).ok().map(|_| 0) },
                            Message::RefreshPlaylists,
                        );
                    }
                    ActiveDialog::ExportConfirm {
                        id,
                        name: _,
                        format,
                        mode,
                    } => {
                        return Task::perform(
                            async move {
                                match mode {
                                    ExportMode::PortableFolder => {
                                        if let Some(folder) = rfd::AsyncFileDialog::new()
                                            .set_title("Seleccionar carpeta de exportación")
                                            .pick_folder()
                                            .await
                                        {
                                            Some((
                                                id,
                                                folder.path().to_string_lossy().into_owned(),
                                                format,
                                                mode,
                                            ))
                                        } else {
                                            None
                                        }
                                    }
                                    ExportMode::SingleFile => {
                                        let ext = match format {
                                            ExportFormat::M3U => "m3u",
                                            ExportFormat::M3U8 => "m3u8",
                                        };
                                        if let Some(file) = rfd::AsyncFileDialog::new()
                                            .set_title("Guardar lista de reproducción")
                                            .add_filter("Lista de reproducción", &[ext])
                                            .save_file()
                                            .await
                                        {
                                            Some((
                                                id,
                                                file.path().to_string_lossy().into_owned(),
                                                format,
                                                mode,
                                            ))
                                        } else {
                                            None
                                        }
                                    }
                                }
                            },
                            |res| {
                                if let Some((pid, path, fmt, m)) = res {
                                    Message::ExportPlaylist(pid, path, fmt, m)
                                } else {
                                    Message::NoOp
                                }
                            },
                        );
                    }
                    ActiveDialog::EqPresetLoad { .. } => {
                        return Task::done(Message::AudioCenterMsg(
                            crate::gui::audio_center::AudioCenterMessage::EqPresetLoadConfirm,
                        ));
                    }
                    ActiveDialog::EqPresetSave { .. } => {
                        return Task::done(Message::AudioCenterMsg(
                            crate::gui::audio_center::AudioCenterMessage::EqPresetSaveConfirm,
                        ));
                    }
                    _ => {}
                }
                Task::none()
            }
            Message::OpenFolderPicker => {
                let core = self.core.clone();
                Task::perform(
                    async move { rfd::FileDialog::new().pick_folder() },
                    move |folder| {
                        if let Some(f) = folder {
                            let path_str = f.to_string_lossy().to_string();
                            core.scan_folder(path_str);
                        }
                        Message::NoOp
                    },
                )
            }
            Message::OpenPlaylistFilePicker => Task::perform(
                async move {
                    rfd::FileDialog::new()
                        .add_filter("Lista de reproducción", &["m3u", "m3u8"])
                        .pick_file()
                },
                |file| {
                    if let Some(f) = file {
                        Message::ImportPlaylistFile(f.to_string_lossy().to_string())
                    } else {
                        Message::NoOp
                    }
                },
            ),
            Message::ImportPlaylistFile(path) => {
                let core = self.core.clone();
                let path_clone = path.clone();

                // Usamos perform para no bloquear la UI si el M3U es muy grande
                Task::perform(async move { core.import_m3u(&path_clone) }, |playlist_id| {
                    if let Some(id) = playlist_id {
                        Message::SwitchPlaylist(id, true)
                    } else {
                        Message::NoOp
                    }
                })
            }
            Message::SelectExportFormat(fmt) => {
                if let ActiveDialog::ExportConfirm { format, .. } = &mut self.active_dialog {
                    *format = fmt;
                }
                Task::none()
            }
            Message::SelectExportMode(m) => {
                if let ActiveDialog::ExportConfirm { mode, .. } = &mut self.active_dialog {
                    *mode = m;
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }
}
