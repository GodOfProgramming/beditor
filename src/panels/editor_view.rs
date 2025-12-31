use crate::{
	EditorExtension, EditorState, EditorUi,
	panels::{
		BundleDnd,
		managed_view::{self, EditorManagedViewUi},
	},
	private::{
		EditorInternal, EditorInternalFilter, EditorInternalQuery, EditorInternalSingle, UserHidden,
		cam::EditorCamera,
		scene::PrimaryScene,
		ui::{EditorEguiContext, misc::UiState},
	},
	util::WorldExtensions as _,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::EguiContext;
use egui_phosphor_icons::icons;
use singleton::{SingletonBehavior, SingletonPlugin};
use transform_gizmo_bevy::{GizmoMode, GizmoOptions};
use uuid::uuid;

#[derive(Default)]
pub struct EditorViewUiExtension;

impl EditorExtension for EditorViewUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<EditorViewUi>();
	}

	fn build_app(&self, app: &mut App) {
		app
			.add_plugins(
				SingletonPlugin::<TemporaryEntity, EditorInternalFilter>::new(
					SingletonBehavior::RemoveOther,
				),
			)
			.add_systems(OnExit(EditorState::Editing), despawn_temporaries)
			.add_systems(
				FixedUpdate,
				detect_enter.run_if(in_state(EditorState::Editing)),
			)
			.add_systems(
				Update,
				move_temporaries.run_if(in_state(EditorState::Editing)),
			);
	}
}

#[derive(Component, Reflect, Default)]
#[require(EditorInternal)]
pub struct EditorViewUi;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	commands: Commands<'w, 's>,
	managed_view_params: managed_view::Params<'w, 's, EditorCamera>,
	managed_view: Local<'s, EditorManagedViewUi<EditorCamera>>,
	gizmo_options: ResMut<'w, GizmoOptions>,
	temporary: Option<
		EditorInternalSingle<'w, 's, (Entity, Option<&'static Transform>), With<TemporaryEntity>>,
	>,

	editor_camera:
		Option<EditorInternalSingle<'w, 's, (Has<Camera2d>, Has<Camera3d>), With<EditorCamera>>>,
}

impl EditorUi for EditorViewUi {
	const NAME: &str = "Edior View";

	const ID: uuid::Uuid = uuid!("c910a397-a017-4a29-99bc-6282b4b1a214");

	const CAN_CLEAR: bool = EditorManagedViewUi::<EditorCamera>::CAN_CLEAR;

	const UNIQUE: bool = EditorManagedViewUi::<EditorCamera>::UNIQUE;

	const POPOUT: bool = EditorManagedViewUi::<EditorCamera>::POPOUT;

	const SCROLL_BARS: [bool; 2] = EditorManagedViewUi::<EditorCamera>::SCROLL_BARS;

	type Params<'w, 's> = Params<'w, 's>;

	fn spawn(params: Self::Params<'_, '_>) -> Self {
		let _ = EditorManagedViewUi::<EditorCamera>::spawn(params.managed_view_params);
		default()
	}

	fn on_despawn(&mut self, mut params: Self::Params<'_, '_>) {
		params.managed_view.on_despawn(params.managed_view_params);
	}

