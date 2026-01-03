pub mod axes;
pub mod cam2d;
pub mod cam3d;

use crate::{
	panels::editor_view::EditorViewUi,
	private::{
		EditorInternalFilter, EditorInternalQuery, EditorInternalSingle, EditorScene, UserHidden,
		input,
		ui::{EditorEguiContext, misc::UiState},
	},
	settings::{Setting, SettingsGroup, SettingsTable},
	util::storage::ProjectSettings,
};
use axes::AxesGizmoPlugin;
use bevy::{
	camera::{
		NormalizedRenderTarget, RenderTarget,
		visibility::{Layer, RenderLayers},
	},
	color::palettes::tailwind,
	ecs::system::SystemState,
	picking::{
		PickingSystems,
		pointer::{Location, PointerId, PointerInput},
	},
	prelude::*,
	render::render_resource::TextureFormat,
};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use cam2d::EditorCam2dPlugin;
use cam3d::EditorCam3dPlugin;
use derive_more::derive::Deref;
use derive_new::new;
use notify::Notification;
use serde::{Deserialize, Serialize};
use singleton::{SingletonBehavior, SingletonPlugin};
use smallvec::SmallVec;
use transform_gizmo_bevy::{GizmoCamera, GizmoOptions};
use uuid::Uuid;

pub const EDITOR_UI_RENDER_LAYER: Layer = 31;
pub const EDITOR_VIEW_RENDER_LAYER: Layer = 30;
const EDITOR_AXIS_RENDER_LAYER: Layer = 29;

pub struct EditorCamPlugin;

impl Plugin for EditorCamPlugin {
	fn build(&self, app: &mut App) {
		let mut system_state = SystemState::<ProjectSettings>::new(app.world_mut());
		let mut settings = system_state.get_mut(app.world_mut());
		let active_cam_state = settings.get(ActiveEditorCameraSetting).unwrap_or_default();

		app
			.add_plugins((
				SingletonPlugin::<EditorCameraScene, EditorInternalFilter>::new(
					SingletonBehavior::RemoveOther,
				),
				SingletonPlugin::<EditorWindowCamera, EditorInternalFilter>::new(
					SingletonBehavior::RemoveOther,
				),
				EditorCam2dPlugin,
				EditorCam3dPlugin,
				AxesGizmoPlugin,
			))
			.configure_sets(
				Update,
				(
					CameraInputSystems::Mouse.run_if(mouse_hovered_in_editor_view),
					OrbitSystems.run_if(in_state(OrbitState::Active)),
					PanSystems.run_if(in_state(PanState::Active)),
					CameraInputSystems::Keyboard
						.in_set(input::Unfocused)
						.run_if(mouse_movement_active),
				),
			)
			.insert_state(OrbitState::Inactive)
			.insert_state(PanState::Inactive)
			.insert_resource(active_cam_state)
			.init_resource::<RenderCameras>()
			.init_resource::<GameCameraColor>()
			.add_observer(MoveTo::handle)
			.add_observer(manage_camera)
			.add_observer(on_manage_camera)
			.add_observer(on_unmanage_camera)
			.add_observer(on_new_editor_scene)
			.add_systems(Startup, retrieve_show_cameras_value)
			.add_systems(PostStartup, init_camera)
			.add_systems(
				First,
				(
					(
						editor_picking_forwarding.in_set(PickingSystems::PostInput),
						EditorManagedCamera::sync_gizmos,
					),
					EditorManagedCamera::on_frame_end,
				)
					.chain(),
			)
			.add_systems(
				FixedUpdate,
				on_active_camera_change.run_if(resource_changed::<ActiveEditorCamera>),
			);
	}
}

#[derive(new, EntityEvent)]
pub struct MoveTo(pub Entity);

impl MoveTo {
	pub(super) fn handle(
		event: On<Self>,
		mut commands: Commands,
		mut q_transforms: EditorInternalQuery<&mut Transform>,
		q_cams: EditorInternalQuery<Entity, With<EditorCamera>>,
	) {
		let entity = event.event_target();
		let Ok(target) = q_transforms.get(entity).cloned() else {
			commands.trigger(
				Notification::warn("Tried to look at entity with no transform").with_context(
					serde_json::json!({
						"entity": entity
					}),
				),
			);
			return;
		};

		for cam in &q_cams {
			if let Ok(mut transform) = q_transforms.get_mut(cam) {
				transform.translation = target.translation;
			}
		}
	}
}

impl Command for MoveTo {
	fn apply(self, world: &mut World) {
		world.trigger(self);
	}
}

fn init_camera(mut settings: ProjectSettings, mut active_camera: ResMut<ActiveEditorCamera>) {
	let setting = settings.get(ActiveEditorCameraSetting).unwrap_or_default();
	*active_camera = setting;
}

