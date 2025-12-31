use super::{ActiveEditorCamera, EditorCamera, EditorManagedCamera, PanState};
use crate::{
	private::{EditorInternalQuery, EditorInternalSingle, UserHidden, input::EditorActions, util},
	settings::{ActiveEditorCameraSetting, CamStateSetting2d},
	util::storage::ProjectSettings,
};
use bevy::{
	input::mouse::MouseMotion,
	prelude::*,
	window::{PrimaryWindow, SystemCursorIcon},
};
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
pub struct Cam2dSystems;

#[derive(Component, Default)]
#[require(EditorCamera, UserHidden, Camera2d, CameraSettings)]
pub struct EditorCamera2d;

pub fn enable(mut commands: Commands, mut settings: ProjectSettings) {
	info!("Using 2D Camera");

	settings
		.set(ActiveEditorCameraSetting, ActiveEditorCamera::Cam2D)
		.ok();

	let CameraSaveData {
		settings,
		transform,
		orthographic_scale,
	} = settings.get(CamStateSetting2d).unwrap_or_default();

	let mut ortho = OrthographicProjection::default_2d();

	if let Some(scale) = orthographic_scale {
		ortho.scale = scale;
	}

	commands.spawn((
		Name::new("Editor Camera 2D"),
		EditorCamera2d,
		CameraState::default(),
		settings,
		transform,
		Projection::Orthographic(ortho),
	));
}

pub fn save_settings(
	mut settings: ProjectSettings,
	q_cam: EditorInternalQuery<(&Transform, &CameraSettings, &Projection), With<EditorCamera2d>>,
) -> Result {
	for (cam_transform, cam_settings, cam_proj) in &q_cam {
		if let Projection::Orthographic(cam_ortho) = &cam_proj {
			settings.set(
				CamStateSetting2d,
				CameraSaveData {
					settings: cam_settings.clone(),
					transform: *cam_transform,
					orthographic_scale: Some(cam_ortho.scale),
				},
			)?;
		}
	}

	Ok(())
}

pub(super) fn mouse_input_actions(
	mut commands: Commands,
	mut q_cam_states: EditorInternalQuery<&mut CameraState, With<EditorCamera2d>>,
	q_action_states: EditorInternalQuery<&ActionState<EditorActions>>,
	primary_window: Single<(Entity, &Window), With<PrimaryWindow>>,
	mut pan_state: ResMut<NextState<PanState>>,
) {
	let (window_entity, window) = *primary_window;
	for action_state in &q_action_states {
		if action_state.just_pressed(&EditorActions::PanCamera) {
			util::window::set_cursor_icon(&mut commands, window_entity, SystemCursorIcon::Grab);

			for mut cam_state in &mut q_cam_states {
				cam_state.pan_viewport_start = window.cursor_position();
			}

			pan_state.set(PanState::Active);
		}
	}
}

pub(super) fn released_mouse_input_actions(
	mut commands: Commands,
	q_action_states: EditorInternalQuery<&ActionState<EditorActions>>,
	primary_window: Single<Entity, With<PrimaryWindow>>,
	mut pan_state: ResMut<NextState<PanState>>,
) {
	for action_state in &q_action_states {
		if action_state.just_released(&EditorActions::PanCamera) {
			util::window::set_cursor_icon(&mut commands, *primary_window, SystemCursorIcon::default());

			pan_state.set(PanState::Inactive);
		}
	}
}

pub fn movement_system(
	q_action_states: EditorInternalQuery<&ActionState<EditorActions>>,
	mut editor_camera: EditorInternalSingle<(&CameraSettings, &mut Transform), With<EditorCamera2d>>,
	time: Res<Time>,
) {
	for action_state in &q_action_states {
		let (ref mut cam_settings, ref mut cam_transform) = *editor_camera;

		let mut movement = Vec3::ZERO;

		if action_state.pressed(&EditorActions::MoveNorth) {
			movement += Vec3::Y;
		}

		if action_state.pressed(&EditorActions::MoveSouth) {
			movement -= Vec3::Y;
		}

		if action_state.pressed(&EditorActions::MoveWest) {
			movement -= Vec3::X;
		}

		if action_state.pressed(&EditorActions::MoveEast) {
			movement += Vec3::X;
		}

		let moved = movement != Vec3::ZERO;

		if moved {
			let movement = movement.normalize() * cam_settings.move_speed * time.delta_secs();
			cam_transform.translation += movement;
		}
	}
}

pub fn zoom_system(
	q_action_states: EditorInternalQuery<&ActionState<EditorActions>>,
	mut editor_camera: EditorInternalSingle<(&CameraSettings, &mut Projection), With<EditorCamera2d>>,
	time: Res<Time>,
) {
	let (cam_settings, ref mut projection) = *editor_camera;

	let Projection::Orthographic(ref mut projection) = **projection else {
		return;
	};

	for action_state in &q_action_states {
		let zoom = 1.0
			- action_state.clamped_value(&EditorActions::Zoom)
				* cam_settings.zoom_sensitivity
				* time.delta_secs();

		projection.scale *= zoom;
	}
}

pub fn pan_system(
	mut camera: EditorInternalSingle<
		(
			&Camera,
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
	let (camera, managed_camera, projection, ref mut transform, settings) = *camera;

	let Projection::Orthographic(ortho) = projection else {
		return;
	};

	let texture_size = camera
		.target
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

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct CameraSaveData {
	settings: CameraSettings,
	transform: Transform,
	orthographic_scale: Option<f32>,
}

#[derive(Component, Reflect, Serialize, Deserialize, Clone)]
#[require(UserHidden)]
pub struct CameraSettings {
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
pub struct CameraState {
	pan_viewport_start: Option<Vec2>,
}