	fn ui(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Params {
			mut commands,
			mut managed_view_params,
			mut managed_view,
			mut gizmo_options,
			temporary,
			editor_camera,
		} = params;

		let window_rect = ui.clip_rect();

		let (_, Some(payload)) = super::panel_dnd_drop_ui::<BundleDnd, ()>(ui, |ui| {
			let has_camera = managed_view_params.has_camera();

			managed_view.ui(ui, managed_view_params);

			if !has_camera {
				return;
			}

			let margin = ui.style().spacing.window_margin;
			let outer_ui = egui::Rect::from_min_max(
				window_rect.min + egui::vec2(margin.leftf(), margin.topf()),
				window_rect.max - egui::vec2(margin.rightf(), margin.bottomf()),
			);

			ui.scope_builder(egui::UiBuilder::new().max_rect(outer_ui), |ui| {
				let Some(editor_camera) = editor_camera else {
					ui.label("No editor camera");
					return;
				};

				let (is_2d, is_3d) = *editor_camera;

				match (is_2d, is_3d) {
					(true, false) => {
						overlay2d();
					}
					(false, true) => {
						overlay3d(ui, &mut gizmo_options);
					}
					(false, false) => {
						ui.label("No camera kind");
					}
					(true, true) => {
						ui.label("Multiple cameras registered");
					}
				}
			});
		}) else {
			return;
		};

		let Some(temp) = temporary else {
			return;
		};
		let (temp_entity, transform) = *temp;

		// spawn a new entity in case it has its own picking rules

		commands.entity(temp_entity).despawn();

		let translation = transform.map(|t| t.translation);
		commands.queue(move |world: &mut World| {
			let Some(new_entity) = world.spawn_stateful_entity() else {
				return;
			};

			payload.insert(std::iter::once(new_entity), world);

			'make_child: {
				let mut query = world.query_filtered::<Entity, EditorInternalFilter<With<PrimaryScene>>>();
				let Ok(root_entity) = query.query_mut(world).single() else {
					break 'make_child;
				};

				world.entity_mut(new_entity).insert(ChildOf(root_entity));
			}

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

#[derive(Component)]
#[require(UserHidden)]
struct TemporaryEntity;

fn move_temporaries(
	editor_camera: EditorInternalSingle<Entity, With<EditorCamera>>,
	pointers: Query<&bevy::picking::pointer::PointerInteraction>,
	mut q_temporaries: EditorInternalQuery<&mut Transform, With<TemporaryEntity>>,
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
	temporary: Option<EditorInternalSingle<Entity, With<TemporaryEntity>>>,
	editor_view_state: EditorInternalSingle<&UiState, With<EditorViewUi>>,
	mut hovered: Local<bool>,
	mut context: EditorInternalSingle<&mut EguiContext, With<EditorEguiContext>>,
) {
	let ctx = context.get_mut();

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

fn despawn_temporaries(
	mut commands: Commands,
	q_temporaries: Query<Entity, With<TemporaryEntity>>,
) {
	for entity in &q_temporaries {
		commands.entity(entity).despawn();
	}
}

fn overlay2d() {}

fn overlay3d(ui: &mut egui::Ui, gizmo_options: &mut GizmoOptions) {
	let style = ui.style_mut();
	style.spacing.window_margin = egui::Margin::same(6);
	style.spacing.item_spacing.x = 6.0;

	ui.horizontal(|ui| {
		if ui
			.selectable_label(gizmo_options.snapping, icons::MAGNET)
			.clicked()
		{
			gizmo_options.snapping ^= true;
		}

		if gizmo_options.snapping {
			ui.scope(|ui| {
				ui.style_mut().spacing.item_spacing.x = 0.0;
				ui.add_enabled(false, egui::Button::selectable(false, icons::GRID_NINE));
				ui.add(egui::DragValue::new(&mut gizmo_options.snap_distance));
			});

			ui.scope(|ui| {
				ui.style_mut().spacing.item_spacing.x = 0.0;
				ui.add_enabled(false, egui::Button::selectable(false, icons::ANGLE));
				ui.drag_angle(&mut gizmo_options.snap_angle);
			});

			ui.scope(|ui| {
				ui.style_mut().spacing.item_spacing.x = 0.0;
				ui.add_enabled(
					false,
					egui::Button::selectable(false, icons::ARROWS_OUT_SIMPLE),
				);
				ui.add(egui::DragValue::new(&mut gizmo_options.snap_scale));
			});
		}

		ui.scope(|ui| {
			ui.style_mut().spacing.item_spacing.x = 0.0;

			let only_selecting = gizmo_options.gizmo_modes.is_empty();
			if ui.selectable_label(only_selecting, icons::CURSOR).clicked() {
				gizmo_options.gizmo_modes.clear();
			}

			for (set, icon) in [
				(GizmoMode::all_translate(), icons::ARROWS_OUT_CARDINAL),
				(GizmoMode::all_rotate(), icons::ARROWS_CLOCKWISE),
				(GizmoMode::all_scale(), icons::ARROW_SQUARE_OUT),
			] {
				let enabled = set.is_subset(gizmo_options.gizmo_modes);
				if ui.selectable_label(enabled, icon).clicked() {
					if enabled {
						gizmo_options.gizmo_modes.remove_all(set);
					} else {
						gizmo_options.gizmo_modes.insert_all(set);
					}
				}
			}
		});
	});
}