fn on_manage_camera(
	event: On<Add, EditorManagedCamera>,
	mut images: ResMut<Assets<Image>>,
	mut q_cameras: EditorInternalQuery<&mut Camera>,
	mut user_textures: ResMut<EguiUserTextures>,
	mut commands: Commands,
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

	user_textures.add_image(EguiTextureHandle::Weak(image_handle.id()));

	commands
		.entity(event.event_target())
		.observe(|e: On<Pointer<Click>>| {
			info!("MANAGED\n{e:?}");
		});
}

fn on_unmanage_camera(
	event: On<Remove, EditorManagedCamera>,
	q_cameras: EditorInternalQuery<&Camera>,
	mut user_textures: ResMut<EguiUserTextures>,
) {
	if let Ok(camera) = q_cameras.get(event.event_target())
		&& let RenderTarget::Image(image) = &camera.target
	{
		user_textures.remove_image(image.handle.id());
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

#[derive(Default, Component, Reflect)]
#[require(
  UserHidden,
  MeshPickingCamera,
  EditorManagedCamera,
  RenderLayers = RenderLayers::from_layers(&[0, EDITOR_VIEW_RENDER_LAYER]),
)]
pub struct EditorCamera;

#[derive(Component)]
#[require(
  UserHidden,
  SceneRoot,
  Name = Name::new("Editor Camera Scene")
)]
pub struct EditorCameraScene;

#[derive(Default, Component, Reflect)]
#[require(
  UserHidden,
  EditorEguiContext,
  Camera2d,
  Camera = Camera { order: isize::MAX, ..default() },
  RenderLayers = RenderLayers::layer(EDITOR_UI_RENDER_LAYER),
  PointerId = PointerId::Custom(Uuid::new_v4()),
  Name = Name::new("Editor Window Camera"),
)]
pub struct EditorWindowCamera;

#[derive(Component, Default, Reflect)]
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

#[derive(Resource, Reflect, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActiveEditorCamera {
	Cam2D,
	#[default]
	Cam3D,
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

fn on_active_camera_change(
	mut commands: Commands,
	cam_state: Res<ActiveEditorCamera>,
	mut settings: ProjectSettings,
	editor_scene: EditorInternalSingle<Entity, With<EditorScene>>,
) {
	settings.set(ActiveEditorCameraSetting, *cam_state).ok();
	commands.spawn((EditorCameraScene, ChildOf(*editor_scene)));
}

fn on_new_editor_scene(event: On<Add, EditorScene>, mut commands: Commands) {
	let scene = event.event_target();
	commands.spawn((EditorWindowCamera, ChildOf(scene)));
	commands.spawn((EditorCameraScene, ChildOf(scene)));
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

fn manage_camera(
	event: On<Add, Camera>,
	mut commands: Commands,
	editor_ui_camera: EditorInternalSingle<Entity, Without<EditorWindowCamera>>,
) {
	let entity = event.event_target();

	if entity == *editor_ui_camera {
		return;
	}

	commands
		.entity(entity)
		.insert(EditorManagedCamera::default());
}

fn editor_picking_forwarding(
	mut commands: Commands,
	q_managed_cameras: EditorInternalQuery<(&Camera, &PointerId, &EditorManagedCamera)>,
	mut pointer_inputs: MessageReader<PointerInput>,
) {
	let inputs = pointer_inputs.read().collect::<SmallVec<[_; 4]>>();

	let editor_camera_inputs = inputs
		.iter()
		.filter(move |input| matches!(input.pointer_id, PointerId::Mouse | PointerId::Touch(_)))
		.collect::<SmallVec<[_; 2]>>();

	let iter = q_managed_cameras
		.iter()
		.filter_map(|(camera, managed_camera_pointer_id, managed_camera)| {
			if managed_camera.hovered && !managed_camera.context_menu_opened {
				managed_camera
					.viewport_rect
					.zip(camera.target.as_image())
					.map(|(viewport_rect, target)| (target, viewport_rect, managed_camera_pointer_id))
			} else {
				None
			}
		})
		.flat_map(|(target, viewport_rect, &managed_camera_pointer_id)| {
			editor_camera_inputs
				.iter()
				.map(move |input| (target, viewport_rect, managed_camera_pointer_id, input))
		});

	for (image_target, viewport_rect, managed_camera_pointer_id, pointer_input) in iter {
		let location = Location {
			position: pointer_input.location.position - viewport_rect.min,
			target: NormalizedRenderTarget::Image(image_target.clone().into()),
		};

		let msg = PointerInput {
			pointer_id: managed_camera_pointer_id,
			location,
			action: pointer_input.action,
		};

		commands.write_message(msg);
	}
}

/* Camera Settings */

pub struct CameraSettingsGroup;

impl SettingsGroup for CameraSettingsGroup {
	type Table = SettingsTable;
	const NAME: &str = "view";
}

pub struct RenderCamerasSetting;

impl Setting for RenderCamerasSetting {
	type Type = bool;
	type Group = CameraSettingsGroup;
	const NAME: &str = "render_cameras";
}

pub struct ActiveEditorCameraSetting;

impl Setting for ActiveEditorCameraSetting {
	type Type = ActiveEditorCamera;
	type Group = CameraSettingsGroup;
	const NAME: &str = "active_editor_camera";
}
