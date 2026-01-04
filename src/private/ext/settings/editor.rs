use crate::{
	APP_DIR, EditorExtension, Settings,
	private::{
		EditorInternalSingle,
		ui::{EditorEguiContext, EditorUiEguiContextPass},
	},
	settings::CurrentThemeSetting,
	util::storage::{Global, GlobalEditorSettings},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::{EguiContext, EguiContextSettings};
use convert_case::{Case, Casing};
use egui::Widget;
use egui_phosphor_icons::icons;
use itertools::Itertools;
use notify::Notification;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
use strum::VariantArray;
use strum_macros::{Display, VariantArray};

#[derive(Default)]
pub struct EditorSettingsUiExtension;

impl EditorExtension for EditorSettingsUiExtension {
	fn build_editor(&self, _ctx: &mut crate::EditorExtensionContext) {}

	fn build_app(&self, app: &mut App) {
		app
			.add_message::<ShowEditorSettings>()
			.init_resource::<EditorSettings>()
			.add_systems(EditorUiEguiContextPass, ShowEditorSettings::show_menu);
	}
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	commands: Commands<'w, 's>,
	editor_settings: ResMut<'w, EditorSettings>,
	global_settings: GlobalEditorSettings<'w, 's>,
	context_settings:
		EditorInternalSingle<'w, 's, &'static mut EguiContextSettings, With<EditorEguiContext>>,
}

#[derive(Message)]
pub struct ShowEditorSettings;

impl ShowEditorSettings {
	fn show_menu(
		mut messages: MessageReader<Self>,
		mut contexts: EditorInternalSingle<&mut EguiContext, With<EditorEguiContext>>,
		mut modal: Local<widgets::MenuModal>,
		mut menu: Local<widgets::CategoryMenu<EditorSettingCategory>>,
		params: Params,
	) {
		modal.open |= !messages.is_empty();
		messages.clear();

		let ctx = contexts.get_mut();
		let id = egui::Id::new("beditor-editor-settings-modal");
		modal.show(ctx, id, |ui| {
			ui.heading("Editor Settings");

			ui.separator();

			menu.ui(
				ui,
				EditorSettingCategory::VARIANTS,
				|ui, selected_category| {
					if let Some(category) = selected_category {
						category.ui(ui, params);
					} else {
						ui.label("Select a category");
					}
				},
			);
		});
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

#[derive(Reflect, Clone, Copy, Display, PartialEq, Eq, VariantArray)]
enum EditorSettingCategory {
	Appearance,

	AdvancedOptions,
}

impl From<EditorSettingCategory> for egui::WidgetText {
	fn from(value: EditorSettingCategory) -> Self {
		Self::Text(value.to_string().to_case(Case::Title))
	}
}

impl From<&EditorSettingCategory> for egui::WidgetText {
	fn from(value: &EditorSettingCategory) -> Self {
		Self::Text(value.to_string().to_case(Case::Title))
	}
}

#[derive(Default)]
struct AppearanceSettings {
	current_theme: String,
	loaded_themes: BTreeMap<String, ThemePair>,
}

impl AppearanceSettings {
	fn ui(ui: &mut egui::Ui, params: Params) {
		let Params {
			mut commands,
			mut editor_settings,
			mut global_settings,
			mut context_settings,
		} = params;

		let EditorSettings {
			appearance_settings: this,
			..
		} = &mut *editor_settings;

		ui.horizontal(|ui| {
			ui.label(format!(
				"Zoom ({zoom:.2}x)",
				zoom = context_settings.scale_factor
			));

			if ui.add(egui::Button::new(icons::MINUS)).clicked() {
				context_settings.scale_factor -= 0.25;
			}

			if ui.add(egui::Button::new(icons::PLUS)).clicked() {
				context_settings.scale_factor += 0.25;
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
	fn ui(self, ui: &mut egui::Ui, params: Params) {
		match self {
			Self::Appearance => AppearanceSettings::ui(ui, params),
			Self::AdvancedOptions => AdvancedOptions::ui(ui),
		}
	}
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
