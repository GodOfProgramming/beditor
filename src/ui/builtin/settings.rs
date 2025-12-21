use std::{collections::BTreeMap, path::PathBuf};

use crate::{
	APP_DIR, EditorState, EditorUi, NoParams, Notification, Settings,
	util::storage::{CurrentThemeSetting, EditorEguiSettings, Global, GlobalSettings},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::{EguiContexts, PrimaryEguiContext};
use convert_case::{Case, Casing};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct EditorSettingsUi {
	selected_category: Option<EditorSettingCategory>,
}

#[derive(SystemParam)]
pub struct EditorSettingsUiParams<'w, 's> {
	commands: Commands<'w, 's>,
	editor_settings: ResMut<'w, EditorSettings>,
	global_settings: GlobalSettings<'w>,
}

impl EditorUi for EditorSettingsUi {
	const NAME: &str = "Editor Settings";

	const ID: uuid::Uuid = uuid!("5c929b24-50f2-4840-93c4-41e865645e64");

	const HIDDEN: bool = true;

	const SCROLL_BARS: [bool; 2] = [true, true];

	type Params<'w, 's> = EditorSettingsUiParams<'w, 's>;

	fn init(app: &mut App) {
		app
			.init_resource::<EditorSettings>()
			.add_observer(on_new_ctx)
			.add_systems(OnEnter(EditorState::Exiting), save_settings);
	}

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		default()
	}

	fn render(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
		let selected = settings_display(
			ui,
			self.selected_category,
			EditorSettingCategory::iter(),
			|ui| {
				if let Some(category) = self.selected_category {
					category.ui(ui, &mut params);
				} else {
					ui.label("Select a category");
				}
			},
		);

		if selected.is_some() {
			self.selected_category = selected;
		}
	}
}

#[derive(Resource)]
struct EditorSettings {
	appearance_settings: AppearanceSettings,

	advanced_options: AdvancedOptions,
}

impl FromWorld for EditorSettings {
	fn from_world(world: &mut World) -> Self {
		let current_theme = world.resource_scope(|_, mut settings: Mut<Settings<Global>>| {
			settings.get::<CurrentThemeSetting>().ok()
		});

		let themes_dir = APP_DIR.join("themes");
		std::fs::create_dir_all(&themes_dir).expect("Could not create themes directory");
		let entries = std::fs::read_dir(themes_dir).unwrap();
		let loaded_themes = entries
			.filter_map_ok(|entry| {
				let path = entry.path();

				let file = std::fs::File::open(&path).ok()?;
				let rdr = std::io::BufReader::new(file);
				let theme: SavedTheme = ron::de::from_reader(rdr).ok()?;
				Some((theme, path))
			})
			.flatten()
			.map(|(t, p)| {
				(
					t.name,
					ThemePair {
						dark: t.dark,
						light: t.light,
						source: p,
					},
				)
			})
			.collect();

		Self {
			appearance_settings: AppearanceSettings {
				current_theme: current_theme.unwrap_or_else(|| String::from("default")),
				loaded_themes,
			},
			advanced_options: default(),
		}
	}
}

#[derive(Reflect, Clone, Copy, EnumIter, Display, PartialEq, Eq)]
enum EditorSettingCategory {
	Appearance,

	AdvancedOptions,
}

#[derive(Default)]
struct AppearanceSettings {
	current_theme: String,
	loaded_themes: BTreeMap<String, ThemePair>,
}

impl AppearanceSettings {
	fn ui(ui: &mut egui::Ui, params: &mut EditorSettingsUiParams) {
		let EditorSettingsUiParams {
			commands,
			editor_settings,
			global_settings,
		} = params;

		let EditorSettings {
			appearance_settings: this,
			..
		} = &mut **editor_settings;

		let ctx = ui.ctx().clone();
		ctx.settings_ui(ui);
		ui.horizontal(|ui| {
			egui::ComboBox::from_label("Selected Theme")
				.selected_text(&this.current_theme)
				.show_ui(ui, |ui| {
					for (key, value) in this.loaded_themes.iter() {
						if ui
							.selectable_value(&mut this.current_theme, key.clone(), key)
							.clicked()
						{
							if let Err(err) = global_settings.set::<CurrentThemeSetting>(&this.current_theme) {
								error!("{err}");
							}
							ctx.set_style_of(egui::Theme::Dark, value.dark.clone());
							ctx.set_style_of(egui::Theme::Light, value.light.clone());
							commands.trigger(Notification::success("Changed Theme"));
						}
					}
				});

			if ui.button("Save Theme").clicked() {
				let dark = ctx.style_of(egui::Theme::Dark);
				let light = ctx.style_of(egui::Theme::Light);
				let saved_theme = SavedTheme {
					name: this.current_theme.clone(),
					dark: egui::Style::clone(&dark),
					light: egui::Style::clone(&light),
				};

				match ron::ser::to_string_pretty(&saved_theme, ron::ser::PrettyConfig::new()) {
					Ok(data) => {
						let file_path = APP_DIR
							.join("themes")
							.join(format!("{}.ron", saved_theme.name));
						match std::fs::write(&file_path, data) {
							Ok(_) => {
								this.loaded_themes.insert(
									this.current_theme.clone(),
									ThemePair {
										dark: saved_theme.dark,
										light: saved_theme.light,
										source: file_path,
									},
								);
							}
							Err(err) => {
								commands.trigger(Notification::error("Failed to save theme").with_context(err));
							}
						}
					}
					Err(err) => {
						commands.trigger(Notification::error("Failed to serialize theme").with_context(err));
					}
				}
			}

			if ui.button("Remove Theme").clicked()
				&& let Some(theme) = this.loaded_themes.remove(&this.current_theme)
			{
				match std::fs::remove_file(&theme.source) {
					Ok(_) => {
						commands.trigger(Notification::success(format!(
							"Removed theme {}",
							this.current_theme
						)));
					}
					Err(err) => {
						commands.trigger(
							Notification::error(format!("Failed to remove theme {}", this.current_theme))
								.with_context(err),
						);

						this.loaded_themes.insert(this.current_theme.clone(), theme);
					}
				}
			}

			ui.text_edit_singleline(&mut this.current_theme);
		});
	}
}

