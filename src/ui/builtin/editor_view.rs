use crate::{
	EditorUi,
	ui::builtin::managed_view::{self, EditorManagedView},
	view::cam::EditorCamera,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use transform_gizmo_bevy::{GizmoMode, GizmoOptions};
use uuid::uuid;

#[derive(Component, Reflect, Default)]
pub struct EditorView {}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	managed_view_params: managed_view::Params<'w, 's, EditorCamera>,
	managed_view: Local<'s, EditorManagedView<EditorCamera>>,
	gizmo_options: ResMut<'w, GizmoOptions>,
}

impl EditorUi for EditorView {
	const NAME: &str = "Edior View";

	const ID: uuid::Uuid = uuid!("c910a397-a017-4a29-99bc-6282b4b1a214");

	const CAN_CLEAR: bool = EditorManagedView::<EditorCamera>::CAN_CLEAR;

	const UNIQUE: bool = EditorManagedView::<EditorCamera>::UNIQUE;

	const POPOUT: bool = EditorManagedView::<EditorCamera>::POPOUT;

	const SCROLL_BARS: [bool; 2] = EditorManagedView::<EditorCamera>::SCROLL_BARS;

	type Params<'w, 's> = Params<'w, 's>;

	fn init(app: &mut App) {
		EditorManagedView::<EditorCamera>::init(app);
	}

	fn spawn(params: Self::Params<'_, '_>) -> Self {
		EditorManagedView::<EditorCamera>::spawn(params.managed_view_params);
		default()
	}

	fn on_despawn(&mut self, mut params: Self::Params<'_, '_>) {
		params.managed_view.on_despawn(params.managed_view_params);
	}

	fn ui(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
		let window_rect = ui.clip_rect();

		let has_camera = params.managed_view_params.has_camera();
		params.managed_view.ui(ui, params.managed_view_params);

		if !has_camera {
			return;
		}

		let margin = ui.style().spacing.window_margin;
		let outer_ui = egui::Rect::from_min_max(
			window_rect.min + egui::vec2(margin.leftf(), margin.topf()),
			window_rect.max - egui::vec2(margin.rightf(), margin.bottomf()),
		);

		ui.scope_builder(egui::UiBuilder::new().max_rect(outer_ui), |ui| {
			let style = ui.style_mut();
			style.spacing.window_margin = egui::Margin::same(6);

			ui.horizontal(|ui| {
				let only_selecting = params.gizmo_options.gizmo_modes.is_empty();
				if ui
					.selectable_label(only_selecting, egui_phosphor_icons::icons::CURSOR)
					.clicked()
				{
					params.gizmo_options.gizmo_modes.clear();
				}

				for (set, icon) in [
					(
						GizmoMode::all_translate(),
						egui_phosphor_icons::icons::ARROWS_OUT_CARDINAL,
					),
					(
						GizmoMode::all_rotate(),
						egui_phosphor_icons::icons::ARROWS_CLOCKWISE,
					),
					(
						GizmoMode::all_scale(),
						egui_phosphor_icons::icons::ARROW_SQUARE_OUT,
					),
				] {
					let enabled = set.is_subset(params.gizmo_options.gizmo_modes);
					if ui.selectable_label(enabled, icon).clicked() {
						if enabled {
							params.gizmo_options.gizmo_modes.remove_all(set);
						} else {
							params.gizmo_options.gizmo_modes.insert_all(set);
						}
					}
				}
			});
		});
	}

	fn context_menu(
		&mut self,
		ui: &mut egui::Ui,
		mut params: Self::Params<'_, '_>,
		surface: egui_dock::SurfaceIndex,
		node: egui_dock::NodeIndex,
	) {
		params
			.managed_view
			.context_menu(ui, params.managed_view_params, surface, node);
	}
}
