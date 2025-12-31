use crate::{
	APP_DIR, EditorExtension, EditorOwned, EditorState, EditorUi, Settings,
	settings::{CurrentThemeSetting, EditorEguiSettings, EditorUiScale},
	util::storage::{Global, GlobalEditorSettings},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::{EguiContextSettings, EguiContexts, PrimaryEguiContext};
use egui::Widget;
use egui_phosphor_icons::icons;
use itertools::Itertools;
use notify::Notification;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};
use uuid::uuid;

#[derive(Default)]
pub struct EditorSettingsUiExtension;

impl EditorExtension for EditorSettingsUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<EditorSettingsUi>();
	}

	fn build_app(&self, app: &mut App) {
		app.init_resource::<EditorSettings>().add_systems(
			OnEnter(EditorState::Exiting),
			(save_context_options, save_scale_factor),
		);
	}
}

#[derive(Default, Component, Reflect)]
#[require(EditorOwned)]
pub struct EditorSettingsUi {
	selected_category: Option<EditorSettingCategory>,
}

#[derive(SystemParam)]
pub struct EditorSettingsUiParams<'w, 's> {
	commands: Commands<'w, 's>,
	editor_settings: ResMut<'w, EditorSettings>,
	global_settings: GlobalEditorSettings<'w, 's>,
	q_contexts: Query<'w, 's, &'static mut EguiContextSettings, With<PrimaryEguiContext>>,
}

impl EditorUi for EditorSettingsUi {
	const NAME: &str = "Editor Settings";

	const ID: uuid::Uuid = uuid!("5c929b24-50f2-4840-93c4-41e865645e64");

	const HIDDEN: bool = true;

	const SCROLL_BARS: [bool; 2] = [true, true];

	type Params<'w, 's> = EditorSettingsUiParams<'w, 's>;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		default()
	}

	fn ui(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
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
			settings.get(CurrentThemeSetting).ok()
		});

		Self {
			appearance_settings: AppearanceSettings {
				current_theme: current_theme.unwrap_or_else(|| String::from("default")),
				loaded_themes: load_themes(),
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
			q_contexts,
		} = params;

		let EditorSettings {
			appearance_settings: this,
			..
		} = &mut **editor_settings;

		ui.horizontal(|ui| {
			for mut ctx_settings in q_contexts {
				ui.label(format!(
					"Zoom ({zoom:.2}x)",
					zoom = ctx_settings.scale_factor
				));

				if ui.add(egui::Button::new(icons::MINUS)).clicked() {
					ctx_settings.scale_factor -= 0.25;
				}

				if ui.add(egui::Button::new(icons::PLUS)).clicked() {
					ctx_settings.scale_factor += 0.25;
				}
			}
		});

		let ctx = ui.ctx().clone();

		ctx.settings_ui(ui);

		ui.vertical(|ui| {
			egui_autocomplete::AutoCompleteTextEdit::new(
				&mut this.current_theme,
				this.loaded_themes.keys(),
			)
			.popup_on_focus(true)
			.ui(ui);

			ui.horizontal(|ui| {
				if ui.button("Change Theme").clicked()
					&& let Some(value) = this.loaded_themes.get(&this.current_theme)
				{
					global_settings
						.set(CurrentThemeSetting, this.current_theme.clone())
						.ok();

					ctx.set_style_of(egui::Theme::Dark, value.dark.clone());
					ctx.set_style_of(egui::Theme::Light, value.light.clone());
					commands.trigger(Notification::success("Changed Theme"));
				}

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

fn save_context_options(mut contexts: EguiContexts, mut settings: GlobalEditorSettings) {
	if let Ok(ctx) = contexts.ctx_mut() {
		let opts = ctx.options(|opts| opts.clone());
		let _ = settings.set(EditorEguiSettings, opts);
	}
}

fn save_scale_factor(
	ctx_settings: Single<&EguiContextSettings, With<PrimaryEguiContext>>,
	mut settings: GlobalEditorSettings,
) {
	settings.set(EditorUiScale, ctx_settings.scale_factor).ok();
}

#[derive(Serialize, Deserialize)]
struct SavedTheme {
	name: String,
	dark: egui::Style,
	light: egui::Style,
}

fn load_themes() -> BTreeMap<String, ThemePair> {
	let themes_dir = APP_DIR.join("themes");
	std::fs::create_dir_all(&themes_dir).expect("Could not create themes directory");
	let entries = std::fs::read_dir(themes_dir).expect("Themes directory was just created");
	entries
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
		.collect()
}
