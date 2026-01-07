mod private;

use bevy::{ecs::system::SystemParam, prelude::*};
use ron::ser::PrettyConfig;
use serde::{Serialize, de::DeserializeSeed};
use std::{fmt::Write, path::Path};

#[derive(Reflect, Default)]
pub struct WorldManifest {
	#[reflect(ignore)]
	state: Vec<Box<dyn 'static + PartialReflect>>,
	entries: Vec<Handle<Scene>>,
}

#[derive(SystemParam)]
pub struct WorldManifests<'w> {
	asset_server: Res<'w, AssetServer>,
	type_registry: Res<'w, AppTypeRegistry>,
}

impl WorldManifests<'_> {
	pub fn load(&self, path: impl AsRef<Path>) -> Result<WorldManifest> {
		let manifest_data = std::fs::read_to_string(path)?;
		let type_registry = self.type_registry.read();
		let de = private::serde::ManifestDeserializer {
			type_registry: &type_registry,
			asset_server: &self.asset_server,
		};
		let mut ron_de = ron::de::Deserializer::from_str(&manifest_data)?;

		let res = de.deserialize(&mut ron_de)?;

		Ok(res)
	}

	pub fn save(&self, manifest: &WorldManifest, writer: impl Write) -> Result {
		let type_registry = self.type_registry.read();
		let ser = private::serde::ManifestSerializer {
			type_registry: &type_registry,
			manifest,
		};

		let mut ron_ser =
			ron::ser::Serializer::new(writer, Some(PrettyConfig::new().struct_names(true)))?;

		ser.serialize(&mut ron_ser)?;

		Ok(())
	}
}
