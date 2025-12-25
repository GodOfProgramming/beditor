use beditor::{
	brefabs::{NoParams, Prefab},
	prelude::*,
};
use bevy::prelude::*;
use brefabs::PrefabPlugin;
use serde::Deserialize;

fn main() {
	App::new()
		.add_plugins((
			EditorPlugin::new(),
			PrefabPlugin::default().with_prefab::<SamplePrefab>(),
		))
		.run();
}

#[derive(Bundle, Reflect)]
struct SamplePrefab {
	name: Name,
}

#[derive(Asset, Reflect, Deserialize, Clone)]
struct SamplePrefabDescriptor {
	name: String,
}

impl Prefab for SamplePrefab {
	const EXTENSIONS: &[&str] = &["ron"];

	const VARIANT_FIELD: Option<&str> = Some("name");

	type Descriptor = SamplePrefabDescriptor;

	type Params<'w, 's> = NoParams<'w, 's>;

	fn spawn(_entity: Entity, desc: Self::Descriptor, _params: Self::Params<'_, '_>) -> Self {
		Self {
			name: Name::new(desc.name),
		}
	}

	fn path() -> impl Into<std::path::PathBuf> {
		"prefabs"
	}
}
