use crate::{Editing, EditorSettings, EditorState, ui::KeyboardFocus};
use bevy::prelude::*;
use leafwing_input_manager::{
  Actionlike,
  plugin::InputManagerPlugin,
  prelude::{ActionState, InputMap, MouseScrollAxis},
};

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum EditorActions {
  Play,
  PanCamera,
  OrbitCamera,
  #[actionlike(Axis)]
  Zoom,
  MoveNorth,
  MoveSouth,
  MoveWest,
  MoveEast,
  ToggleUi,
}

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
pub struct Unfocused;

pub struct InputPlugin;

impl InputPlugin {
  fn init_input(mut commands: Commands) {
    let inputs = InputMap::default()
      .with(EditorActions::Play, KeyCode::F5)
      .with(EditorActions::OrbitCamera, MouseButton::Right)
      .with(EditorActions::PanCamera, MouseButton::Middle)
      .with_axis(EditorActions::Zoom, MouseScrollAxis::Y)
      .with(EditorActions::MoveNorth, KeyCode::KeyW)
      .with(EditorActions::MoveSouth, KeyCode::KeyS)
      .with(EditorActions::MoveWest, KeyCode::KeyA)
      .with(EditorActions::MoveEast, KeyCode::KeyD)
      .with(EditorActions::ToggleUi, KeyCode::F11);

    commands.spawn((Name::new("Editor Input"), inputs));
  }
}

impl Plugin for InputPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_plugins(InputManagerPlugin::<EditorActions>::default())
      .configure_sets(
        Update,
        Unfocused
          .in_set(Editing)
          .run_if(in_state(KeyboardFocus::Unfocused)),
      )
      .add_systems(Startup, Self::init_input)
      .add_systems(Update, global_input_actions.in_set(Unfocused));
  }
}

pub fn global_input_actions(
  q_action_states: Query<&ActionState<EditorActions>>,
  current_state: Res<State<EditorState>>,
  mut next_editor_state: ResMut<NextState<EditorState>>,
  mut editor_settings: ResMut<EditorSettings>,
) {
  for action_state in &q_action_states {
    if action_state.just_pressed(&EditorActions::Play) {
      if *current_state.get() == EditorState::Editing {
        next_editor_state.set(EditorState::Testing);
      } else {
        next_editor_state.set(EditorState::Editing);
      }
    }

    if action_state.just_pressed(&EditorActions::ToggleUi) {
      editor_settings.render_ui = !editor_settings.render_ui;
    }
  }
}
