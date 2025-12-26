use crate::{
	settings::{CurrentLayoutSetting, EditorEguiSettings, SaveLayoutOnExitSetting},
	ui::{
		EditorUiCamera, EditorUiHitCaptureNode, SavedLayout, UiManager,
		misc::{MissingUi, UiState},
	},
	util::storage::{GlobalEditorSettings, ProjectSettings},
};
use bevy::{prelude::*, ui::FocusPolicy};
use bevy_egui::PrimaryEguiContext;
use persistent_id::PersistentId;

pub fn startup(mut commands: Commands) {
	commands.spawn((Name::new("Editor UI Camera"), EditorUiCamera));
	commands.spawn((
		Name::new("Editor Ui Pointer Capture"),
		EditorUiHitCaptureNode,
		FocusPolicy::Pass,
		Node {
			width: vw(100),
			height: vh(100),
			..default()
		},
	));
}

pub fn on_new_ctx(
	event: On<Add, PrimaryEguiContext>,
	mut q_ctx: Query<
		(
			&mut bevy_egui::EguiContext,
			&mut bevy_egui::EguiContextSettings,
		),
		Added<PrimaryEguiContext>,
	>,
	mut settings: GlobalEditorSettings,
) {
	let Ok((mut ctx, mut ctx_settings)) = q_ctx.get_mut(event.event_target()) else {
		return;
	};

	let ctx = ctx.get_mut();

	let mut fonts = egui::FontDefinitions::default();
	egui_phosphor_icons::add_fonts(&mut fonts);
	ctx.set_fonts(fonts.clone());

	if let Ok(options) = settings.get(EditorEguiSettings) {
		ctx.options_mut(|opts| {
			// corrects this running after the other that deals with themes
			let dark = opts.dark_style.clone();
			let light = opts.light_style.clone();
			*opts = options;
			opts.dark_style = dark;
			opts.light_style = light;
		});
	}

	ctx_settings.capture_pointer_input = false;
}

pub fn reset_ui_state(mut q_ui_infos: Query<&mut UiState>) {
	q_ui_infos.par_iter_mut().for_each(|mut ui_info| {
		ui_info.hovered = false;
	});
}

pub fn render(world: &mut World) {
	world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
		ui_manager.ui(world);
	});
}

pub fn on_app_exit(
	ui_manager: Res<UiManager>,
	q_uuids: Query<&PersistentId, Without<MissingUi>>,
	q_missing: Query<&MissingUi>,
	mut settings: ProjectSettings,
) -> Result {
	let save_on_exit = settings.get(SaveLayoutOnExitSetting).unwrap_or(true);
	let current_layout = if save_on_exit {
		let name = match settings.get(CurrentLayoutSetting).ok() {
			Some(opt) => opt,
			None => {
				let default_layout = String::from("default");
				settings.set(CurrentLayoutSetting, default_layout.clone())?;
				default_layout
			}
		};

		Some(name)
	} else {
		None
	};

	if let Some(name) = current_layout {
		let new_state = ui_manager.save_state(&q_uuids, &q_missing);
		settings.set(SavedLayout(name), new_state)?;
	}

	Ok(())
}
