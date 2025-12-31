pub mod cam2d;
pub mod cam3d;
pub mod commands;

use crate::{
	EditorState,
	panels::editor_view::EditorViewUi,
	private::{
		EditorInternalFilter, EditorInternalQuery, EditorInternalSingle, EditorOwned, input,
		ui::{EditorUiHitCaptureNode, misc::UiState},
	},
	settings::{ActiveEditorCameraSetting, RenderCamerasSetting},
	util::storage::ProjectSettings,
};
use bevy::{
	camera::{
		NormalizedRenderTarget, RenderTarget,
		visibility::{Layer, RenderLayers},
	},
	color::palettes::tailwind,
	ecs::system::SystemState,
	picking::{
		PickingSystems,
		hover::HoverMap,
		pointer::{Location, PointerId, PointerInput},
	},
	prelude::*,
	render::render_resource::TextureFormat,
	ui::FocusPolicy,
};
use bevy_axes_gizmo::{AxesGizmoSyncCamera, AxesGizmoTexture};
use bevy_egui::PrimaryEguiContext;
use cam2d::Cam2dSystems;
use cam3d::Cam3dSystems;
use commands::{LookAt, MoveTo};
use derive_more::derive::Deref;
use macros::Identifiable;
use serde::{Deserialize, Serialize};
use singleton::{SingletonBehavior, SingletonPlugin};
use smallvec::SmallVec;
use transform_gizmo_bevy::{GizmoCamera, GizmoOptions};
use uuid::Uuid;

pub const EDITOR_UI_RENDER_LAYER: Layer = 31;
pub const EDITOR_VIEW_RENDER_LAYER: Layer = 30;

pub struct EditorCamPlugin;

impl Plugin for EditorCamPlugin {
	fn build(&self, app: &mut App) {
		let mut system_state = SystemState::<ProjectSettings>::new(app.world_mut());
		let mut settings = system_state.get_mut(app.world_mut());
		let active_cam_state = settings.get(ActiveEditorCameraSetting).unwrap_or_default();

		app
			.add_plugins((
				SingletonPlugin::<EditorCamera, EditorInternalFilter>::new(SingletonBehavior::RemoveOther),
				SingletonPlugin::<EditorUiCamera, EditorInternalFilter>::new(
					SingletonBehavior::RemoveOther,
				),
			))
			.configure_sets(
				Update,
				(
					CameraInputSystems::Mouse.run_if(mouse_hovered_in_editor_view),
					Cam2dSystems.run_if(in_state(ActiveEditorCamera::Cam2D)),
					Cam3dSystems.run_if(in_state(ActiveEditorCamera::Cam3D)),
					OrbitSystems.run_if(in_state(OrbitState::Active)),
					PanSystems.run_if(in_state(PanState::Active)),
					CameraInputSystems::Keyboard
						.in_set(input::Unfocused)
						.run_if(mouse_movement_active),
				),
			)
			.insert_state(active_cam_state)
			.insert_state(OrbitState::Inactive)
			.insert_state(PanState::Inactive)
			.init_resource::<RenderCameras>()
			.init_resource::<GameCameraColor>()
			.add_observer(LookAt::handle)
			.add_observer(MoveTo::handle)
			.add_observer(on_manage_camera)
			.add_observer(on_unmanage_camera)
			.add_systems(OnEnter(ActiveEditorCamera::Cam2D), cam2d::enable)
			.add_systems(OnExit(ActiveEditorCamera::Cam2D), cam2d::save_settings)
			.add_systems(OnEnter(ActiveEditorCamera::Cam3D), cam3d::enable)
			.add_systems(OnExit(ActiveEditorCamera::Cam3D), cam3d::save_settings)
			.add_systems(Startup, (startup, retrieve_show_cameras_value))
			.add_systems(PostStartup, init_camera)
			.add_systems(
				OnEnter(EditorState::Exiting),
				(cam2d::save_settings, cam3d::save_settings),
			)
			.add_systems(
				First,
				(
					spawn_axis_ui,
					(
						(
							EditorManagedCamera::viewport_picking.in_set(PickingSystems::PostInput),
							EditorManagedCamera::sync_gizmos,
						),
						EditorManagedCamera::on_frame_end,
					)
						.chain(),
				),
			)
			.add_systems(
				FixedUpdate,
				track_editor_camera_changes.run_if(state_changed::<ActiveEditorCamera>),
			)
			.add_systems(
				Update,
				(
					render_2d_cameras
						.in_set(Cam2dSystems)
						.run_if(should_show_cameras),
					render_3d_cameras
						.in_set(Cam3dSystems)
						.run_if(should_show_cameras),
				),
			)
			.add_systems(
				Update,
				(
					(
						cam2d::released_mouse_input_actions,
						cam2d::mouse_input_actions,
						(cam2d::pan_system.in_set(PanSystems), cam2d::zoom_system),
					)
						.chain()
						.in_set(CameraInputSystems::Mouse),
					cam2d::movement_system.in_set(CameraInputSystems::Keyboard),
				)
					.chain()
					.in_set(Cam2dSystems),
			)
			.add_systems(
				Update,
				(
					(
						cam3d::released_mouse_input_actions,
						cam3d::mouse_input_actions,
						(
							cam3d::orbit_system.in_set(OrbitSystems),
							cam3d::pan_system.in_set(PanSystems),
							cam3d::zoom_system,
						),
					)
						.chain()
						.in_set(CameraInputSystems::Mouse),
					cam3d::movement_system.in_set(CameraInputSystems::Keyboard),
				)
					.chain()
					.in_set(Cam3dSystems),
			);
	}
}

