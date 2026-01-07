use bevy::{
	ecs::archetype::Archetype,
	picking::pointer::PointerId,
	prelude::*,
	window::{Monitor, PrimaryWindow},
};
use std::any::TypeId;

/// Guesses an appropriate entity name like `Light (6)` or falls back to `Entity (8)`
pub fn guess_entity_name(world: &World, entity: Entity) -> String {
	match world.get_entity(entity) {
		Ok(entity_ref) => {
			if let Some(name) = entity_ref.get::<Name>() {
				return format!("{} ({})", name.as_str(), entity);
			}

			maybe_component_entity_name(world, entity, entity_ref.archetype())
		}
		Err(_) => format!("Entity {} (inexistent)", entity.index()),
	}
}

pub fn maybe_component_entity_name(world: &World, entity: Entity, archetype: &Archetype) -> String {
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
			return format!("Entity ({entity}) - {name}");
		}
	}

	format!("Entity ({entity})")
}
