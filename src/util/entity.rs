use std::any::TypeId;

use bevy::{
	ecs::{archetype::Archetype, prelude::*},
	prelude::*,
};

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

pub(crate) fn guess_entity_name_restricted(world: &World, entity: Entity) -> String {
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

fn guess_entity_name_inner(world: &World, entity: Entity, archetype: &Archetype) -> String {
	#[rustfmt::skip]
	let associations = [
		(TypeId::of::<bevy::window::PrimaryWindow>(), "Primary Window"),
		(TypeId::of::<bevy::camera::Camera3d>(), "Camera3d"),
		(TypeId::of::<bevy::camera::Camera2d>(), "Camera2d"),
		(TypeId::of::<bevy::light::PointLight>(), "PointLight"),
		(TypeId::of::<bevy::light::DirectionalLight>(), "DirectionalLight"),
		(TypeId::of::<bevy::ui::widget::Text>(), "Text"),
		(TypeId::of::<bevy::ui::Node>(), "Node"),
		(TypeId::of::<bevy::pbr::MeshMaterial3d<bevy::pbr::StandardMaterial>>(), "Pbr Mesh"),
		(TypeId::of::<bevy::window::Window>(), "Window"),
		(TypeId::of::<bevy::ecs::observer::Observer>(), "Observer"),
		(TypeId::of::<bevy::window::Monitor>(), "Monitor"),
		(TypeId::of::<bevy::picking::pointer::PointerId>(), "Pointer"),
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
