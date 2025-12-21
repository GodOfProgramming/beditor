use super::UP;
use crate::util::storage::{ActiveEditorCameraSetting, ProjectSettings, RenderCamerasSetting};
use bevy::{
	camera::{ImageRenderTarget, RenderTarget},
	color::palettes::tailwind,
	picking::pointer::PointerId,
	prelude::*,
	render::render_resource::TextureFormat,
	window::PrimaryWindow,
};
use derive_more::derive::Deref;
use derive_new::new;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub struct EditorCamPlugin;

impl Plugin for EditorCamPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<RenderCameras>()
			.init_resource::<GameCameraColor>()
			.add_observer(PointCameraEvent::handle)
			.add_observer(MoveCameraEvent::handle)
			.add_observer(SyncRenderCamerasEvent::handle)
			.add_observer(on_editor_camera_spawn)
			.add_observer(on_editor_camera_despawn)
			.add_systems(Startup, retrieve_show_cameras_value)
			.add_systems(PostStartup, init_camera)
			.add_systems(OnEnter(ActiveEditorCamera::None), despawn_editor_cameras)
			.add_systems(
				FixedUpdate,
				track_editor_camera_changes.run_if(state_changed::<ActiveEditorCamera>),
			);
	}
}

fn init_camera(
	mut settings: ProjectSettings,
	mut next_state: ResMut<NextState<ActiveEditorCamera>>,
) {
	let state = settings.get_or_default::<ActiveEditorCameraSetting>();
	next_state.set(state);
}

fn on_editor_camera_spawn(
	event: On<Add, EditorCamera>,
	mut commands: Commands,
	mut contexts: bevy_egui::EguiContexts,
	q_cameras: Query<&Camera>,
	window: Single<&Window, With<PrimaryWindow>>,
	mut images: ResMut<Assets<Image>>,
) {
	let camera = q_cameras.get(event.event_target()).ok();

	let image_size = get_viewport_size(camera, &window);
	let image = Image::new_target_texture(image_size.x, image_size.y, TextureFormat::Rgba32Float);
	let image_handle = images.add(image);

	contexts.add_image(bevy_egui::EguiTextureHandle::Weak(image_handle.id()));

	commands.entity(event.event_target()).insert((Camera {
		order: isize::MIN,
		target: RenderTarget::Image(ImageRenderTarget::from(image_handle)),
		..default()
	},));
}

fn on_editor_camera_despawn(
	event: On<Remove, EditorCamera>,
	q_cameras: Query<&Camera>,
	mut contexts: bevy_egui::EguiContexts,
) {
	if let Ok(camera) = q_cameras.get(event.event_target())
		&& let RenderTarget::Image(image) = &camera.target
	{
		contexts.remove_image(image.handle.id());
	}
}

fn despawn_editor_cameras(mut commands: Commands, q_cams: Query<Entity, With<EditorCamera>>) {
	info!("Despawning all editor cameras");
	for entity in &q_cams {
		commands.entity(entity).despawn();
	}
}

pub fn disable_camera<C: Component>(mut q_camera: Query<&mut Camera, With<C>>) {
	for mut camera in &mut q_camera {
		camera.is_active = false;
	}
}

#[derive(Default, Component, Reflect)]
#[require(
  MeshPickingCamera,
  PointerId = PointerId::Custom(Uuid::new_v4()),
)]
pub struct EditorCamera;

#[derive(
	Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default, Serialize, Deserialize, Reflect,
)]
pub enum ActiveEditorCamera {
	#[default]
	None,
	Cam2D,
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

fn track_editor_camera_changes(
	cam_state: Res<State<ActiveEditorCamera>>,
	mut settings: ProjectSettings,
) -> Result {
	settings.set::<ActiveEditorCameraSetting>(**cam_state)
}

#[derive(new, Event)]
pub struct MoveCameraEvent(Vec3);

impl MoveCameraEvent {
	fn handle(event: On<Self>, mut q_cam_transforms: Query<&mut Transform, With<EditorCamera>>) {
		for mut cam in &mut q_cam_transforms {
			cam.translation = event.0;
		}
	}
}

#[derive(new, Event)]
pub struct PointCameraEvent(Vec3);

impl PointCameraEvent {
	fn handle(event: On<Self>, mut q_cam_transforms: Query<&mut Transform, With<EditorCamera>>) {
		for mut cam in &mut q_cam_transforms {
			cam.look_at(event.0, UP);
		}
	}
}

#[derive(Event)]
pub struct SyncRenderCamerasEvent;

impl SyncRenderCamerasEvent {
	fn handle(
		_: On<Self>,
		render_cameras: Res<RenderCameras>,
		mut settings: ProjectSettings,
	) -> Result {
		settings.set::<RenderCamerasSetting>(**render_cameras)?;
		Ok(())
	}
}

fn retrieve_show_cameras_value(
	mut render_cameras: ResMut<RenderCameras>,
	mut settings: ProjectSettings,
) {
	**render_cameras = settings.get_or_default::<RenderCamerasSetting>();
}

pub fn should_show_cameras(render_cameras: Res<RenderCameras>) -> bool {
	**render_cameras
}

#[allow(clippy::type_complexity)]
pub fn render_2d_cameras<C: Component>(
	mut gizmos: Gizmos,
	q_cam: Query<(&Transform, &Projection), (With<Camera2d>, With<C>)>,
	cam_color: Res<GameCameraColor>,
) {
	for (transform, projection) in &q_cam {
		if let Projection::Orthographic(ortho) = projection {
			let rect_pos = transform.translation;
			gizmos.rect(rect_pos, ortho.area.max - ortho.area.min, **cam_color);
		}
	}
}

#[allow(clippy::type_complexity)]
pub fn render_3d_cameras<C: Component>(
	mut gizmos: Gizmos,
	q_cam: Query<(&Transform, &Projection), (With<Camera3d>, With<C>)>,
	cam_color: Res<GameCameraColor>,
) {
	for (transform, projection) in &q_cam {
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

pub fn get_viewport_size(camera: Option<&Camera>, window: &Window) -> UVec2 {
	camera
		.and_then(|c| c.viewport.as_ref().map(|vp| vp.physical_size))
		.unwrap_or(window.physical_size())
}
