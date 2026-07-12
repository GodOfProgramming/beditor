use bevy::prelude::*;
use notify::Notification;
use serde_json::json;

pub fn insert_bundle_from_world<T: Bundle + FromWorld>() -> impl EntityCommand {
	move |mut entity: EntityWorldMut| {
		let id = entity.id();
		let is_despawned = entity.is_despawned();

		let Ok(value) = entity.world_scope(|world| -> Result<T, ()> {
			if is_despawned {
				world.trigger(
					Notification::error("Tried to insert value on despawned entity").with_context(json!({
						"entity": id
					})),
				);
				Err(())?;
			}

			Ok(T::from_world(world))
		}) else {
			return;
		};

		entity.insert(value);
	}
}
