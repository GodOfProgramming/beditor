use crate::{
	EditorState, SimulationState,
	private::{EditorInternalQuery, EditorScene, UserHidden, ui::input::KeyboardFocus},
};
use bevy::prelude::*;
use leafwing_input_manager::{
	Actionlike,
	plugin::InputManagerPlugin,
	prelude::{ActionState, InputMap},
};

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum EditorActions {
	SimulationToggle,
	StopSimulation,
}

pub struct EditorInputPlugin;

impl EditorInputPlugin {}

impl Plugin for EditorInputPlugin {
	fn build(&self, app: &mut App) {
		app
			.add_plugins(InputManagerPlugin::<EditorActions>::default())
			.add_observer(on_new_scene)
			.add_systems(
				Update,
				global_input_actions.run_if(in_state(KeyboardFocus::Unfocused)),
			);
	}
}

fn on_new_scene(event: On<Add, EditorScene>, mut commands: Commands) {
	let inputs = InputMap::default()
		.with(EditorActions::SimulationToggle, KeyCode::F5)
		.with(EditorActions::StopSimulation, KeyCode::Escape);

	commands.spawn((
		Name::new("Editor Input"),
		UserHidden,
		inputs,
		ChildOf(event.event_target()),
	));
}

pub fn global_input_actions(
	q_action_states: EditorInternalQuery<&ActionState<EditorActions>>,
	current_state: Res<State<EditorState>>,
	mut next_editor_state: ResMut<NextState<EditorState>>,
) {
	for action_state in &q_action_states {
		if action_state.just_pressed(&EditorActions::SimulationToggle) {
			if *current_state.get() == EditorState::Editing {
				next_editor_state.set(EditorState::Simulating(SimulationState::Live));
			} else {
				next_editor_state.set(EditorState::Simulating(SimulationState::Idle));
			}
		}

		if *current_state.get() != EditorState::Editing
			&& action_state.just_pressed(&EditorActions::StopSimulation)
		{
			next_editor_state.set(EditorState::Editing);
		}
	}
}
