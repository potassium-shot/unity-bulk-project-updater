use std::path::PathBuf;

use anyhow::Result;
use egui_file_dialog::FileDialog;

use crate::{
    extensions::EguiPathBuf,
    updater::{Update, UpdateState, UpdateStateKind, Updater},
};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct UpdaterApp {
    unity_versions_path: PathBuf,
    unity_version: Option<String>,
    projects_list: Vec<PathBuf>,

    #[serde(skip)]
    cached_versions: Option<Result<Vec<String>>>,

    #[serde(skip)]
    picking_folder_for_idx: Option<usize>,
    #[serde(skip)]
    file_dialog: Option<FileDialog>,
    file_dialog_default_dir: PathBuf,

    #[serde(skip)]
    updater: Option<Updater>,

    max_processes: usize,
}

impl Default for UpdaterApp {
    fn default() -> Self {
        Self {
            unity_versions_path: PathBuf::from("C:\\Program Files\\Unity\\Hub\\Editor"),
            unity_version: None,
            projects_list: Vec::new(),

            cached_versions: None,

            picking_folder_for_idx: None,
            file_dialog: None,
            file_dialog_default_dir: PathBuf::from("C:"),

            updater: None,

            max_processes: 2,
        }
    }
}

impl UpdaterApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            let mut found: UpdaterApp =
                eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
            found.file_dialog =
                Some(FileDialog::new().initial_directory(found.file_dialog_default_dir.clone()));

            return found;
        }

        Default::default()
    }
}

