use bevy::prelude::*;

pub fn insert_bundle_from_world<T: Bundle + FromWorld>() -> impl EntityCommand {
	move |mut entity: EntityWorldMut| {
		let value = entity.world_scope(|world| T::from_world(world));
		entity.insert(value);
	}
}
