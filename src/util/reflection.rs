pub mod ron;
pub mod serde;

use crate::util::reflection::serde::SerdeRegistry;
use bevy::{prelude::*, reflect::TypeInfo};

pub struct ReflectionExtensionsPlugin;

impl Plugin for ReflectionExtensionsPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<SerdeRegistry>()
			.init_resource::<ReflectDefaultCache>()
			.add_systems(
				First,
				ReflectDefaultCache::rebuild_cache.run_if(resource_changed::<AppTypeRegistry>),
			);
	}
}

#[derive(Resource, Default, Deref)]
pub struct ReflectDefaultCache {
	#[deref]
	inner: Vec<&'static TypeInfo>,
}

impl ReflectDefaultCache {
	fn rebuild_cache(
		mut cache: ResMut<ReflectDefaultCache>,
		app_type_registry: Res<AppTypeRegistry>,
	) {
		let type_registry = app_type_registry.read();

		cache.inner = type_registry
			.iter()
			.filter_map(|t| t.data::<ReflectDefault>().map(|_| t.type_info()))
			.collect();

		cache
			.inner
			.sort_by(|t1, t2| t1.type_path().cmp(t2.type_path()));
	}
}
