use crate::{
	RuntimeSettings,
	inspector::ui::TypeRegistryExtensions,
	ui::{EditorUi, builtin::inspector::InspectorSettings},
	util::log::LogLevel,
	view::cam::{RenderCameras, SyncRenderCamerasEvent},
};
use bevy::{diagnostic::DiagnosticsStore, ecs::system::SystemParam, prelude::*};
use bevy_egui::{EguiContext, egui};
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct DebugMenu {
	ui_debug_on_hover: bool,
	log_level: LogLevel,
}

impl DebugMenu {
	fn diagnostics(&self, ui: &mut egui::Ui, params: &Params) {
		egui::Grid::new("sys-diagnostics").show(ui, |ui| {
			for diagnostic in params.diagnostics.iter() {
				ui.label(diagnostic.path().as_str());
				if let Some(average) = diagnostic.average() {
					ui.label(format!("{:.2}", average));
				}
				ui.end_row();
			}
		});

		let ctx = ui.ctx().clone();
		ctx.inspection_ui(ui);
	}

	fn handle_ui_debug(event: On<DebugUiEvent>, mut q_egui_ctx: Query<&mut EguiContext>) {
		for mut ctx in &mut q_egui_ctx {
			let ctx = ctx.get_mut();
			ctx.set_debug_on_hover(event.0);
		}
	}
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	commands: Commands<'w, 's>,
	type_registry: Res<'w, AppTypeRegistry>,
	diagnostics: Res<'w, DiagnosticsStore>,
	render_cameras: ResMut<'w, RenderCameras>,
	editor_settings: ResMut<'w, RuntimeSettings>,
	inspector_settings: ResMut<'w, InspectorSettings>,
}

impl EditorUi for DebugMenu {
	const NAME: &str = "Debug Menu";
	const ID: uuid::Uuid = uuid!("9473f6e1-a595-41e2-8e29-a4f041580fa6");

	const UNIQUE: bool = true;

	type Params<'w, 's> = Params<'w, 's>;

	fn init(app: &mut App) {
		app.add_observer(Self::handle_ui_debug);
	}

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		Self::default()
	}

	fn ui(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
		self.diagnostics(ui, &params);

		ui.separator();

		if ui
			.checkbox(&mut self.ui_debug_on_hover, "Debug Editor UI")
			.clicked()
		{
			params
				.commands
				.trigger(DebugUiEvent(self.ui_debug_on_hover));
		}

		if ui
			.checkbox(&mut params.render_cameras, "Render Cameras")
			.clicked()
		{
			params.commands.trigger(SyncRenderCamerasEvent);
		}

		let _ = ui.checkbox(
			&mut params.inspector_settings.highlight_changes,
			"Highlight Component Changes",
		);

		let type_registry = params.type_registry.read();
		type_registry.ui_for_value(ui, &mut *params.editor_settings);
	}
}

#[derive(Event)]
struct DebugUiEvent(bool);
