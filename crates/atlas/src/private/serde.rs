use crate::WorldManifest;
use bevy::{
	asset::{AssetPath, uuid::Uuid},
	prelude::*,
	reflect::TypeRegistry,
	scene::serde::{SceneMapDeserializer, SceneMapSerializer},
};
use nameof::name_of;
use serde::{
	Deserialize, Deserializer, Serialize, Serializer,
	de::{DeserializeSeed, Error as DeError, Visitor},
	ser::{Error as SerError, SerializeStruct},
};

pub const MANIFEST_STATE: &str = name_of!(state in WorldManifest);
pub const MANIFEST_ENTRIES: &str = name_of!(entries in WorldManifest);
pub const MANIFEST_FIELDS: &[&str] = &[MANIFEST_STATE, MANIFEST_ENTRIES];

#[derive(Serialize, Deserialize)]
enum Entry {
	AssetPath(AssetPath<'static>),
	Uuid(Uuid),
}

pub struct ManifestSerializer<'r, 'm> {
	pub type_registry: &'r TypeRegistry,
	pub manifest: &'m WorldManifest,
}

impl<'r, 'm> Serialize for ManifestSerializer<'r, 'm> {
	fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let mut state = serializer.serialize_struct(name_of!(type WorldManifest), 2)?;
		state.serialize_field(
			MANIFEST_STATE,
			&SceneMapSerializer {
				entries: &self.manifest.state,
				registry: self.type_registry,
			},
		)?;

		let mut entries = Vec::with_capacity(self.manifest.entries.len());
		for entry in &self.manifest.entries {
			if let Some(path) = entry.path() {
				entries.push(Entry::AssetPath(path.clone()));
			} else if let Handle::Uuid(uuid, ..) = entry {
				entries.push(Entry::Uuid(*uuid));
			} else {
				return Err(SerError::custom(format_args!(
					"Scene in manifest does not contain a valid handle"
				)))?;
			}
		}
		state.serialize_field(MANIFEST_ENTRIES, &entries)?;

		state.end()
	}
}

pub struct ManifestDeserializer<'r> {
	pub type_registry: &'r TypeRegistry,
	pub asset_server: &'r AssetServer,
}

impl<'de, 'r> DeserializeSeed<'de> for ManifestDeserializer<'r> {
	type Value = WorldManifest;

	fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
	where
		D: Deserializer<'de>,
	{
		deserializer.deserialize_struct(
			name_of!(type WorldManifest),
			MANIFEST_FIELDS,
			ManifestVisitor {
				type_registry: self.type_registry,
				asset_server: self.asset_server,
			},
		)
	}
}

struct ManifestVisitor<'r> {
	type_registry: &'r TypeRegistry,
	asset_server: &'r AssetServer,
}

impl<'de, 't> Visitor<'de> for ManifestVisitor<'t> {
	type Value = WorldManifest;

	fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
		formatter.write_str("manifest struct")
	}

	fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
	where
		A: serde::de::MapAccess<'de>,
	{
		let mut state = Vec::new();
		let mut entries = Vec::new();

		while let Some(key) = map.next_key()? {
			match key {
				MANIFEST_STATE => {
					state = map.next_value_seed(SceneMapDeserializer {
						registry: self.type_registry,
					})?;
				}
				MANIFEST_ENTRIES => {
					let asset_paths = map.next_value::<Vec<Entry>>()?;

					for path in asset_paths {
						match path {
							Entry::AssetPath(asset_path) => {
								let handle = self.asset_server.load::<Scene>(asset_path);
								entries.push(handle);
							}
							Entry::Uuid(uuid) => {
								entries.push(Handle::from(uuid));
							}
						}
					}
				}
				k => return Err(DeError::unknown_field(k, MANIFEST_FIELDS)),
			}
		}

		Ok(WorldManifest { state, entries })
	}
}