fn init_camera(
	mut settings: ProjectSettings,
	mut next_state: ResMut<NextState<ActiveEditorCamera>>,
) {
	let state = settings.get(ActiveEditorCameraSetting).unwrap_or_default();
	next_state.set(state);
}

fn on_manage_camera(
	event: On<Add, EditorManagedCamera>,
	mut contexts: bevy_egui::EguiContexts,
	mut images: ResMut<Assets<Image>>,
	mut q_cameras: EditorInternalQuery<&mut Camera>,
) {
	let Ok(mut camera) = q_cameras.get_mut(event.event_target()) else {
		return;
	};

	let image_handle = if let RenderTarget::Image(render_target) = &camera.target {
		render_target.handle.clone()
	} else {
		let image = Image::new_target_texture(1, 1, TextureFormat::bevy_default());
		let handle = images.add(image);
		camera.target = RenderTarget::Image(handle.clone().into());
		handle
	};

	contexts.add_image(bevy_egui::EguiTextureHandle::Weak(image_handle.id()));
}

fn on_unmanage_camera(
	event: On<Remove, EditorManagedCamera>,
	q_cameras: EditorInternalQuery<&Camera>,
	mut contexts: bevy_egui::EguiContexts,
) {
	if let Ok(camera) = q_cameras.get(event.event_target())
		&& let RenderTarget::Image(image) = &camera.target
	{
		contexts.remove_image(image.handle.id());
	}
}

fn spawn_axis_ui(
	q_new_editor_cameras: EditorInternalQuery<Entity, Added<EditorCamera>>,
	mut commands: Commands,
	axes_gizmo_image: Res<AxesGizmoTexture>,
) {
	for editor_camera in &q_new_editor_cameras {
		commands.spawn((
			Name::new("Axis Image"),
			EditorOwned,
			Pickable::IGNORE,
			FocusPolicy::Pass,
			UiTargetCamera(editor_camera),
			EditorCameraUi(editor_camera),
			BackgroundColor(Color::NONE),
			Node {
				position_type: PositionType::Absolute,
				left: px(0),
				bottom: px(0),
				width: vmin(20),
				height: vmin(20),
				..default()
			},
			ImageNode {
				image: axes_gizmo_image.0.clone(),
				..default()
			},
		));
	}
}

#[derive(SystemSet, PartialEq, Eq, Hash, Clone, Debug)]
enum CameraInputSystems {
	Keyboard,
	Mouse,
}

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct OrbitSystems;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
enum OrbitState {
	Active,
	Inactive,
}

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct PanSystems;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
enum PanState {
	Active,
	Inactive,
}

#[derive(Default, Component, Reflect, Identifiable)]
#[require(
  EditorOwned,
  MeshPickingCamera,
  EditorManagedCamera,
  AxesGizmoSyncCamera = AxesGizmoSyncCamera,
  RenderLayers = RenderLayers::from_layers(&[0, EDITOR_VIEW_RENDER_LAYER]),
)]
#[id("00000000-0000-0000-0000-000000000000")]
pub struct EditorCamera;