#[derive(Clone)]
struct ThemePair {
	dark: egui::Style,
	light: egui::Style,
	source: PathBuf,
}

#[derive(Reflect, Default)]
struct AdvancedOptions;

impl AdvancedOptions {
	fn ui(_ui: &mut egui::Ui) {}
}

impl EditorSettingCategory {
	fn ui(self, ui: &mut egui::Ui, params: &mut EditorSettingsUiParams) {
		match self {
			Self::Appearance => AppearanceSettings::ui(ui, params),
			Self::AdvancedOptions => AdvancedOptions::ui(ui),
		}
	}
}

#[derive(Default, Component, Reflect)]
pub struct ProjectSettingsUi;

impl EditorUi for ProjectSettingsUi {
	const NAME: &str = "Project Settings";

	const ID: uuid::Uuid = uuid!("a28755a1-ab68-44e6-b2b0-17cc14de0081");

	const HIDDEN: bool = true;

	type Params<'w, 's> = NoParams;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		default()
	}

	fn render(&mut self, ui: &mut egui::Ui, _params: Self::Params<'_, '_>) {}
}

fn settings_display<C>(
	ui: &mut egui::Ui,
	selected_category: Option<C>,
	category_list: impl Iterator<Item = C>,
	content: impl FnOnce(&mut egui::Ui),
) -> Option<C>
where
	C: std::fmt::Display + Eq + Copy,
{
	let mut out = None;

	ui.allocate_ui_with_layout(
		ui.available_size(),
		egui::Layout::left_to_right(egui::Align::Center),
		|ui| {
			egui::ScrollArea::vertical()
				.max_width(ui.available_width() * 0.1)
				.auto_shrink([false; 2])
				.id_salt("categories")
				.show(ui, |ui| {
					ui.vertical(|ui| {
						ui.heading("Categories");
						ui.separator();

						for item in category_list {
							if ui
								.selectable_label(
									selected_category == Some(item),
									item.to_string().to_case(Case::Title),
								)
								.clicked()
							{
								out = Some(item);
							}
						}
					});
				});

			ui.separator();

			egui::ScrollArea::vertical()
				.auto_shrink([true, false])
				.id_salt("contents")
				.show(ui, |ui| {
					ui.vertical(|ui| {
						if let Some(selected_category) = selected_category {
							ui.heading(selected_category.to_string().to_case(Case::Title));
							ui.separator();
						}

						(content)(ui);
					});
				});
		},
	);

	out
}

fn save_settings(mut contexts: EguiContexts, mut settings: GlobalSettings) {
	if let Ok(ctx) = contexts.ctx_mut() {
		let opts = ctx.options(|opts| opts.clone());
		let _ = settings.set::<EditorEguiSettings>(opts);
	}
}

fn on_new_ctx(
	event: On<Add, PrimaryEguiContext>,
	mut q_ctx: Query<&mut bevy_egui::EguiContext>,
	editor_settings: Res<EditorSettings>,
) {
	if let Some(value) = editor_settings
		.appearance_settings
		.loaded_themes
		.get(&editor_settings.appearance_settings.current_theme)
	{
		let Ok(mut ctx) = q_ctx.get_mut(event.event_target()) else {
			return;
		};

		let ctx = ctx.get_mut();
		ctx.set_style_of(egui::Theme::Dark, value.dark.clone());
		ctx.set_style_of(egui::Theme::Light, value.light.clone());
		info!(
			"Restored style of {}",
			editor_settings.appearance_settings.current_theme
		);
	}
}

#[derive(Serialize, Deserialize)]
struct SavedTheme {
	name: String,
	dark: egui::Style,
	light: egui::Style,
}
