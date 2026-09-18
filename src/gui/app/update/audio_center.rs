//! Audio center message handlers extracted from the update dispatcher.

use crate::gui::app::view::dialogs::DIALOG_TEXT_INPUT_ID;
use crate::gui::app::{ActiveDialog, AppFocus, Message};
use crate::gui::audio_center::AudioCenterMessage;
use iced::Task;
use iced::widget::operation::focus;

impl crate::gui::app::AudoxidyApp {
    pub(in crate::gui::app) fn handle_audio_center(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleAudioCenter(opt_tab) => {
                if let Some(tab_idx) = opt_tab {
                    if self.audio_center_manager.open
                        && self.audio_center_manager.selected_tab == tab_idx
                    {
                        // Si ya está abierto en la misma pestaña, lo cerramos
                        self.audio_center_manager
                            .save_eq_settings_to_db(&*self.database);
                        self.audio_center_manager
                            .save_dsp_settings_to_db(&*self.audio_manager, &*self.database);
                        self.audio_center_manager.open = false;
                        self.audio_center_manager.window_pos = None;
                    } else {
                        // Si está cerrado o en otra pestaña, lo abrimos/cambiamos a esa pestaña
                        if !self.audio_center_manager.open {
                            self.audio_center_manager.open = true;
                            self.audio_center_manager.window_pos = None;
                        }
                        self.audio_center_manager.selected_tab = tab_idx;
                        if self.audio_center_manager.first_open {
                            self.audio_center_manager
                                .sync_from_engine(&self.audio_manager);
                            self.audio_center_manager.first_open = false;
                        }
                    }
                } else {
                    // Alternancia genérica (comportamiento anterior)
                    if self.audio_center_manager.open {
                        self.audio_center_manager
                            .save_eq_settings_to_db(&*self.database);
                        self.audio_center_manager
                            .save_dsp_settings_to_db(&*self.audio_manager, &*self.database);
                    }
                    self.audio_center_manager.open = !self.audio_center_manager.open;
                    if self.audio_center_manager.open {
                        self.audio_center_manager.window_pos = None;
                        if self.audio_center_manager.first_open {
                            self.audio_center_manager
                                .sync_from_engine(&self.audio_manager);
                            self.audio_center_manager.first_open = false;
                        }
                    }
                }
                Task::none()
            }
            Message::EqPresetImportFile => {
                return Task::perform(
                    async {
                        let file = rfd::AsyncFileDialog::new()
                            .add_filter("Audoxidy EQ Preset", &["json", "aeqp"])
                            .set_title("Importar preset de ecualizador")
                            .pick_file()
                            .await;
                        if let Some(f) = file {
                            let data = f.read().await;
                            Some((f.path().to_path_buf(), data))
                        } else {
                            None
                        }
                    },
                    |result| {
                        Message::AudioCenterMsg(AudioCenterMessage::EqPresetFileImported(result))
                    },
                );
            }
            Message::EqPresetExportFile(_preset_name) => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Audoxidy EQ Preset", &["json", "aeqp"])
                            .set_title("Exportar preset de ecualizador")
                            .save_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    |result| {
                        Message::AudioCenterMsg(AudioCenterMessage::EqPresetFileExported(result))
                    },
                );
            }
            Message::AudioCenterMsg(ac_msg) => {
                match &ac_msg {
                    crate::gui::audio_center::AudioCenterMessage::SliderHoverActive(true) => {
                        if self.focus != AppFocus::AudioCenter {
                            self.previous_focus = self.focus;
                        }
                        self.focus = AppFocus::AudioCenter;
                    }
                    crate::gui::audio_center::AudioCenterMessage::SliderHoverActive(false) => {
                        self.focus = self.previous_focus;
                    }
                    _ => {}
                }

                if let crate::gui::audio_center::AudioCenterMessage::DragStart = ac_msg {
                    self.audio_center_manager.drag_start = Some(self.last_mouse_pos);
                    self.audio_center_manager.is_dragging = true;
                }

                // EqPresetIconLoad opens the Load dialog in app.rs
                if let crate::gui::audio_center::AudioCenterMessage::EqPresetIconLoad = &ac_msg {
                    let backup = crate::gui::audio_center::EqStateBackup {
                        preamp_gain: self.audio_center_manager.preamp_gain,
                        eq_band_gains: self.audio_center_manager.eq_band_gains.clone(),
                        selected_preset: self.audio_center_manager.selected_preset.clone(),
                    };
                    self.active_dialog = ActiveDialog::EqPresetLoad {
                        selected_preset_idx: None,
                        preview_backup: Some(backup),
                    };
                    self.dialog_pos = Some(self.last_mouse_pos);
                }

                // EqPresetLoadSelected: update dialog state for selection persistence
                if let crate::gui::audio_center::AudioCenterMessage::EqPresetLoadSelected(idx) =
                    &ac_msg
                {
                    if let ActiveDialog::EqPresetLoad {
                        selected_preset_idx,
                        ..
                    } = &mut self.active_dialog
                    {
                        *selected_preset_idx = *idx;
                    }
                }

                // EqPresetIconSave opens the Save dialog in app.rs
                if let crate::gui::audio_center::AudioCenterMessage::EqPresetIconSave = &ac_msg {
                    let focus_task = focus(DIALOG_TEXT_INPUT_ID.clone());
                    self.active_dialog = ActiveDialog::EqPresetSave {
                        name_input: String::new(),
                    };
                    self.dialog_pos = Some(self.last_mouse_pos);
                    return focus_task;
                }

                // Forward to audio_center_manager.update()
                self.audio_center_manager.update(
                    ac_msg.clone(),
                    &self.audio_manager,
                    &self.database,
                );

                // Persistir inmediatamente cada cambio a la base de datos
                match &ac_msg {
                    // EQ: cualquier cambio se guarda de inmediato
                    crate::gui::audio_center::AudioCenterMessage::EqToggleSelected(_)
                    | crate::gui::audio_center::AudioCenterMessage::EqBandsSelected(_)
                    | crate::gui::audio_center::AudioCenterMessage::EqPreampChanged(_)
                    | crate::gui::audio_center::AudioCenterMessage::EqBandChanged(_, _)
                    | crate::gui::audio_center::AudioCenterMessage::EqPresetIconReset
                    | crate::gui::audio_center::AudioCenterMessage::EqPresetLoadConfirm
                    | crate::gui::audio_center::AudioCenterMessage::EqPresetSaveConfirm
                    | crate::gui::audio_center::AudioCenterMessage::EqPresetDelete(_)
                    | crate::gui::audio_center::AudioCenterMessage::EqPresetFileImported(_) => {
                        self.audio_center_manager
                            .save_eq_settings_to_db(&*self.database);
                    }
                    _ => {}
                }
                match &ac_msg {
                    // DSP: cualquier cambio se guarda de inmediato
                    crate::gui::audio_center::AudioCenterMessage::DspToggle(_, _)
                    | crate::gui::audio_center::AudioCenterMessage::DspValueChanged(_, _)
                    | crate::gui::audio_center::AudioCenterMessage::AudioStateToggle(_, _)
                    | crate::gui::audio_center::AudioCenterMessage::AudioStateValueChanged(_, _)
                    | crate::gui::audio_center::AudioCenterMessage::StereoExpanderModeToggled(_)
                    | crate::gui::audio_center::AudioCenterMessage::EqPresetFileExported(_) => {
                        self.audio_center_manager
                            .save_dsp_settings_to_db(&*self.audio_manager, &*self.database);
                    }
                    _ => {}
                }
                match &ac_msg {
                    // Volumen y Mezcla: cualquier cambio se guarda de inmediato
                    crate::gui::audio_center::AudioCenterMessage::VolumenFadesToggle(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenSmoothVolumeToggle(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenFadeInToggle(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenFadeOutToggle(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenFadeInChanged(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenFadeOutChanged(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenSilenceToggle(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenSilenceDurationChanged(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenSilenceThresholdChanged(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenSilenceEdgeTrimToggle(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenRgMasterToggle(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenRgTrackToggle(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenRgAlbumToggle(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenRgAnalyzeRtToggle(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenRgOffsetAlbumChanged(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenRgOffsetTrackChanged(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenRgOffsetRtChanged(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenRgFixedToggle(_)
                    | crate::gui::audio_center::AudioCenterMessage::VolumenRgFixedChanged(_) => {
                        self.audio_center_manager
                            .save_volumen_settings_to_db(&*self.audio_manager, &*self.database);
                    }
                    _ => {}
                }
                match &ac_msg {
                    // Mezcla Cruzada / Crossfade: cualquier cambio se guarda de inmediato
                    crate::gui::audio_center::AudioCenterMessage::CrossfadeToggle(_)
                    | crate::gui::audio_center::AudioCenterMessage::CrossfadeManualToggle(_)
                    | crate::gui::audio_center::AudioCenterMessage::CrossfadeManualChanged(_)
                    | crate::gui::audio_center::AudioCenterMessage::CrossfadeAutoToggle(_)
                    | crate::gui::audio_center::AudioCenterMessage::CrossfadeAutoChanged(_) => {
                        self.audio_center_manager
                            .save_crossfade_settings_to_db(&*self.audio_manager, &*self.database);
                    }
                    _ => {}
                }

                Task::none()
            }
            _ => Task::none(),
        }
    }
}
