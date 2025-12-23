use crate::{
	APP_DIR, EditorState, EditorUi, Notification, Settings,
	util::storage::{CurrentThemeSetting, EditorEguiSettings, Global, GlobalEditorSettings},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::{EguiContexts, PrimaryEguiContext};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
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
	global_settings: GlobalEditorSettings<'w>,
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
		let selected = super::settings_display(
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

	_advanced_options: AdvancedOptions,
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
			_advanced_options: default(),
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

fn save_settings(mut contexts: EguiContexts, mut settings: GlobalEditorSettings) {
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
	let Ok(mut ctx) = q_ctx.get_mut(event.event_target()) else {
		return;
	};

	let ctx = ctx.get_mut();

	if let Some(value) = editor_settings
		.appearance_settings
		.loaded_themes
		.get(&editor_settings.appearance_settings.current_theme)
	{
		ctx.set_style_of(egui::Theme::Dark, value.dark.clone());
		ctx.set_style_of(egui::Theme::Light, value.light.clone());
		info!(
			"Restored style of {}",
			editor_settings.appearance_settings.current_theme
		);
	} else {
		for theme in [egui::Theme::Dark, egui::Theme::Light] {
			ctx.style_mut_of(theme, |style| {
				style.spacing.window_margin = egui::Margin::same(0);
			});
		}
	}
}

#[derive(Serialize, Deserialize)]
struct SavedTheme {
	name: String,
	dark: egui::Style,
	light: egui::Style,
}
