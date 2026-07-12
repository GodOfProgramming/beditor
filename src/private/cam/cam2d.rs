use super::{
	ActiveEditorCamera, CameraInputSystems, CameraSettingsGroup, EditorCamera, EditorManagedCamera,
	GameCameraColor, PanState, PanSystems, should_show_cameras,
};
use crate::{
	EditorState,
	private::{EditorInternalQuery, EditorInternalSingle, UserHidden, cam::EditorCameraScene, util},
	storage::{ProjectSettings, settings::Setting},
};
use bevy::{
	camera::RenderTarget,
	input::mouse::MouseMotion,
	prelude::*,
	window::{PrimaryWindow, SystemCursorIcon},
};
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

pub struct EditorCam2dPlugin;

impl Plugin for EditorCam2dPlugin {
	fn build(&self, app: &mut App) {
		app
			.add_plugins(InputManagerPlugin::<CameraActions>::default())
			.add_observer(on_new_camera_scene)
			.add_observer(on_camera_despawn)
			.add_systems(OnEnter(EditorState::Exiting), save_settings)
			.add_systems(
				Update,
				(
					(
						(
							released_mouse_input_actions,
							mouse_input_actions,
							(pan_system.in_set(PanSystems), zoom_system),
						)
							.chain()
							.in_set(CameraInputSystems::Mouse),
						movement_system.in_set(CameraInputSystems::Keyboard),
					)
						.chain(),
					render_2d_cameras
						.run_if(should_show_cameras.and_then(any_with_component::<EditorCamera2d>)),
				),
			);
	}
}

