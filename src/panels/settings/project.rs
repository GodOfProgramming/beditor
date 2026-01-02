use crate::{
	EditorExtension, EditorUi,
	private::{
		EditorInternal, EditorInternalQuery,
		cam::ActiveEditorCamera,
		ui::{
			LayoutManager, LoadLayout, SavedLayout, UiManager,
			misc::{DockExtensions as _, MissingUi},
		},
	},
	settings::SaveLayoutOnExitSetting,
	util::storage::ProjectSettings,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use egui::TextBuffer;
use notify::Notification;
use persistent_id::PersistentId;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};
use uuid::uuid;

#[derive(Default)]
pub struct ProjectSettingsUiExtension;

impl EditorExtension for ProjectSettingsUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<ProjectSettingsUi>();
	}

	fn build_app(&self, app: &mut App) {
		app
			.add_message::<SaveLayoutMessage>()
			.add_message::<ResetLayoutMessage>()
			.add_message::<LoadLayoutMessage>()
			.add_systems(
				FixedUpdate,
				(
					ResetLayoutMessage::handle,
					SaveLayoutMessage::handle,
					LoadLayoutMessage::handle,
				),
			);
	}
}

#[derive(Component)]
#[require(EditorInternal)]
pub struct ProjectSettingsUi {
	selected_category: Option<ProjectSettingCategory>,
	save_layout_dialog: widgets::Dialog,
	reset_layout_dialog: widgets::Dialog,
}

#[derive(SystemParam)]
pub struct ProjectSettingsUiParams<'w, 's> {
	commands: Commands<'w, 's>,
	project_settings: ProjectSettings<'w, 's>,
	active_camera: ResMut<'w, ActiveEditorCamera>,
	layout_manager: ResMut<'w, LayoutManager>,
	layout_name: Local<'s, String>,

	save_layout_on_exit: Local<'s, bool>,
}

impl EditorUi for ProjectSettingsUi {
	const NAME: &str = "Project Settings";

	const ID: uuid::Uuid = uuid!("a28755a1-ab68-44e6-b2b0-17cc14de0081");

	const HIDDEN: bool = true;

	type Params<'w, 's> = ProjectSettingsUiParams<'w, 's>;

	fn spawn(mut params: Self::Params<'_, '_>) -> Self {
		*params.save_layout_on_exit = params
			.project_settings
			.get(SaveLayoutOnExitSetting)
			.unwrap_or(true);

		Self {
			selected_category: None,
			save_layout_dialog: widgets::Dialog::new(egui::Id::new("save_layout_dialog"), "Save Layout"),
			reset_layout_dialog: widgets::Dialog::new(
				egui::Id::new("reset_layout_dialog"),
				"Reset Layout?",
			),
		}
	}

	fn ui(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
		self.save_layout_dialog.show(ui.ctx(), |ui, open| {
			ui.horizontal(|ui| {
				ui.label("Name");
				ui.text_edit_singleline(&mut *params.layout_name);
			});

			ui.horizontal(|ui| {
				if ui.button("Save").clicked() {
					params
						.commands
						.write_message(SaveLayoutMessage(params.layout_name.take()));
					*open = false;
				}

				if ui.button("Cancel").clicked() {
					*open = false;
				}
			});
		});

		self.reset_layout_dialog.show(ui.ctx(), |ui, open| {
			ui.label("This will reset your layout to the default configuration. Continue?");
			ui.horizontal(|ui| {
				if ui.button("Ok").clicked() {
					params.commands.write_message(ResetLayoutMessage);
					*open = false;
				}

				if ui.button("Cancel").clicked() {
					*open = false;
				}
			});
		});

		let selected = super::settings_display(
			ui,
			self.selected_category,
			ProjectSettingCategory::iter(),
			|ui| {
				if let Some(category) = self.selected_category {
					category.ui(ui, &mut params, self);
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

#[derive(Reflect, Clone, Copy, EnumIter, Display, PartialEq, Eq)]
enum ProjectSettingCategory {
	Camera,
	Layouts,
}

impl ProjectSettingCategory {
	fn ui(
		self,
		ui: &mut egui::Ui,
		params: &mut ProjectSettingsUiParams<'_, '_>,
		settings_ui: &mut ProjectSettingsUi,
	) {
		match self {
			Self::Camera => {
				ui.horizontal(|ui| {
					ui.label("Editor Camera Mode:");

					for (text, state) in [
						("2D", ActiveEditorCamera::Cam2D),
						("3D", ActiveEditorCamera::Cam3D),
					] {
						ui.add_enabled_ui(*params.active_camera != state, |ui| {
							if ui.button(text).clicked() {
								*params.active_camera = state;
							}
						});
					}
				});
			}
			Self::Layouts => {
				ui.add_enabled_ui(!settings_ui.save_layout_dialog.open, |ui| {
					if ui.button("Save Layout").clicked() {
						settings_ui.save_layout_dialog.open = true;
					}
				});

				if !params.layout_manager.is_empty() {
					ui.add_enabled_ui(
						!settings_ui.save_layout_dialog.open && !settings_ui.reset_layout_dialog.open,
						|ui| {
							ui.menu_button("Restore", |ui| {
								for layout in params.layout_manager.iter() {
									if ui.button(layout).clicked() {
										params
											.commands
											.write_message(LoadLayoutMessage(layout.clone()));
									}
								}
							});
						},
					);
				}

				ui.add_enabled_ui(!settings_ui.reset_layout_dialog.open, |ui| {
					if ui.button("Restore Default").clicked() {
						settings_ui.reset_layout_dialog.open = true;
					}
				});

				ui.horizontal(|ui| {
					ui.label("Save On Exit");
					if ui.checkbox(&mut params.save_layout_on_exit, ()).clicked()
						&& let Err(err) = params
							.project_settings
							.set(SaveLayoutOnExitSetting, *params.save_layout_on_exit)
					{
						params
							.commands
							.trigger(Notification::error("Failed to save setting").with_context(err))
					}
				});
			}
		}
	}
}

#[derive(Message)]
struct SaveLayoutMessage(String);

impl SaveLayoutMessage {
	fn handle(
		mut reader: MessageReader<Self>,
		ui_manager: Res<UiManager>,
		mut layout_manager: ResMut<LayoutManager>,
		q_uuids: EditorInternalQuery<&PersistentId, Without<MissingUi>>,
		q_missing: EditorInternalQuery<&MissingUi>,
		mut settings: ProjectSettings,
	) {
		for msg in reader.read() {
			let dock = ui_manager
				.state()
				.decouple(&ui_manager, &q_uuids, &q_missing);
			if settings.set(SavedLayout::new(msg.0.clone()), dock).is_ok() {
				layout_manager.insert(msg.0.clone());
			}
		}
	}
}

#[derive(Message)]
struct LoadLayoutMessage(String);

impl LoadLayoutMessage {
	fn handle(
		mut reader: MessageReader<Self>,
		mut commands: Commands,
		mut settings: ProjectSettings,
	) -> Result {
		for msg in reader.read() {
			let layout_name = msg.0.clone();
			let dock = settings.get(SavedLayout::new(layout_name))?;
			commands.queue(LoadLayout(dock));
		}

		Ok(())
	}
}

#[derive(Message)]
struct ResetLayoutMessage;

impl ResetLayoutMessage {
	fn handle(mut reader: MessageReader<ResetLayoutMessage>, mut commands: Commands) {
		if reader.is_empty() {
			return;
		}

		reader.clear();

		commands.queue(|world: &mut World| {
			world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
				let default_state = ui_manager.default_dock_state(world);
				ui_manager.switch_state(default_state, world);
			});
		});
	}
}