#[derive(Default, Component, Reflect)]
#[require(
  Camera2d,
  Camera = Camera { order: isize::MAX, ..default() },
  PrimaryEguiContext = PrimaryEguiContext,
  RenderLayers = RenderLayers::layer(EDITOR_UI_RENDER_LAYER),
  EditorOwned,
)]
pub struct EditorUiCamera;

#[derive(Component)]
#[relationship_target(relationship = EditorCameraUi, linked_spawn)]
struct EditorCameraUis(Vec<Entity>);

#[derive(Component)]
#[relationship(relationship_target = EditorCameraUis)]
struct EditorCameraUi(Entity);

#[derive(Component, Default)]
#[require(
  Camera,
  PointerId = PointerId::Custom(Uuid::new_v4()),
)]
pub struct EditorManagedCamera {
	context_menu_opened: bool,
	hovered: bool,
	viewport_rect: Option<Rect>,
	last_viewport: Option<Rect>,
	ignore_size_mismatch: bool,
}

impl EditorManagedCamera {
	pub fn set_ctx_menu_open(&mut self, open: bool) {
		self.context_menu_opened = open;
	}

	pub fn set_hovered(&mut self, hovered: bool) {
		self.hovered = hovered;
	}

	pub fn viewport(&self) -> Option<Rect> {
		self.last_viewport
	}

	pub fn set_viewport(&mut self, rect: Rect) {
		self.viewport_rect = Some(rect);
	}

	pub fn should_sync_to_viewport(&self) -> bool {
		!self.ignore_size_mismatch
	}

	pub fn ignore_viewport_size(&mut self) {
		self.ignore_size_mismatch = true;
	}

	pub fn sync_viewport_size(&mut self) {
		self.ignore_size_mismatch = false;
	}

	fn viewport_picking(
		mut commands: Commands,
		q_managed_cameras: EditorInternalQuery<(&Camera, &PointerId, &Self)>,
		ui_hit_node: EditorInternalSingle<Entity, With<EditorUiHitCaptureNode>>,
		hover_map: Res<HoverMap>,
		mut pointer_inputs: MessageReader<PointerInput>,
	) {
		let node_pointers = hover_map.iter().flat_map(|(pointer_id, hits)| {
			hits.keys().filter_map(|entity| {
				if *entity == *ui_hit_node {
					Some(*pointer_id)
				} else {
					None
				}
			})
		});

		let inputs = pointer_inputs.read().collect::<SmallVec<[_; 4]>>();

		let filtered_inputs = node_pointers
			.flat_map(|node_pointer_id| {
				inputs
					.iter()
					.filter(move |input| input.pointer_id == node_pointer_id)
			})
			.collect::<SmallVec<[_; 2]>>();

		let iter = q_managed_cameras
			.iter()
			.filter_map(|(camera, managed_camera_pointer_id, managed_camera)| {
				if !managed_camera.hovered || managed_camera.context_menu_opened {
					None
				} else {
					managed_camera
						.viewport_rect
						.zip(camera.target.as_image())
						.map(|(viewport_rect, target)| (target, viewport_rect, managed_camera_pointer_id))
				}
			})
			.flat_map(|(target, viewport_rect, managed_camera_pointer_id)| {
				filtered_inputs
					.iter()
					.map(move |input| (target, viewport_rect, managed_camera_pointer_id, input))
			});

		for (target, viewport_rect, managed_camera_pointer_id, input) in iter {
			let location = Location {
				position: input.location.position - viewport_rect.min,
				target: NormalizedRenderTarget::Image(target.clone().into()),
			};

			let msg = PointerInput {
				pointer_id: *managed_camera_pointer_id,
				location,
				action: input.action,
			};

			commands.write_message(msg);
		}
	}

	fn sync_gizmos(
		gizmo_camera: EditorInternalSingle<&Self, With<GizmoCamera>>,
		mut gizmos_options: ResMut<GizmoOptions>,
	) {
		gizmos_options.viewport_rect = gizmo_camera.viewport_rect;
	}

	fn on_frame_end(mut q_managed_cameras: EditorInternalQuery<&mut Self>) {
		for mut cam in &mut q_managed_cameras {
			cam.last_viewport = cam.viewport_rect.take();
			cam.set_ctx_menu_open(false);
			cam.hovered = false;
		}
	}
}

#[derive(
	Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default, Serialize, Deserialize, Reflect,
)]
pub enum ActiveEditorCamera {
	Cam2D,
	#[default]
	Cam3D,
}

