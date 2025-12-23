use crate::{
	EditorUi, Layouts, Notification,
	ui::{DockExtensions, LayoutManager, UiManager, misc::MissingUi, widgets},
	util::storage::{ProjectSettings, SaveLayoutOnExitSetting, StartEditorInTestingSetting},
	view::cam::ActiveEditorCamera,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use egui::TextBuffer;
use egui_dock::DockState;
use persistent_id::PersistentId;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct ProjectSettingsUi {
	selected_category: Option<ProjectSettingCategory>,
}

#[derive(SystemParam)]
pub struct ProjectSettingsUiParams<'w, 's> {
	commands: Commands<'w, 's>,
	project_settings: ProjectSettings<'w>,
	active_camera_state: Res<'w, State<ActiveEditorCamera>>,
	next_active_camera: ResMut<'w, NextState<ActiveEditorCamera>>,
	layout_manager: ResMut<'w, LayoutManager>,
	save_layout_dialog: Local<'s, widgets::Dialog>,
	reset_layout_dialog: Local<'s, widgets::Dialog>,
	layout_name: Local<'s, String>,

	save_layout_on_exit: Local<'s, bool>,
	start_in_testing: Local<'s, bool>,
}

impl EditorUi for ProjectSettingsUi {
	const NAME: &str = "Project Settings";

	const ID: uuid::Uuid = uuid!("a28755a1-ab68-44e6-b2b0-17cc14de0081");

	const HIDDEN: bool = true;

	type Params<'w, 's> = ProjectSettingsUiParams<'w, 's>;

	fn init(app: &mut App) {
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

	fn spawn(mut params: Self::Params<'_, '_>) -> Self {
		*params.save_layout_on_exit = params
			.project_settings
			.get_or::<SaveLayoutOnExitSetting>(true);

		*params.start_in_testing = params
			.project_settings
			.get_or_default::<StartEditorInTestingSetting>();

		params.save_layout_dialog.set_title("Save Layout");
		params.reset_layout_dialog.set_title("Reset Layout?");

		default()
	}

	fn render(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
		let selected = super::settings_display(
			ui,
			self.selected_category,
			ProjectSettingCategory::iter(),
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

#[derive(Reflect, Clone, Copy, EnumIter, Display, PartialEq, Eq)]
enum ProjectSettingCategory {
	Core,
	Camera,
	Layouts,
}

impl ProjectSettingCategory {
	fn ui(self, ui: &mut egui::Ui, params: &mut ProjectSettingsUiParams<'_, '_>) {
		params.save_layout_dialog.show(ui.ctx(), |ui, open| {
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
			});
		});

		params.reset_layout_dialog.show(ui.ctx(), |ui, open| {
			ui.label("This will reset your layout to the default configuration. Continue?");
			ui.horizontal(|ui| {
				if ui.button("Ok").clicked() {
					params.commands.write_message(ResetLayoutMessage);
					*open = false;
				}
			});
		});

		match self {
			Self::Core => {
				ui.label("Start In Testing");
				if ui.checkbox(&mut params.start_in_testing, ()).clicked()
					&& let Err(err) = params
						.project_settings
						.set::<StartEditorInTestingSetting>(*params.start_in_testing)
				{
					params
						.commands
						.trigger(Notification::error("Failed to save setting").with_context(err));
				}
			}
			Self::Camera => {
				ui.horizontal(|ui| {
					ui.label("Editor Camera Mode:");

					for (text, state) in [
						("2D", ActiveEditorCamera::Cam2D),
						("3D", ActiveEditorCamera::Cam3D),
					] {
						ui.add_enabled_ui(*params.active_camera_state.get() != state, |ui| {
							if ui.button(text).clicked() {
								params.next_active_camera.set(state);
							}
						});
					}
				});
			}
			Self::Layouts => {
				ui.add_enabled_ui(!params.save_layout_dialog.open, |ui| {
					if ui.button("Save Layout").clicked() {
						params.save_layout_dialog.open = true;
					}
				});

				if !params.layout_manager.is_empty() {
					ui.add_enabled_ui(
						!params.save_layout_dialog.open && !params.reset_layout_dialog.open,
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

				ui.add_enabled_ui(!params.reset_layout_dialog.open, |ui| {
					if ui.button("Restore Default").clicked() {
						params.reset_layout_dialog.open = true;
					}
				});

				ui.horizontal(|ui| {
					ui.label("Save On Exit");
					if ui.checkbox(&mut params.save_layout_on_exit, ()).clicked()
						&& let Err(err) = params
							.project_settings
							.set::<SaveLayoutOnExitSetting>(*params.save_layout_on_exit)
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
		mut commands: Commands,
		mut reader: MessageReader<Self>,
		ui_manager: Res<UiManager>,
		mut layout_manager: ResMut<LayoutManager>,
		q_uuids: Query<&PersistentId, Without<MissingUi>>,
		q_missing: Query<&MissingUi>,
		mut layouts: Layouts,
	) {
		for msg in reader.read() {
			let dock = ui_manager
				.state()
				.decouple(&ui_manager, &q_uuids, &q_missing);
			if let Err(err) = layouts.save_layout(&msg.0, dock) {
				commands.trigger(Notification::error("Failed to save layout").with_context(err));
			} else {
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
		mut layouts: Layouts,
	) -> Result {
		for msg in reader.read() {
			let layout_name = msg.0.clone();
			let dock = layouts.get_layout(layout_name)?;
			commands.queue(move |world: &mut World| {
				world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
					let new_state = DockState::restore(&dock, ui_manager.vtables(), world);
					ui_manager.switch_state(new_state, world);
				})
			});
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
