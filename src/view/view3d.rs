use super::{OrbitState, PanState, UP};
use crate::{
	input::EditorActions,
	settings::CamStateSetting3d,
	util::{self, storage::ProjectSettings},
	view::cam::EditorCamera,
};
use bevy::{
	core_pipeline::prepass::DepthPrepass, input::mouse::MouseMotion, prelude::*,
	window::CursorOptions,
};
use bevy_mesh_outline::OutlineCamera;
use leafwing_input_manager::prelude::ActionState;
use serde::{Deserialize, Serialize};
use transform_gizmo_bevy::GizmoCamera;

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
pub struct View3d;

#[derive(Component, Default)]
#[require(
  Camera3d,
  EditorCamera,
  CameraSettings,
  GizmoCamera = GizmoCamera,
  OutlineCamera = OutlineCamera, Msaa::Off, DepthPrepass,
)]
pub struct EditorCamera3d;

pub fn enable(
	mut commands: Commands,
	mut q_prev_cams: Query<Entity, With<EditorCamera>>,
	mut settings: ProjectSettings,
) {
	info!("Switched to 3d camera");

	for entity in &mut q_prev_cams {
		commands.entity(entity).despawn();
	}

	let CameraSaveData {
		settings,
		transform,
	} = settings.get(CamStateSetting3d).unwrap_or_default();

	commands.spawn((
		Name::new("Editor Camera 3D"),
		EditorCamera3d,
		settings,
		transform,
	));
}

pub fn save_settings(
	mut settings: ProjectSettings,
	q_cam: Query<(&Transform, &CameraSettings), With<EditorCamera3d>>,
) -> Result {
	for (cam_transform, cam_settings) in &q_cam {
		settings.set(
			CamStateSetting3d,
			CameraSaveData {
				settings: cam_settings.clone(),
				transform: *cam_transform,
			},
		)?;
	}

	Ok(())
}

pub(super) fn mouse_input_actions(
	q_action_states: Query<&ActionState<EditorActions>>,
	mut q_cursors: Query<&mut CursorOptions>,
	mut orbit_state: ResMut<NextState<OrbitState>>,
	mut pan_state: ResMut<NextState<PanState>>,
) {
	for action_state in &q_action_states {
		let orbit_active = action_state.just_pressed(&EditorActions::OrbitCamera);
		let pan_active = action_state.just_pressed(&EditorActions::PanCamera);

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

pub(super) fn released_mouse_input_actions(
	q_action_states: Query<&ActionState<EditorActions>>,
	mut q_cursors: Query<&mut CursorOptions>,
	mut orbit_state: ResMut<NextState<OrbitState>>,
	mut pan_state: ResMut<NextState<PanState>>,
) {
	for action_state in &q_action_states {
		let orbit_inactive = action_state.just_released(&EditorActions::OrbitCamera);
		let pan_inactive = action_state.just_released(&EditorActions::PanCamera);

		if (orbit_inactive && action_state.released(&EditorActions::PanCamera))
			|| (pan_inactive && action_state.released(&EditorActions::OrbitCamera))
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

pub fn movement_system(
	q_action_states: Query<&ActionState<EditorActions>>,
	mut q_cam: Single<(&CameraSettings, &mut Transform), With<EditorCamera3d>>,
	time: Res<Time>,
) {
	for action_state in &q_action_states {
		let (cam_settings, cam_transform) = &mut *q_cam;

		let forward = cam_transform.forward().as_vec3();
		let mut movement = Vec3::ZERO;

		if action_state.pressed(&EditorActions::MoveNorth) {
			movement += forward;
		}

		if action_state.pressed(&EditorActions::MoveSouth) {
			movement -= forward;
		}

		if action_state.pressed(&EditorActions::MoveWest) {
			movement -= forward.cross(UP);
		}

		if action_state.pressed(&EditorActions::MoveEast) {
			movement += forward.cross(UP);
		}

		let moved = movement != Vec3::ZERO;

		if moved {
			let movement = movement.normalize() * cam_settings.move_speed * time.delta_secs();
			cam_transform.translation += movement;
		}
	}
}

pub fn orbit_system(
	q_action_states: Query<&ActionState<EditorActions>>,
	mut q_cam: Single<(&CameraSettings, &mut Transform), With<EditorCamera3d>>,
	mut mouse_motion: MessageReader<MouseMotion>,
	time: Res<Time>,
) {
	let should_orbit = q_action_states
		.iter()
		.any(|state| state.pressed(&EditorActions::OrbitCamera));

	if !should_orbit {
		return;
	}

	let (settings, transform) = &mut *q_cam;

	let orbit = mouse_motion
		.read()
		.map(|motion| motion.delta)
		.reduce(|c, n| c + n)
		.map(|mouse| mouse * settings.orbit_sensitivity * time.delta_secs())
		.unwrap_or_default();

	let right = transform.right();
	let Some(up) = Dir3::new(UP).ok() else {
		return;
	};

	transform.rotate_axis(right, -orbit.y);
	transform.rotation = transform.rotation.normalize();

	transform.rotate_axis(up, -orbit.x);
	transform.rotation = transform.rotation.normalize();
}

pub fn pan_system(
	q_action_states: Query<&ActionState<EditorActions>>,
	mut q_cam: Single<(&CameraSettings, &mut Transform), With<EditorCamera3d>>,
	mut mouse_motion: MessageReader<MouseMotion>,
	time: Res<Time>,
) {
	let should_pan = q_action_states
		.iter()
		.any(|state| state.pressed(&EditorActions::PanCamera));

	if !should_pan {
		return;
	}

	let (cam_settings, cam_transform) = &mut *q_cam;

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

pub fn zoom_system(
	q_action_states: Query<&ActionState<EditorActions>>,
	mut q_cam: Query<(&CameraSettings, &mut Projection), With<EditorCamera3d>>,
	time: Res<Time>,
) {
	let Ok((cam_settings, mut projection)) = q_cam.single_mut() else {
		return;
	};

	for action_state in &q_action_states {
		let zoom = 1.0
			- action_state.clamped_value(&EditorActions::Zoom)
				* cam_settings.zoom_sensitivity
				* time.delta_secs();

		match &mut *projection {
			Projection::Perspective(perspective_projection) => {
				perspective_projection.fov *= zoom;
			}
			Projection::Orthographic(orthographic_projection) => {
				orthographic_projection.scale *= zoom;
			}
			_ => (),
		}
	}
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct CameraSaveData {
	settings: CameraSettings,
	transform: Transform,
}

#[derive(Component, Reflect, Serialize, Deserialize, Clone)]
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
