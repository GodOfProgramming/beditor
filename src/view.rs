pub mod view2d;
pub mod view3d;

use crate::{
  Editing,
  cache::{Cache, Saveable},
  input,
  ui::{
    misc::UiInfo,
    prebuilt::{editor_view::EditorView, game_view::GameView},
  },
};
use bevy::{color::palettes::tailwind, prelude::*};
use derive_new::new;
use serde::{Deserialize, Serialize};
use view2d::View2d;
use view3d::View3d;

pub const UP: Vec3 = Vec3::Y;

const GAME_CAMERA_COLOR: Srgba = tailwind::GREEN_700;

pub struct EditorViewPlugin;

impl EditorViewPlugin {
  fn set_initial_state(cache: Res<Cache>, mut next_state: ResMut<NextState<ActiveEditorCamera>>) {
    let state = cache.get::<ActiveEditorCamera>().unwrap_or_default();
    next_state.set(state);
  }
}

impl Plugin for EditorViewPlugin {
  fn build(&self, app: &mut bevy::prelude::App) {
    app
      .configure_sets(
        Update,
        (
          CameraInput::Mouse
            .run_if(CameraInput::mouse_hovered)
            .in_set(Editing),
          View2d
            .in_set(Editing)
            .run_if(in_state(ActiveEditorCamera::Cam2D)),
          View3d
            .in_set(Editing)
            .run_if(in_state(ActiveEditorCamera::Cam3D)),
          OrbitSet.run_if(in_state(OrbitState::Active)),
          PanSet.run_if(in_state(PanState::Active)),
          ZoomSet.in_set(CameraInput::Mouse),
          CameraInput::Keyboard
            .in_set(input::Unfocused)
            .run_if(mouse_movement_active),
        ),
      )
      .register_type::<ActiveEditorCamera>()
      .register_type::<view2d::CameraSettings>()
      .register_type::<view2d::CameraState>()
      .add_event::<MoveCameraEvent>()
      .add_event::<PointCameraEvent>()
      .insert_state(ActiveEditorCamera::None)
      .insert_state(OrbitState::Inactive)
      .insert_state(PanState::Inactive)
      .add_systems(PostStartup, Self::set_initial_state)
      .add_systems(OnEnter(ActiveEditorCamera::None), despawn_editor_cameras)
      .add_systems(OnEnter(ActiveEditorCamera::Cam2D), view2d::enable)
      .add_systems(OnExit(ActiveEditorCamera::Cam2D), view2d::save_settings)
      .add_systems(OnEnter(ActiveEditorCamera::Cam3D), view3d::enable)
      .add_systems(OnExit(ActiveEditorCamera::Cam3D), view3d::save_settings)
      .add_systems(
        Update,
        (
          view2d::released_mouse_input_actions,
          (
            view2d::mouse_input_actions.in_set(CameraInput::Mouse),
            (
              view2d::pan_system.in_set(PanSet),
              view2d::zoom_system.in_set(ZoomSet),
            ),
          )
            .chain(),
          view2d::movement_system.in_set(CameraInput::Keyboard),
        )
          .chain()
          .in_set(View2d),
      )
      .add_systems(
        Update,
        (
          view3d::released_mouse_input_actions,
          (
            view3d::mouse_input_actions.in_set(CameraInput::Mouse),
            (
              view3d::orbit_system.in_set(OrbitSet),
              view3d::pan_system.in_set(PanSet),
              view3d::zoom_system.in_set(ZoomSet),
            ),
          )
            .chain(),
          view3d::movement_system.in_set(CameraInput::Keyboard),
        )
          .chain()
          .in_set(View3d),
      )
      .add_systems(
        FixedUpdate,
        (MoveCameraEvent::handle, PointCameraEvent::handle),
      );
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

impl Saveable for ActiveEditorCamera {
  const KEY: &str = "view_state";
}

#[derive(SystemSet, PartialEq, Eq, Hash, Clone, Debug)]
enum CameraInput {
  Keyboard,
  Mouse,
}

impl CameraInput {
  fn mouse_hovered(q_editor_view_ui_info: Query<&UiInfo, With<EditorView>>) -> bool {
    q_editor_view_ui_info.iter().any(UiInfo::hovered)
  }
}

fn despawn_editor_cameras(mut commands: Commands, q_cams: Query<Entity, With<EditorCamera>>) {
  info!("Despawning all editor cameras");
  for entity in &q_cams {
    commands.entity(entity).despawn();
  }
}

pub fn save_view_state(mut cache: ResMut<Cache>, view_state: Res<State<ActiveEditorCamera>>) {
  cache.store(view_state.get());
}

pub fn disable_camera<C: Component>(mut q_camera: Query<&mut Camera, With<C>>) {
  for mut camera in &mut q_camera {
    camera.is_active = false;
  }
}

pub fn add_game_camera<C>(app: &mut App)
where
  C: Component + Reflect + TypePath,
{
  app
    .register_type::<GameView<C>>()
    .add_systems(PostStartup, disable_camera::<C>)
    .add_systems(
      Update,
      (
        render_2d_cameras::<C>.in_set(View2d),
        render_3d_cameras::<C>.in_set(View3d),
      ),
    );
}

#[allow(clippy::type_complexity)]
fn render_2d_cameras<C: Component>(
  mut gizmos: Gizmos,
  q_cam: Query<(&Transform, &Projection), (With<Camera2d>, With<C>)>,
) {
  for (transform, projection) in &q_cam {
    if let Projection::Orthographic(ortho) = projection {
      let rect_pos = transform.translation;
      gizmos.rect(rect_pos, ortho.area.max - ortho.area.min, GAME_CAMERA_COLOR);
    }
  }
}

#[allow(clippy::type_complexity)]
fn render_3d_cameras<C: Component>(
  mut gizmos: Gizmos,
  q_cam: Query<(&Transform, &Projection), (With<Camera3d>, With<C>)>,
) {
  for (transform, projection) in &q_cam {
    match projection {
      Projection::Perspective(perspective) => {
        show_camera(*transform, perspective.aspect_ratio, &mut gizmos);
      }
      Projection::Orthographic(orthographic) => {
        show_camera(*transform, orthographic.scale, &mut gizmos);
      }
      _ => (),
    }
  }
}

fn show_camera(transform: Transform, scaler: f32, gizmos: &mut Gizmos) {
  gizmos.cuboid(transform, GAME_CAMERA_COLOR);

  let forward = transform.forward().as_vec3();

  let rect_pos = transform.translation + forward;
  let rect_iso = Isometry3d::new(rect_pos, transform.rotation);
  let rect_dim = Vec2::new(scaler, 1.0);

  gizmos.rect(rect_iso, rect_dim, GAME_CAMERA_COLOR);

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
    gizmos.line(start, corner, GAME_CAMERA_COLOR);
  }
}

fn mouse_movement_active(orbit: Res<State<OrbitState>>, pan: Res<State<PanState>>) -> bool {
  *orbit == OrbitState::Active || *pan == PanState::Active
}

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct OrbitSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
enum OrbitState {
  Active,
  Inactive,
}

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct PanSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
enum PanState {
  Active,
  Inactive,
}

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct ZoomSet;

#[derive(new, Event)]
pub struct MoveCameraEvent(Vec3);

impl MoveCameraEvent {
  fn handle(
    mut event_reader: EventReader<Self>,
    mut q_cam_transforms: Query<&mut Transform, With<EditorCamera>>,
  ) {
    for event in event_reader.read() {
      for mut cam in &mut q_cam_transforms {
        cam.translation = event.0;
      }
    }
  }
}

#[derive(new, Event)]
pub struct PointCameraEvent(Vec3);

impl PointCameraEvent {
  fn handle(
    mut event_reader: EventReader<Self>,
    mut q_cam_transforms: Query<&mut Transform, With<EditorCamera>>,
  ) {
    for event in event_reader.read() {
      for mut cam in &mut q_cam_transforms {
        cam.look_at(event.0, UP);
      }
    }
  }
}