impl ActiveEditorCamera {
	pub fn is_active(&self) -> bool {
		matches!(self, Self::Cam2D | Self::Cam3D)
	}

	pub fn is_2d(&self) -> bool {
		*self == Self::Cam2D
	}

	pub fn is_3d(&self) -> bool {
		*self == Self::Cam3D
	}
}

#[derive(Resource, Reflect, Deref)]
#[reflect(Resource, Default)]
pub struct GameCameraColor(Color);

impl Default for GameCameraColor {
	fn default() -> Self {
		Self(tailwind::GREEN_700.into())
	}
}

#[derive(Resource, Reflect, Default, Deref, DerefMut)]
#[reflect(Resource, Default)]
pub struct RenderCameras(bool);

fn track_editor_camera_changes(
	cam_state: Res<State<ActiveEditorCamera>>,
	mut settings: ProjectSettings,
) -> Result {
	settings.set(ActiveEditorCameraSetting, **cam_state)
}

fn startup(mut commands: Commands) {
	commands.spawn((Name::new("Editor UI Camera"), EditorUiCamera));
}

pub fn mouse_hovered_in_editor_view(
	q_editor_view_ui_state: EditorInternalQuery<&UiState, With<EditorViewUi>>,
) -> bool {
	q_editor_view_ui_state.iter().any(UiState::hovered)
}

fn mouse_movement_active(orbit: Res<State<OrbitState>>, pan: Res<State<PanState>>) -> bool {
	*orbit == OrbitState::Active || *pan == PanState::Active
}

fn retrieve_show_cameras_value(
	mut render_cameras: ResMut<RenderCameras>,
	mut settings: ProjectSettings,
) {
	**render_cameras = settings.get(RenderCamerasSetting).unwrap_or_default();
}

pub fn should_show_cameras(render_cameras: Res<RenderCameras>) -> bool {
	**render_cameras
}

#[allow(clippy::type_complexity)]
pub fn render_2d_cameras(
	mut gizmos: Gizmos,
	q_app_cameras: Query<
		(&Transform, &Projection),
		(
			With<Camera2d>,
			With<EditorManagedCamera>,
			// to support this in both dev & user mode
			Without<EditorCamera>,
		),
	>,
	cam_color: Res<GameCameraColor>,
) {
	// TODO render only to editor camera
	for (transform, projection) in &q_app_cameras {
		if let Projection::Orthographic(ortho) = projection {
			let rect_pos = transform.translation;
			gizmos.rect(rect_pos, ortho.area.max - ortho.area.min, **cam_color);
		}
	}
}

#[allow(clippy::type_complexity)]
pub fn render_3d_cameras(
	mut gizmos: Gizmos,
	q_app_cameras: Query<
		(&Transform, &Projection),
		(
			With<Camera3d>,
			With<EditorManagedCamera>,
			// to support this in both dev & user mode
			Without<EditorCamera>,
		),
	>,
	cam_color: Res<GameCameraColor>,
) {
	// TODO render only to editor camera
	for (transform, projection) in &q_app_cameras {
		match projection {
			Projection::Perspective(perspective) => {
				render_3d_camera(
					*transform,
					perspective.aspect_ratio,
					&mut gizmos,
					&cam_color,
				);
			}
			Projection::Orthographic(orthographic) => {
				render_3d_camera(*transform, orthographic.scale, &mut gizmos, &cam_color);
			}
			_ => (),
		}
	}
}

fn render_3d_camera(
	transform: Transform,
	scaler: f32,
	gizmos: &mut Gizmos,
	cam_color: &GameCameraColor,
) {
	gizmos.cuboid(transform, **cam_color);

	let forward = transform.forward().as_vec3();

	let rect_pos = transform.translation + forward;
	let rect_iso = Isometry3d::new(rect_pos, transform.rotation);
	let rect_dim = Vec2::new(scaler, 1.0);

	gizmos.rect(rect_iso, rect_dim, **cam_color);

	let start = transform.translation + forward * transform.scale / 2.0;

	let rect_corners = [
		rect_dim,
		-rect_dim,
		rect_dim.with_x(-rect_dim.x),
		rect_dim.with_y(-rect_dim.y),
	]
	.map(|corner| Vec3::from((corner / 2.0, 0.0)))
	.map(|corner| rect_iso * corner);

	for corner in rect_corners {
		gizmos.line(start, corner, **cam_color);
	}
}
