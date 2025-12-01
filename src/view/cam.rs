use super::UP;
use crate::{
	Settings,
	util::storage::{ActiveEditorCameraSetting, RenderCamerasSetting},
};
use bevy::{color::palettes::tailwind, prelude::*};
use derive_new::new;
use serde::{Deserialize, Serialize};

pub struct EditorCamPlugin;

impl Plugin for EditorCamPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<RenderCameras>()
			.init_resource::<GameCameraColor>()
			.add_observer(PointCameraEvent::handle)
			.add_observer(MoveCameraEvent::handle)
			.add_observer(SyncRenderCamerasEvent::handle)
			.add_systems(Startup, retrieve_show_cameras_value)
			.add_systems(PostStartup, set_initial_state)
			.add_systems(OnEnter(ActiveEditorCamera::None), despawn_editor_cameras)
			.add_systems(
				FixedUpdate,
				(track_editor_camera_changes.run_if(state_changed::<ActiveEditorCamera>),),
			);
	}
}

fn set_initial_state(
	mut settings: Settings,
	mut next_state: ResMut<NextState<ActiveEditorCamera>>,
) {
	let state = settings.get_or_default::<ActiveEditorCameraSetting>();
	next_state.set(state);
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
#[require(MeshPickingCamera)]
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
	mut settings: Settings,
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
	fn handle(_: On<Self>, render_cameras: Res<RenderCameras>, mut settings: Settings) -> Result {
		settings.set::<RenderCamerasSetting>(**render_cameras)?;
		Ok(())
	}
}

fn retrieve_show_cameras_value(mut render_cameras: ResMut<RenderCameras>, mut settings: Settings) {
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
