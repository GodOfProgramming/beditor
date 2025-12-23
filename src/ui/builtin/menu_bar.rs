use crate::{
	EditorState,
	ui::{
		InspectorSelection,
		builtin::settings::{EditorSettingsUi, ProjectSettingsUi},
		events::OpenSingleUiMessage,
	},
	view::cam::{ActiveEditorCamera, MoveCameraEvent, PointCameraEvent},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use uuid::Uuid;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	commands: Commands<'w, 's>,

	editor_state: Res<'w, State<EditorState>>,
	next_editor_state: ResMut<'w, NextState<EditorState>>,
	active_camera_state: Res<'w, State<ActiveEditorCamera>>,
	selection: Res<'w, InspectorSelection>,

	q_transforms: Query<'w, 's, &'static Transform>,

	q_editor_settings_ui: Query<'w, 's, (), With<EditorSettingsUi>>,
	q_project_settings_ui: Query<'w, 's, (), With<ProjectSettingsUi>>,
}

#[derive(Resource, Reflect, Default)]
struct CachedSettings {
	save_layout_on_exit: bool,
	start_in_testing: bool,
}

pub fn render(ui: &mut egui::Ui, mut params: Params<'_, '_>) {
	egui::MenuBar::new().ui(ui, |ui| {
		file_menu(ui);
		edit_menu(ui, &mut params);
		tools_menu(ui, &mut params);
		view_menu(ui, &mut params);
		game_control(ui, &mut params);
	});
}

fn file_menu(ui: &mut egui::Ui) {
	ui.menu_button("File", |_ui| {});
}

fn edit_menu(ui: &mut egui::Ui, params: &mut Params<'_, '_>) {
	ui.menu_button("Edit", |ui| {
		ui.add_enabled_ui(params.q_editor_settings_ui.is_empty(), |ui| {
			if ui.button("Editor Settings").clicked() {
				params
					.commands
					.write_message(OpenSingleUiMessage::new::<EditorSettingsUi>());
			}
		});

		ui.add_enabled_ui(params.q_project_settings_ui.is_empty(), |ui| {
			if ui.button("Project Settings").clicked() {
				params
					.commands
					.write_message(OpenSingleUiMessage::new::<ProjectSettingsUi>());
			}
		});
	});
}

fn tools_menu(ui: &mut egui::Ui, params: &mut Params) {
	ui.menu_button("Tools", |ui| {
		if ui.button("Spawn Empty Entity").clicked() {
			params.commands.spawn_empty();
		}

		if ui.button("Copy New UUID").clicked() {
			ui.output_mut(|output| {
				output
					.commands
					.push(egui::OutputCommand::CopyText(Uuid::new_v4().to_string()));
			});
		}
	});
}

fn view_menu(ui: &mut egui::Ui, params: &mut Params) {
	ui.menu_button("View", |ui| {
		camera_menu(ui, params);
	});
}

fn game_control(ui: &mut egui::Ui, params: &mut Params) {
	match **params.editor_state {
		EditorState::Editing => {
			play_button(ui, params);
		}
		EditorState::Testing => {
			pause_button(ui, params);
		}
		_ => (),
	}
}

fn camera_menu(ui: &mut egui::Ui, params: &mut Params) {
	ui.menu_button("Camera", |ui| {
		if *params.editor_state == EditorState::Editing {
			if *params.active_camera_state == ActiveEditorCamera::Cam3D {
				look_at_origin_button(ui, params);
			}

			entity_commands(ui, params);
		}
	});
}

fn look_at_origin_button(ui: &mut egui::Ui, params: &mut Params) {
	if ui.button("Look At Origin").clicked() {
		params.commands.trigger(PointCameraEvent::new(Vec3::ZERO));
	}
}

fn entity_commands(ui: &mut egui::Ui, params: &mut Params) {
	let InspectorSelection::Entities(selected_entities) = &*params.selection else {
		return;
	};

	let Some(entity) = (selected_entities.len() == 1)
		.then(|| selected_entities.iter().next())
		.flatten()
	else {
		return;
	};

	if matches!(
		**params.active_camera_state,
		ActiveEditorCamera::Cam2D | ActiveEditorCamera::Cam3D
	) {
		move_to_target_button(ui, params, entity);

		if *params.active_camera_state == ActiveEditorCamera::Cam3D {
			look_at_target_button(ui, params, entity);
		}
	}
}

fn move_to_target_button(ui: &mut egui::Ui, params: &mut Params, entity: Entity) {
	if ui.button("Move To Selected").clicked() {
		let Ok(entity_pos) = params.q_transforms.get(entity).map(|t| t.translation) else {
			return;
		};

		params.commands.trigger(MoveCameraEvent::new(entity_pos));
	}
}

fn look_at_target_button(ui: &mut egui::Ui, params: &mut Params, entity: Entity) {
	if ui.button("Look At Selected").clicked() {
		let Ok(entity_pos) = params.q_transforms.get(entity).map(|t| t.translation) else {
			return;
		};

		params.commands.trigger(PointCameraEvent::new(entity_pos));
	}
}

fn play_button(ui: &mut egui::Ui, params: &mut Params) {
	if ui.button("▶").clicked() {
		params.next_editor_state.set(EditorState::Testing);
	}
}

fn pause_button(ui: &mut egui::Ui, params: &mut Params) {
	if ui.button("⏸").clicked() {
		params.next_editor_state.set(EditorState::Editing);
	}
}
