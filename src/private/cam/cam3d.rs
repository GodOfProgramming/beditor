use super::{
	ActiveEditorCamera, CameraInputSystems, CameraSettingsGroup, EditorCamera, EditorCameraScene,
	GameCameraColor, OrbitState, OrbitSystems, PanState, PanSystems, should_show_cameras,
};
use crate::{
	EditorState,
	private::{EditorInternalQuery, EditorInternalSingle, UserHidden, util},
	storage::{ProjectSettings, settings::Setting},
};
use bevy::{input::mouse::MouseMotion, prelude::*, window::CursorOptions};
use derive_new::new;
use leafwing_input_manager::prelude::*;
use notify::Notification;
use serde::{Deserialize, Serialize};

pub struct EditorCam3dPlugin;

impl Plugin for EditorCam3dPlugin {
	fn build(&self, app: &mut App) {
		app
			.add_plugins(InputManagerPlugin::<CameraActions>::default())
			.add_observer(on_new_camera_scene)
			.add_observer(on_camera_despawn)
			.add_observer(LookAt::on_event)
			.add_systems(OnEnter(EditorState::Exiting), save_settings)
			.add_systems(
				Update,
				(
					(
						(
							released_mouse_input_actions,
							mouse_input_actions,
							(
								orbit_system.in_set(OrbitSystems),
								pan_system.in_set(PanSystems),
								zoom_system,
							),
						)
							.chain()
							.in_set(CameraInputSystems::Mouse),
						movement_system.in_set(CameraInputSystems::Keyboard),
					)
						.chain(),
					render_3d_cameras
						.run_if(should_show_cameras.and_then(any_with_component::<EditorCamera3d>)),
				),
			);
	}
}

#[derive(Component, Default)]
#[require(Camera3d, EditorCamera, UserHidden, CameraSettings)]
struct EditorCamera3d;

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum CameraActions {
	PanCamera,
	OrbitCamera,
	#[actionlike(Axis)]
	Zoom,
	MoveNorth,
	MoveSouth,
	MoveWest,
	MoveEast,
}

#[derive(new, EntityEvent)]
pub struct LookAt(pub Entity);

impl LookAt {
	fn on_event(
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
				transform.look_at(target.translation, Vec3::Y);
			}
		}
	}
}

