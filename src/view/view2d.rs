use super::{PanState, UP};
use crate::{
  input::EditorActions,
  util::{
    self,
    storage::{CamStateSetting2d, Settings},
  },
  view::cam::EditorCamera,
};
use bevy::{
  input::mouse::MouseMotion,
  picking::pointer::PointerLocation,
  prelude::*,
  window::{PrimaryWindow, SystemCursorIcon},
};
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
pub struct View2d;

#[derive(Component, Default)]
#[require(EditorCamera, Camera2d, CameraSettings)]
pub struct EditorCamera2d;

pub fn enable(
  mut commands: Commands,
  mut q_prev_cams: Query<Entity, With<EditorCamera>>,
  mut settings: Settings,
) {
  info!("Switched to 2d camera");

  for entity in &mut q_prev_cams {
    commands.entity(entity).despawn();
  }

  let CameraSaveData {
    settings,
    transform,
    orthographic_scale,
  } = settings.get_or_default::<CamStateSetting2d>();

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
    Camera {
      order: isize::MIN,
      ..default()
    },
  ));
}

pub fn save_settings(
  mut settings: Settings,
  q_cam: Query<(&Transform, &CameraSettings, &Projection), With<EditorCamera2d>>,
) -> Result {
  for (cam_transform, cam_settings, cam_proj) in &q_cam {
    if let Projection::Orthographic(cam_ortho) = &cam_proj {
      settings.set::<CamStateSetting2d>(CameraSaveData {
        settings: cam_settings.clone(),
        transform: *cam_transform,
        orthographic_scale: Some(cam_ortho.scale),
      })?;
    }
  }

  Ok(())
}

pub(super) fn mouse_input_actions(
  mut commands: Commands,
  mut q_cam_states: Query<(&mut CameraState, &Camera), With<EditorCamera2d>>,
  q_action_states: Query<&ActionState<EditorActions>>,
  primary_window: Single<Entity, With<PrimaryWindow>>,
  q_pointers: Query<&PointerLocation>,
  mut pan_state: ResMut<NextState<PanState>>,
) {
  for action_state in &q_action_states {
    if action_state.just_pressed(&EditorActions::PanCamera) {
      util::set_cursor_icon(&mut commands, *primary_window, SystemCursorIcon::Grab);

      for (mut cam_state, camera) in &mut q_cam_states {
        cam_state.pan_viewport_start = q_pointers
          .iter()
          .next()
          .and_then(|p| p.location.as_ref().zip(camera.viewport.as_ref()))
          .map(|(location, viewport)| location.position - viewport.physical_position.as_vec2());
      }

      pan_state.set(PanState::Active);
    }
  }
}

pub(super) fn released_mouse_input_actions(
  mut commands: Commands,
  q_action_states: Query<&ActionState<EditorActions>>,
  primary_window: Single<Entity, With<PrimaryWindow>>,
  mut pan_state: ResMut<NextState<PanState>>,
) {
  for action_state in &q_action_states {
    if action_state.just_released(&EditorActions::PanCamera) {
      util::set_cursor_icon(&mut commands, *primary_window, SystemCursorIcon::default());

      pan_state.set(PanState::Inactive);
    }
  }
}

pub fn movement_system(
  q_action_states: Query<&ActionState<EditorActions>>,
  mut q_cam: Single<(&CameraSettings, &mut Transform), With<EditorCamera2d>>,
  time: Res<Time>,
) {
  for action_state in &q_action_states {
    let (cam_settings, cam_transform) = &mut *q_cam;

    let mut movement = Vec3::ZERO;

    if action_state.pressed(&EditorActions::MoveNorth) {
      movement += UP;
    }

    if action_state.pressed(&EditorActions::MoveSouth) {
      movement -= UP;
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
  q_action_states: Query<&ActionState<EditorActions>>,
  mut q_cam: Query<(&CameraSettings, &mut Projection), With<EditorCamera2d>>,
  time: Res<Time>,
) {
  let Ok((cam_settings, mut projection)) = q_cam.single_mut() else {
    return;
  };

  let Projection::Orthographic(projection) = &mut *projection else {
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
  q_action_states: Query<&ActionState<EditorActions>>,
  mut q_cam: Single<
    (
      &CameraSettings,
      &mut Transform,
      &GlobalTransform,
      &mut CameraState,
      &Camera,
    ),
    With<EditorCamera2d>,
  >,
  mut mouse_motion: MessageReader<MouseMotion>,
) {
  let should_pan = q_action_states
    .iter()
    .any(|state| state.pressed(&EditorActions::PanCamera));

  if !should_pan {
    return;
  }

  let (cam_settings, cam_transform, cam_g_transform, cam_state, cam) = &mut *q_cam;

  let pan_motion = mouse_motion
    .read()
    .map(|motion| motion.delta)
    .reduce(|c, n| c + n)
    .unwrap_or_default()
    * cam_settings.pan_sensitivity;

  if let Some((pan_vp_new_pos, pan_world_old_pos, pan_world_new_pos)) = cam_state
    .pan_viewport_start
    .map(|p| (p, p + pan_motion))
    .and_then(|(op, np)| {
      cam
        .viewport_to_world_2d(cam_g_transform, op)
        .ok()
        .zip(cam.viewport_to_world_2d(cam_g_transform, np).ok())
        .map(|(ow, nw)| (np, ow, nw))
    })
  {
    let delta = pan_world_new_pos - pan_world_old_pos;

    cam_state.pan_viewport_start = Some(pan_vp_new_pos);
    cam_transform.translation -= delta.extend(0.0);
  }
}

#[derive(Default, Serialize, Deserialize)]
pub struct CameraSaveData {
  settings: CameraSettings,
  transform: Transform,
  orthographic_scale: Option<f32>,
}

#[derive(Component, Reflect, Serialize, Deserialize, Clone)]
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
pub struct CameraState {
  pan_viewport_start: Option<Vec2>,
}
