use crate::{
	private::EditorInternalQuery,
	util::world::{RestrictedWorldView, WorldView},
};
use bevy::{
	ecs::{archetype::Archetype, system::entity_command},
	picking::pointer::PointerId,
	prelude::*,
	window::{Monitor, PrimaryWindow},
};
use std::any::TypeId;

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

pub fn insert_bundle_from_world<T: Bundle + FromWorld>() -> impl EntityCommand {
	move |mut entity: EntityWorldMut| {
		let value = entity.world_scope(|world| T::from_world(world));
		entity.insert(value);
	}
}

/// Guesses an appropriate entity name like `Light (6)` or falls back to `Entity (8)`
pub fn guess_entity_name(world: &World, entity: Entity) -> String {
	match world.get_entity(entity) {
		Ok(entity_ref) => {
			if let Some(name) = entity_ref.get::<Name>() {
				return format!("{} ({})", name.as_str(), entity);
			}

			guess_entity_name_inner(world, entity, entity_ref.archetype())
		}
		Err(_) => format!("Entity {} (inexistent)", entity.index()),
	}
}

pub(crate) fn guess_entity_name_restricted<W>(
	world_view: &RestrictedWorldView<W>,
	entity: Entity,
) -> String
where
	W: WorldView,
{
	match world_view.world().get_entity(entity) {
		Ok(cell) => {
			if world_view.allows_access_to_component((entity, std::any::TypeId::of::<Name>())) {
				// SAFETY: we have access and don't keep reference
				if let Some(name) = cell.get::<Name>() {
					return format!("{} ({})", name.as_str(), entity);
				}
			}
			guess_entity_name_inner(world_view.world(), entity, cell.archetype())
		}
		Err(_) => format!("Entity {} (inexistent)", entity.index()),
	}
}

fn guess_entity_name_inner(world: &World, entity: Entity, archetype: &Archetype) -> String {
	let associations = [
		(TypeId::of::<PrimaryWindow>(), "Primary Window"),
		(TypeId::of::<Camera3d>(), "Camera3d"),
		(TypeId::of::<Camera2d>(), "Camera2d"),
		(TypeId::of::<PointLight>(), "PointLight"),
		(TypeId::of::<DirectionalLight>(), "DirectionalLight"),
		(TypeId::of::<Text>(), "Text"),
		(TypeId::of::<Node>(), "Node"),
		(TypeId::of::<MeshMaterial3d<StandardMaterial>>(), "Pbr Mesh"),
		(TypeId::of::<Window>(), "Window"),
		(TypeId::of::<Observer>(), "Observer"),
		(TypeId::of::<Monitor>(), "Monitor"),
		(TypeId::of::<PointerId>(), "Pointer"),
	];

	let component_types = archetype.components().iter().filter_map(|id| {
		world
			.components()
			.get_info(*id)
			.and_then(|info| info.type_id())
	});

	for component_type in component_types {
		let found_name = associations.iter().find_map(|&(type_id, simplified_name)| {
			(type_id == component_type).then_some(simplified_name)
		});

		if let Some(name) = found_name {
			return format!("{name} ({entity})");
		}
	}

	format!("Entity ({entity})")
}