#[derive(Component, Default)]
#[require(EditorCamera, UserHidden, Camera2d, CameraSettings)]
struct EditorCamera2d;

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum CameraActions {
	PanCamera,
	#[actionlike(Axis)]
	Zoom,
	MoveNorth,
	MoveSouth,
	MoveWest,
	MoveEast,
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct CameraSaveData {
	settings: CameraSettings,
	transform: Transform,
	orthographic_scale: Option<f32>,
}

#[derive(Component, Reflect, Serialize, Deserialize, Clone)]
#[require(UserHidden)]
struct CameraSettings {
	move_speed: f32,
	zoom_sensitivity: f32,
	pan_sensitivity: f32,
}

impl Default for CameraSettings {
	fn default() -> Self {
		CameraSettings {
			move_speed: 128.0,
			zoom_sensitivity: 10.0,
			pan_sensitivity: 1.0,
		}
	}
}

#[derive(Default, Component, Reflect, Serialize, Deserialize, Clone)]
#[require(UserHidden)]
struct CameraState {
	pan_viewport_start: Option<Vec2>,
}

fn on_new_camera_scene(
	event: On<Add, EditorCameraScene>,
	mut commands: Commands,
	mut settings: ProjectSettings,
	active_camera: Res<ActiveEditorCamera>,
) {
	if *active_camera != ActiveEditorCamera::Cam2D {
		return;
	}

	let CameraSaveData {
		settings,
		transform,
		orthographic_scale,
	} = settings.get(CamStateSetting2d).unwrap_or_default();

	let inputs = InputMap::default()
		.with_one_to_many(
			CameraActions::PanCamera,
			[MouseButton::Middle, MouseButton::Right],
		)
		.with_axis(CameraActions::Zoom, MouseScrollAxis::Y)
		.with(CameraActions::MoveNorth, KeyCode::KeyW)
		.with(CameraActions::MoveSouth, KeyCode::KeyS)
		.with(CameraActions::MoveWest, KeyCode::KeyA)
		.with(CameraActions::MoveEast, KeyCode::KeyD);

	let mut ortho = OrthographicProjection::default_2d();

	if let Some(scale) = orthographic_scale {
		ortho.scale = scale;
	}

	commands.spawn((
		Name::new("Editor Camera 2D"),
		EditorCamera2d,
		ChildOf(event.event_target()),
		settings,
		transform,
		inputs,
		Projection::Orthographic(ortho),
		CameraState::default(),
	));
}

fn on_camera_despawn(
	event: On<Remove, Camera2d>,
	mut settings: ProjectSettings,
	q_cameras: EditorInternalQuery<(&Transform, &CameraSettings, &Projection), With<EditorCamera2d>>,
) {
	let Ok((cam_transform, cam_settings, cam_proj)) = q_cameras.get(event.event_target()) else {
		return;
	};

	if let Projection::Orthographic(cam_ortho) = &cam_proj {
		settings
			.set(
				CamStateSetting2d,
				CameraSaveData {
					settings: cam_settings.clone(),
					transform: *cam_transform,
					orthographic_scale: Some(cam_ortho.scale),
				},
			)
			.ok();
	}
}

fn save_settings(
	mut settings: ProjectSettings,
	q_cam: EditorInternalQuery<(&Transform, &CameraSettings, &Projection), With<EditorCamera2d>>,
) {
	for (cam_transform, cam_settings, cam_proj) in &q_cam {
		if let Projection::Orthographic(cam_ortho) = &cam_proj {
			settings
				.set(
					CamStateSetting2d,
					CameraSaveData {
						settings: cam_settings.clone(),
						transform: *cam_transform,
						orthographic_scale: Some(cam_ortho.scale),
					},
				)
				.ok();
		}
	}
}

fn mouse_input_actions(
	mut commands: Commands,
	mut q_cam_states: EditorInternalQuery<(&mut CameraState, &ActionState<CameraActions>)>,
	primary_window: Single<(Entity, &Window), With<PrimaryWindow>>,
	mut pan_state: ResMut<NextState<PanState>>,
) {
	let (window_entity, window) = *primary_window;

	for (mut cam_state, action_state) in &mut q_cam_states {
		if action_state.just_pressed(&CameraActions::PanCamera) {
			util::window::set_cursor_icon(&mut commands, window_entity, SystemCursorIcon::Grab);

			cam_state.pan_viewport_start = window.cursor_position();

			pan_state.set(PanState::Active);
		}
	}
}

fn released_mouse_input_actions(
	mut commands: Commands,
	q_action_states: EditorInternalQuery<&ActionState<CameraActions>>,
	primary_window: Single<Entity, With<PrimaryWindow>>,
	mut pan_state: ResMut<NextState<PanState>>,
) {
	for action_state in &q_action_states {
		if action_state.just_released(&CameraActions::PanCamera) {
			util::window::set_cursor_icon(&mut commands, *primary_window, SystemCursorIcon::default());

			pan_state.set(PanState::Inactive);
		}
	}
}

fn movement_system(
	mut editor_camera: EditorInternalSingle<(
		&CameraSettings,
		&mut Transform,
		&ActionState<CameraActions>,
	)>,
	time: Res<Time>,
) {
	let (ref mut cam_settings, ref mut cam_transform, action_state) = *editor_camera;

	let mut movement = Vec3::ZERO;

	if action_state.pressed(&CameraActions::MoveNorth) {
		movement += Vec3::Y;
	}

	if action_state.pressed(&CameraActions::MoveSouth) {
		movement -= Vec3::Y;
	}

	if action_state.pressed(&CameraActions::MoveWest) {
		movement -= Vec3::X;
	}

	if action_state.pressed(&CameraActions::MoveEast) {
		movement += Vec3::X;
	}

	let moved = movement != Vec3::ZERO;

	if moved {
		let movement = movement.normalize() * cam_settings.move_speed * time.delta_secs();
		cam_transform.translation += movement;
	}
}

fn zoom_system(
	mut editor_camera: EditorInternalSingle<(
		&CameraSettings,
		&mut Projection,
		&ActionState<CameraActions>,
	)>,
	time: Res<Time>,
) {
	let (cam_settings, ref mut projection, action_state) = *editor_camera;

	let Projection::Orthographic(ref mut projection) = **projection else {
		return;
	};

	let zoom = 1.0
		- action_state.clamped_value(&CameraActions::Zoom)
			* cam_settings.zoom_sensitivity
			* time.delta_secs();

	projection.scale *= zoom;
}

fn pan_system(
	mut camera: EditorInternalSingle<
		(
			&RenderTarget,
			&EditorManagedCamera,
			&Projection,
			&mut Transform,
			&CameraSettings,
		),
		With<EditorCamera2d>,
	>,
	mut mouse_motion: MessageReader<MouseMotion>,
	images: Res<Assets<Image>>,
	window: Single<&Window, With<PrimaryWindow>>,
) {
	let (target, managed_camera, projection, ref mut transform, settings) = *camera;

	let Projection::Orthographic(ortho) = projection else {
		return;
	};

	let texture_size = target
		.as_image()
		.and_then(|handle| images.get(handle.id()))
		.map(|image| image.size())
		.unwrap_or_default()
		.as_vec2();

	let ui_viewport = managed_camera
		.viewport()
		.map(|vp| vp.size())
		.unwrap_or(texture_size);

	let delta = mouse_motion
		.read()
		.map(|motion| motion.delta)
		.reduce(|c, n| c + n)
		.unwrap_or_default()
		* (texture_size / ui_viewport)
		* settings.pan_sensitivity
		* ortho.scale
		* window.scale_factor();

	transform.translation.x -= delta.x;
	transform.translation.y += delta.y;
}

#[allow(clippy::type_complexity)]
fn render_2d_cameras(
	mut gizmos: Gizmos,
	q_app_cameras: Query<
		(&Transform, &Projection),
		(
			With<Camera2d>,
			Without<EditorCamera2d>,
			// to support this in both dev & user mode
			Without<UserHidden>,
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

/* Camera Settings */

pub struct CamStateSetting2d;

impl Setting for CamStateSetting2d {
	type Type = CameraSaveData;
	type Group = CameraSettingsGroup;
	const NAME: &str = "cam2d_state";
}
