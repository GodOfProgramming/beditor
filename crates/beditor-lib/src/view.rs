pub mod cam;
pub mod view2d;
pub mod view3d;

use crate::{
  Editing, EditorState, input,
  ui::{
    misc::UiInfo,
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
      );
  }
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
