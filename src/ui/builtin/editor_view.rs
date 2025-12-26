use crate::{
	EditorUi,
	ui::{
		builtin::{
			BundleDnd,
			managed_view::{self, EditorManagedViewUi},
		},
		misc::UiState,
	},
	view::cam::EditorCamera,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::EguiContexts;
use transform_gizmo_bevy::{GizmoMode, GizmoOptions};
use uuid::uuid;

#[derive(Component, Reflect, Default)]
pub struct EditorViewUi;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	commands: Commands<'w, 's>,
	managed_view_params: managed_view::Params<'w, 's, EditorCamera>,
	managed_view: Local<'s, EditorManagedViewUi<EditorCamera>>,
	gizmo_options: ResMut<'w, GizmoOptions>,
	temporary: Option<Single<'w, 's, (Entity, Option<&'static Transform>), With<TemporaryEntity>>>,
}

impl EditorUi for EditorViewUi {
	const NAME: &str = "Edior View";

	const ID: uuid::Uuid = uuid!("c910a397-a017-4a29-99bc-6282b4b1a214");

	const CAN_CLEAR: bool = EditorManagedViewUi::<EditorCamera>::CAN_CLEAR;

	const UNIQUE: bool = EditorManagedViewUi::<EditorCamera>::UNIQUE;

	const POPOUT: bool = EditorManagedViewUi::<EditorCamera>::POPOUT;

	const SCROLL_BARS: [bool; 2] = EditorManagedViewUi::<EditorCamera>::SCROLL_BARS;

	type Params<'w, 's> = Params<'w, 's>;

	fn init(app: &mut App) {
		EditorManagedViewUi::<EditorCamera>::init(app);

		app
			.add_systems(FixedUpdate, detect_enter)
			.add_systems(Update, move_temporaries);
	}

	fn spawn(params: Self::Params<'_, '_>) -> Self {
		EditorManagedViewUi::<EditorCamera>::spawn(params.managed_view_params);
		default()
	}

	fn on_despawn(&mut self, mut params: Self::Params<'_, '_>) {
		params.managed_view.on_despawn(params.managed_view_params);
	}

	fn ui(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
		let window_rect = ui.clip_rect();

		let (_, Some(payload)) = super::panel_dnd_drop_ui::<BundleDnd, ()>(ui, |ui| {
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
		}) else {
			return;
		};

		let Some(temp) = params.temporary else {
			return;
		};
		let (temp_entity, transform) = *temp;

		// spawn a new entity in case it has its own picking rules

		params.commands.entity(temp_entity).despawn();

		let translation = transform.map(|t| t.translation);
		params.commands.queue(move |world: &mut World| {
			let new_entity = world.spawn_empty().id();
			payload.insert(std::iter::once(new_entity), world);
			let mut entity = world.entity_mut(new_entity);

			let Some((translation, mut new_transform)) = translation.zip(entity.get_mut::<Transform>())
			else {
				return;
			};

			new_transform.translation = translation;
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

fn move_temporaries(
	editor_camera: Single<Entity, With<EditorCamera>>,
	pointers: Query<&bevy::picking::pointer::PointerInteraction>,
	mut q_temporaries: Query<&mut Transform, With<TemporaryEntity>>,
) {
	for point in pointers
		.iter()
		.filter_map(|interaction| interaction.get_nearest_hit())
		.filter_map(|(_entity, hit)| (hit.camera == *editor_camera).then_some(hit.position))
		.flatten()
	{
		for mut transform in &mut q_temporaries {
			transform.translation = point;
		}
	}
}

fn detect_enter(
	mut commands: Commands,
	temporary: Option<Single<Entity, With<TemporaryEntity>>>,
	editor_view_state: Single<&UiState, With<EditorViewUi>>,
	mut hovered: Local<bool>,
	mut contexts: EguiContexts,
) {
	let Ok(ctx) = contexts.ctx_mut() else {
		return;
	};

	let is_hovered = editor_view_state.hovered();
	let hovered_before = *hovered;
	let entered = is_hovered && !hovered_before;
	let left = hovered_before && !is_hovered;
	*hovered = is_hovered;

	if entered && let Some(payload) = egui::DragAndDrop::take_payload::<BundleDnd>(ctx) {
		if let Some(temporary) = &temporary {
			commands.entity(**temporary).despawn();
		}

		commands.queue(move |world: &mut World| {
			let entity = world.spawn(TemporaryEntity).id();
			payload.insert(std::iter::once(entity), world);
			world.entity_mut(entity).insert(Pickable::IGNORE);
		});
	}

	if left && let Some(temporary) = &temporary {
		commands.entity(**temporary).despawn();
	}
}

#[derive(Component)]
struct TemporaryEntity;
