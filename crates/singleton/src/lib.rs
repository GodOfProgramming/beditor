use std::marker::PhantomData;

use bevy::{ecs::query::QueryFilter, prelude::*};
use smallvec::SmallVec;

#[derive(Default, Clone, Copy)]
pub enum SingletonBehavior {
	#[default]
	RemoveOther,
	RemoveSelf,
}

pub struct SingletonPlugin<C: Component, Q: QueryFilter = ()> {
	behavior: SingletonBehavior,
	_pd: PhantomData<(C, Q)>,
}

impl<C: Component, Q: QueryFilter> Default for SingletonPlugin<C, Q> {
	fn default() -> Self {
		Self {
			behavior: default(),
			_pd: default(),
		}
	}
}

impl<C: Component, Q: QueryFilter> SingletonPlugin<C, Q> {
	pub fn new(behavior: SingletonBehavior) -> Self {
		Self {
			behavior,
			..default()
		}
	}
}

impl<C: Component, Q: QueryFilter> Plugin for SingletonPlugin<C, Q>
where
	Q: 'static + Send + Sync,
{
	fn build(&self, app: &mut App) {
		let behavior = self.behavior;
		app.add_observer(
			move |event: On<Add, C>, commands: Commands, q_components: Query<Entity, (With<C>, Q)>| {
				on_singleton(event.event_target(), behavior, commands, q_components);
			},
		);
	}
}

fn on_singleton<C: Component, Q: QueryFilter>(
	entity: Entity,
	behavior: SingletonBehavior,
	mut commands: Commands,
	q_components: Query<Entity, (With<C>, Q)>,
) {
	if q_components.count() > 1 {
		match behavior {
			SingletonBehavior::RemoveOther => {
				for other in q_components
					.iter()
					.filter(|&other| entity != other)
					.collect::<SmallVec<[_; 4]>>()
				{
					info!(
						entity = other.to_string(),
						r#type = std::any::type_name::<C>(),
						"Despawning singleton",
					);
					commands.entity(other).despawn();
				}
			}
			SingletonBehavior::RemoveSelf => {
				info!(
					entity = entity.to_string(),
					r#type = std::any::type_name::<C>(),
					"Despawning singleton",
				);
				commands.entity(entity).despawn();
			}
		}
	}
}
