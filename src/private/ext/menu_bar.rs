use crate::{
	EditorState, SimulationState,
	private::{
		EditorInternalQuery,
		ext::settings::{ProjectSettingsUi, ShowEditorSettings},
		scene::{EditorSceneRoot, LoadScene},
	},
	ui::{OpenMode, OpenUi},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use egui_file_dialog::FileDialog;
use egui_phosphor_icons::icons;
use uuid::Uuid;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	commands: Commands<'w, 's>,

	editor_state: Res<'w, State<EditorState>>,
	next_editor_state: ResMut<'w, NextState<EditorState>>,

	q_project_settings_ui: EditorInternalQuery<'w, 's, (), With<ProjectSettingsUi>>,

	file_dialog: Local<'s, FileDialog>,
}

#[derive(Resource, Reflect, Default)]
struct CachedSettings {
	save_layout_on_exit: bool,
	start_in_testing: bool,
}

pub fn render(ui: &mut egui::Ui, mut params: Params<'_, '_>) {
	params.file_dialog.update(ui.ctx());

	if let Some(file) = params.file_dialog.take_picked() {
		params.commands.queue(LoadScene::new(file));
	}

	egui::MenuBar::new().ui(ui, |ui| {
		file_menu(ui, &mut params);
		edit_menu(ui, &mut params);
		tools_menu(ui);
		view_menu(ui);
		game_control(ui, &mut params);
	});
}

fn file_menu(ui: &mut egui::Ui, params: &mut Params) {
	ui.menu_button("File", |ui| {
		if ui.button("New Scene").clicked() {
			params.commands.spawn(EditorSceneRoot);
		}

		if ui.button("Open Scene").clicked() {
			params.file_dialog.pick_file();
		}
	});
}

fn edit_menu(ui: &mut egui::Ui, params: &mut Params<'_, '_>) {
	ui.menu_button("Edit", |ui| {
		if ui.button("Editor Settings").clicked() {
			params.commands.write_message(ShowEditorSettings);
		}

		ui.add_enabled_ui(params.q_project_settings_ui.is_empty(), |ui| {
			if ui.button("Project Settings").clicked() {
				params
					.commands
					.queue(OpenUi::open::<ProjectSettingsUi>(OpenMode::Window));
			}
		});
	});
}

fn tools_menu(ui: &mut egui::Ui) {
	ui.menu_button("Tools", |ui| {
		if ui.button("Copy New UUID").clicked() {
			ui.output_mut(|output| {
				output
					.commands
					.push(egui::OutputCommand::CopyText(Uuid::new_v4().to_string()));
			});
		}
	});
}

fn view_menu(ui: &mut egui::Ui) {
	ui.menu_button("View", |ui| {
		let mut debug_on_hover = ui.ctx().debug_on_hover();
		if ui
			.checkbox(&mut debug_on_hover, "Debug Editor UI")
			.clicked()
		{
			ui.ctx().set_debug_on_hover(debug_on_hover);
		}
	});
}

fn game_control(ui: &mut egui::Ui, params: &mut Params) {
	match **params.editor_state {
		EditorState::Editing => {
			play_button(ui, params, EditorState::SimulationPrep);
		}
		EditorState::Simulating(SimulationState::Idle) => {
			play_button(ui, params, EditorState::Simulating(SimulationState::Live));
			stop_button(ui, params);
		}
		EditorState::Simulating(SimulationState::Live) => {
			pause_button(ui, params);
			stop_button(ui, params);
		}
		_ => (),
	}

	ui.menu_button(icons::DOTS_THREE_VERTICAL, |ui| {
		if ui.button("placeholder").clicked() {
			info!("placeholder");
		}
	});
}

fn play_button(ui: &mut egui::Ui, params: &mut Params, target_state: EditorState) {
	if ui.button(icons::PLAY).clicked() {
		params.next_editor_state.set(target_state);
	}
}

fn pause_button(ui: &mut egui::Ui, params: &mut Params) {
	if ui.button(icons::PAUSE).clicked() {
		params
			.next_editor_state
			.set(EditorState::Simulating(SimulationState::Idle));
	}
}

fn stop_button(ui: &mut egui::Ui, params: &mut Params) {
	if ui.button(icons::STOP).clicked() {
		params.next_editor_state.set(EditorState::Editing);
	}
}
