pub mod cam;
pub mod view2d;
pub mod view3d;

use crate::{
  EditorState, input,
  ui::{
    misc::UiState,
    prebuilt::{editor_view::EditorView, game_view::GameView},
  },
  view::cam::{ActiveEditorCamera, EditorCamPlugin},
};
use bevy::prelude::*;
use view2d::View2d;
use view3d::View3d;

pub const UP: Vec3 = Vec3::Y;

pub struct EditorViewPlugin;

impl EditorViewPlugin {}

impl Plugin for EditorViewPlugin {
  fn build(&self, app: &mut bevy::prelude::App) {
    app
      .add_plugins(EditorCamPlugin)
      .configure_sets(
        Update,
        (
          CameraInputSystems::Mouse.run_if(mouse_hovered_in_editor_view),
          View2d.run_if(in_state(ActiveEditorCamera::Cam2D)),
          View3d.run_if(in_state(ActiveEditorCamera::Cam3D)),
          OrbitSystems.run_if(in_state(OrbitState::Active)),
          PanSystems.run_if(in_state(PanState::Active)),
          CameraInputSystems::Keyboard
            .in_set(input::Unfocused)
            .run_if(mouse_movement_active),
        ),
      )
      .register_type::<ActiveEditorCamera>()
      .register_type::<view2d::CameraSettings>()
      .register_type::<view2d::CameraState>()
      .insert_state(ActiveEditorCamera::None)
      .insert_state(OrbitState::Inactive)
      .insert_state(PanState::Inactive)
      .add_systems(OnEnter(ActiveEditorCamera::Cam2D), view2d::enable)
      .add_systems(OnExit(ActiveEditorCamera::Cam2D), view2d::save_settings)
      .add_systems(OnEnter(ActiveEditorCamera::Cam3D), view3d::enable)
      .add_systems(OnExit(ActiveEditorCamera::Cam3D), view3d::save_settings)
      .add_systems(
        OnEnter(EditorState::Exiting),
        (view2d::save_settings, view3d::save_settings),
      )
      .add_systems(
        Update,
        (
          (
            view2d::released_mouse_input_actions,
            view2d::mouse_input_actions,
            (view2d::pan_system.in_set(PanSystems), view2d::zoom_system),
          )
            .chain()
            .in_set(CameraInputSystems::Mouse),
          view2d::movement_system.in_set(CameraInputSystems::Keyboard),
        )
          .chain()
          .in_set(View2d),
      )
      .add_systems(
        Update,
        (
          (
            view3d::released_mouse_input_actions,
            view3d::mouse_input_actions,
            (
              view3d::orbit_system.in_set(OrbitSystems),
              view3d::pan_system.in_set(PanSystems),
              view3d::zoom_system,
            ),
          )
            .chain()
            .in_set(CameraInputSystems::Mouse),
          view3d::movement_system.in_set(CameraInputSystems::Keyboard),
        )
          .chain()
          .in_set(View3d),
      );
  }
}

#[derive(SystemSet, PartialEq, Eq, Hash, Clone, Debug)]
enum CameraInputSystems {
  Keyboard,
  Mouse,
}

pub fn mouse_hovered_in_editor_view(
  q_editor_view_ui_info: Query<&UiState, With<EditorView>>,
) -> bool {
  q_editor_view_ui_info.iter().any(UiState::hovered)
}

fn mouse_movement_active(orbit: Res<State<OrbitState>>, pan: Res<State<PanState>>) -> bool {
  *orbit == OrbitState::Active || *pan == PanState::Active
}

pub fn add_game_camera<C>(app: &mut App)
where
  C: Component + Reflect + TypePath,
{
  app
    .register_type::<GameView<C>>()
    .add_systems(PostStartup, cam::disable_camera::<C>)
    .add_systems(
      Update,
      (
        cam::render_2d_cameras::<C>
          .in_set(View2d)
          .run_if(cam::should_show_cameras),
        cam::render_3d_cameras::<C>
          .in_set(View3d)
          .run_if(cam::should_show_cameras),
      ),
    );
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