impl Command for LookAt {
	type Out = ();
	fn apply(self, world: &mut World) {
		world.trigger(self);
	}
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct CameraSaveData {
	settings: CameraSettings,
	transform: Transform,
}

#[derive(Component, Reflect, Serialize, Deserialize, Clone)]
#[require(UserHidden)]
pub struct CameraSettings {
	move_speed: f32,
	orbit_sensitivity: f32,
	zoom_sensitivity: f32,
	pan_sensitivity: f32,
}

impl Default for CameraSettings {
	fn default() -> Self {
		CameraSettings {
			move_speed: 10.0,
			orbit_sensitivity: 0.05,
			zoom_sensitivity: 5.0,
			pan_sensitivity: 0.2,
		}
	}
}

fn on_new_camera_scene(
	event: On<Add, EditorCameraScene>,
	mut commands: Commands,
	mut settings: ProjectSettings,
	active_camera: Res<ActiveEditorCamera>,
) {
	if *active_camera != ActiveEditorCamera::Cam3D {
		return;
	}

	let CameraSaveData {
		settings,
		transform,
	} = settings.get(CamStateSetting3d).unwrap_or_default();

	let inputs = InputMap::default()
		.with(CameraActions::OrbitCamera, MouseButton::Right)
		.with(CameraActions::PanCamera, MouseButton::Middle)
		.with_axis(CameraActions::Zoom, MouseScrollAxis::Y)
		.with(CameraActions::MoveNorth, KeyCode::KeyW)
		.with(CameraActions::MoveSouth, KeyCode::KeyS)
		.with(CameraActions::MoveWest, KeyCode::KeyA)
		.with(CameraActions::MoveEast, KeyCode::KeyD);

	commands.spawn((
		Name::new("Editor Camera 3D"),
		EditorCamera3d,
		ChildOf(event.event_target()),
		settings,
		transform,
		inputs,
	));
}

fn on_camera_despawn(
	event: On<Remove, Camera3d>,
	mut settings: ProjectSettings,
	q_cameras: EditorInternalQuery<(&Transform, &CameraSettings)>,
) {
	let Ok((cam_transform, cam_settings)) = q_cameras.get(event.event_target()) else {
		return;
	};

	settings
		.set(
			CamStateSetting3d,
			CameraSaveData {
				settings: cam_settings.clone(),
				transform: *cam_transform,
			},
		)
		.ok();
}

fn save_settings(
	mut settings: ProjectSettings,
	editor_camera: EditorInternalSingle<(&Transform, &CameraSettings)>,
) {
	let (cam_transform, cam_settings) = *editor_camera;
	settings
		.set(
			CamStateSetting3d,
			CameraSaveData {
				settings: cam_settings.clone(),
				transform: *cam_transform,
			},
		)
		.ok();
}

fn mouse_input_actions(
	q_camera_actions: EditorInternalQuery<&ActionState<CameraActions>>,
	mut q_cursors: Query<&mut CursorOptions>,
	mut orbit_state: ResMut<NextState<OrbitState>>,
	mut pan_state: ResMut<NextState<PanState>>,
) {
	for action_state in &q_camera_actions {
		let orbit_active = action_state.just_pressed(&CameraActions::OrbitCamera);
		let pan_active = action_state.just_pressed(&CameraActions::PanCamera);

		if orbit_active || pan_active {
			for mut cursor in &mut q_cursors {
				util::window::hide_cursor(&mut cursor);
			}
		}

		if orbit_active {
			orbit_state.set(OrbitState::Active);
		}

		if pan_active {
			pan_state.set(PanState::Active);
		}
	}
}

fn released_mouse_input_actions(
	q_action_states: EditorInternalQuery<&ActionState<CameraActions>>,
	mut q_cursors: Query<&mut CursorOptions>,
	mut orbit_state: ResMut<NextState<OrbitState>>,
	mut pan_state: ResMut<NextState<PanState>>,
) {
	for action_state in &q_action_states {
		let orbit_inactive = action_state.just_released(&CameraActions::OrbitCamera);
		let pan_inactive = action_state.just_released(&CameraActions::PanCamera);

		if (orbit_inactive && action_state.released(&CameraActions::PanCamera))
			|| (pan_inactive && action_state.released(&CameraActions::OrbitCamera))
		{
			for mut cursor in &mut q_cursors {
				util::window::show_cursor(&mut cursor);
			}
		}

		if orbit_inactive {
			orbit_state.set(OrbitState::Inactive);
		}

		if pan_inactive {
			pan_state.set(PanState::Inactive);
		}
	}
}

fn movement_system(
	mut editor_camera: EditorInternalSingle<
		(&CameraSettings, &mut Transform, &ActionState<CameraActions>),
		With<EditorCamera3d>,
	>,
	time: Res<Time>,
) {
	let (ref mut cam_settings, ref mut cam_transform, action_state) = *editor_camera;

	let forward = cam_transform.forward().as_vec3();
	let mut movement = Vec3::ZERO;

	if action_state.pressed(&CameraActions::MoveNorth) {
		movement += forward;
	}

	if action_state.pressed(&CameraActions::MoveSouth) {
		movement -= forward;
	}

	if action_state.pressed(&CameraActions::MoveWest) {
		movement -= forward.cross(Vec3::Y);
	}

	if action_state.pressed(&CameraActions::MoveEast) {
		movement += forward.cross(Vec3::Y);
	}

	let moved = movement != Vec3::ZERO;

	if moved {
		let movement = movement.normalize() * cam_settings.move_speed * time.delta_secs();
		cam_transform.translation += movement;
	}
}

fn orbit_system(
	mut editor_camera: EditorInternalSingle<(&CameraSettings, &mut Transform), With<EditorCamera3d>>,
	mut mouse_motion: MessageReader<MouseMotion>,
	time: Res<Time>,
) {
	let (ref mut settings, ref mut transform) = *editor_camera;

	let orbit = mouse_motion
		.read()
		.map(|motion| motion.delta)
		.reduce(|c, n| c + n)
		.map(|mouse| mouse * settings.orbit_sensitivity * time.delta_secs())
		.unwrap_or_default();

	let right = transform.right();
	let Some(up) = Dir3::new(Vec3::Y).ok() else {
		return;
	};

	transform.rotate_axis(right, -orbit.y);
	transform.rotation = transform.rotation.normalize();

	transform.rotate_axis(up, -orbit.x);
	transform.rotation = transform.rotation.normalize();
}

fn pan_system(
	mut editor_camera: EditorInternalSingle<(&CameraSettings, &mut Transform), With<EditorCamera3d>>,
	mut mouse_motion: MessageReader<MouseMotion>,
	time: Res<Time>,
) {
	let (ref mut cam_settings, ref mut cam_transform) = *editor_camera;

	let pan = mouse_motion
		.read()
		.map(|motion| motion.delta)
		.reduce(|c, n| c + n)
		.unwrap_or_default();

	let sensitivity = cam_settings.pan_sensitivity * time.delta_secs();
	let horizontal = cam_transform.right() * pan.x * sensitivity;
	let vertical = cam_transform.up() * pan.y * sensitivity;

	cam_transform.translation += horizontal;
	cam_transform.translation -= vertical;
}

fn zoom_system(
	mut editor_camera: EditorInternalSingle<
		(
			&CameraSettings,
			&mut Projection,
			&ActionState<CameraActions>,
		),
		With<EditorCamera3d>,
	>,
	time: Res<Time>,
) {
	let (cam_settings, ref mut projection, action_state) = *editor_camera;

	let zoom = 1.0
		- action_state.clamped_value(&CameraActions::Zoom)
			* cam_settings.zoom_sensitivity
			* time.delta_secs();

	match **projection {
		Projection::Perspective(ref mut perspective_projection) => {
			perspective_projection.fov *= zoom;
		}
		Projection::Orthographic(ref mut orthographic_projection) => {
			orthographic_projection.scale *= zoom;
		}
		_ => (),
	}
}

#[allow(clippy::type_complexity)]
fn render_3d_cameras(
	mut gizmos: Gizmos,
	q_app_cameras: Query<
		(&Transform, &Projection),
		(
			With<Camera3d>,
			Without<EditorCamera3d>,
			// to support this in both dev & user mode
			Without<UserHidden>,
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
	gizmos.cube(transform, **cam_color);

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

/* Camera Settings */

pub struct CamStateSetting3d;

impl Setting for CamStateSetting3d {
	type Type = CameraSaveData;
	type Group = CameraSettingsGroup;
	const NAME: &str = "cam3d_state";
}