impl eframe::App for UpdaterApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.file_dialog.as_mut().unwrap().update(ctx);

        let mut must_clear_version_cache = false;

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Change Unity Versions Path", |ui| {
                    if ui
                        .text_edit_singleline(&mut EguiPathBuf::new(&mut self.unity_versions_path))
                        .lost_focus()
                    {
                        must_clear_version_cache = true;
                    }
                });

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        if must_clear_version_cache {
            self.cached_versions = None;
        }

        let cached_versions = if let Some(ref cached_versions) = self.cached_versions {
            cached_versions
        } else {
            self.cached_versions = Some(Updater::find_avaible_unity_versions(
                &self.unity_versions_path,
            ));

            if let Some(ref mut unity_version) = self.unity_version {
                if let Some(Ok(ref cached_versions)) = self.cached_versions {
                    if !cached_versions.contains(unity_version) {
                        std::mem::take(unity_version);
                    }
                }
            }

            self.cached_versions.as_ref().unwrap()
        };

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Bulk Update Unity Projects");

            match cached_versions {
                Ok(cached_versions) => {
                    ui.horizontal(|ui| {
                        ui.label("Select a Unity version to convert to:");

                        egui::ComboBox::from_id_salt("version_selector")
                            .selected_text(
                                self.unity_version
                                    .as_ref()
                                    .map(|s| s.as_str())
                                    .unwrap_or("<none>"),
                            )
                            .show_ui(ui, |ui| {
                                for version in cached_versions.iter() {
                                    ui.selectable_value(
                                        &mut self.unity_version,
                                        Some(version.to_owned()),
                                        version,
                                    );
                                }
                            });
                    });

                    if let Some(ref unity_version) = self.unity_version {
                        ui.strong("Select the projects to update:");

                        ui.vertical_centered_justified(|ui| {
                            egui::Grid::new("selected_projects")
                                .num_columns(2)
                                .max_col_width(ui.available_width())
                                .spacing([10.0, 8.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    let mut request_delete = None;

                                    for (idx, item) in self.projects_list.iter_mut().enumerate() {
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.small_button("❌").clicked() {
                                                    request_delete = Some(idx);
                                                }

                                                if ui.small_button("📁").clicked() {
                                                    self.picking_folder_for_idx = Some(idx);
                                                    self.file_dialog
                                                        .as_mut()
                                                        .unwrap()
                                                        .pick_directory();
                                                }

                                                ui.add_sized(
                                                    ui.available_size(),
                                                    egui::TextEdit::singleline(
                                                        &mut EguiPathBuf::new(item),
                                                    ),
                                                );
                                            },
                                        );

                                        ui.end_row();
                                    }

                                    if let Some(request_delete) = request_delete {
                                        self.projects_list.remove(request_delete);
                                    }

                                    if ui.button("Add Projects").clicked() {
                                        self.picking_folder_for_idx = None;
                                        self.file_dialog.as_mut().unwrap().pick_multiple();
                                    }
                                });
                        });

                        ui.separator();

                        ui.horizontal(|ui| {
                            if ui
                                .button(egui::RichText::new("Update All").heading())
                                .clicked()
                            {
                                self.updater = Some(Updater::new(&self.unity_versions_path.join(unity_version), self.max_processes));

                                for project in std::mem::take(&mut self.projects_list) {
                                    self.updater.as_mut().unwrap().add_to_queue(project);
                                }
                            }

                            ui.separator();

                            if self.max_processes > 9 {
                                ui.visuals_mut().override_text_color =
                                    Some(ui.visuals().error_fg_color);
                            }

                            ui.label("Max process count:");
                            ui.add(
                                egui::DragValue::new(&mut self.max_processes)
                                    .range(1..=99)
                                    .speed(0.05),
                            ).on_hover_text("Click and drag to change the max number of unity processes that will be spawned.");

                            ui.visuals_mut().override_text_color = None;
                        });

                        let mut clear = false;

                        if let Some(updater) = &mut self.updater {
                        	let occupied = updater.update();
                         	updater.show(ui);

                          	clear = ui
                           		.button(if occupied { "Stop" } else { "Clear" })
                             	.on_hover_text(
                              		"If updaters are still running, they will keep running in the background. The rest is put back in the list."
                              	)
                               	.clicked();
                        }

                        if clear && self.updater.is_some() {
                        	self.projects_list.append(&mut self
                         		.updater
                           		.take()
                             	.unwrap()
                              	.queue
                               	.into_iter()
                                .filter_map(|update| if update.state.kind() == UpdateStateKind::Pending {
                              		Some(update.project)
                                } else {
                                	None
                                })
                                .collect()
                         	);
                        }
                    }
                }
                Err(error) => {
                    ui.separator();
                    ui.colored_label(ui.visuals().error_fg_color, format!("{}", error));
                }
            }

            if let Some(target_idx) = self.picking_folder_for_idx {
            	if let Some(picked) = self.file_dialog.as_mut().unwrap().take_picked() {
                    if let Some(target_entry) = self.projects_list.get_mut(target_idx) {
                        *target_entry = picked.to_owned();
                    }

                    self.file_dialog_default_dir = picked.parent().unwrap_or(&picked).to_owned();
                }
            } else {
            	if let Some(mut picked_many) = self.file_dialog.as_mut().unwrap().take_picked_multiple() {
             		if let Some(first) = picked_many.first() {
               			self.file_dialog_default_dir = first.parent().unwrap_or(first).to_owned();
               		}

             		self.projects_list.append(&mut picked_many);
             	}
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                egui::warn_if_debug_build(ui);
            });
        });
    }
}

impl Updater {
    fn show(&self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            egui::Grid::new("updater")
                .num_columns(2)
                .spacing([4.0, 8.0])
                .striped(true)
                .show(ui, |ui| {
                    for update in self.queue.iter() {
                        update.show(ui);
                        ui.end_row();
                    }
                });
        });
    }
}

impl Update {
    fn show(&self, ui: &mut egui::Ui) {
        ui.label(EguiPathBuf::new(&self.project));

        match &self.state {
            UpdateState::Pending => {
                ui.weak("Waiting for others to finish");
            }
            UpdateState::Processing(_) => {
                ui.spinner();
            }
            UpdateState::Success => {
                ui.colored_label(
                    egui::Color32::from_rgb(50, 220, 20),
                    "Finished without warnings.",
                );
            }
            UpdateState::Error(text) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("Failed: {}", text));
            }
        }
    }
}
