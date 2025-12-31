use std::marker::PhantomData;

use bevy::prelude::*;
use smallvec::SmallVec;

#[derive(Default, Clone, Copy)]
pub enum SingletonBehavior {
	#[default]
	RemoveOther,
	RemoveSelf,
}

pub struct SingletonPlugin<C: Component> {
	behavior: SingletonBehavior,
	_pd: PhantomData<C>,
}

impl<C: Component> Default for SingletonPlugin<C> {
	fn default() -> Self {
		Self {
			behavior: default(),
			_pd: default(),
		}
	}
}

impl<C: Component> SingletonPlugin<C> {
	pub fn new(behavior: SingletonBehavior) -> Self {
		Self {
			behavior,
			..default()
		}
	}
}

impl<C: Component> Plugin for SingletonPlugin<C> {
	fn build(&self, app: &mut App) {
		let behavior = self.behavior;
		app.add_observer(
			move |event: On<Add, C>, mut commands: Commands, q_components: Query<Entity, With<C>>| {
				if q_components.count() > 1 {
					let entity = event.event_target();
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
			},
		);
	}
}
