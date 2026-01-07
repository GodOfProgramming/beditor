pub mod egui;
pub mod entity;
pub mod log;
pub mod window;

use crate::{
	EditorState,
	private::{EditorInternalQuery, EditorOwned, SimulationOwned},
};
use bevy::{ecs::system::entity_command, prelude::*};
use common::extensions::bevy::WorldExtensions as _;
use std::borrow::BorrowMut;

pub fn one_of<C: Component>(
	event: On<Add, C>,
	mut commands: Commands,
	q_others: EditorInternalQuery<Entity, With<C>>,
) {
	for entity in q_others.iter().filter(|&e| e != event.event_target()) {
		if let Ok(mut entity) = commands.get_entity(entity) {
			entity.queue_silenced(entity_command::remove::<C>());
		}
	}
}

pub trait WorldExtensions: BorrowMut<World> {
	fn spawn_stateful_entity(&mut self) -> Option<Entity> {
		self.spawn_stateful_entity_bundle(())
	}

	fn spawn_stateful_entity_bundle(&mut self, bundle: impl Bundle) -> Option<Entity> {
		let world = self.borrow_mut();

		match world.state::<EditorState>() {
			EditorState::Editing => Some(world.spawn((EditorOwned, bundle)).id()),
			EditorState::SimulationPrep | EditorState::Simulating(_) => {
				Some(world.spawn((SimulationOwned, bundle)).id())
			}
			_ => None,
		}
	}
}

impl<T> WorldExtensions for T where T: BorrowMut<World> {}
