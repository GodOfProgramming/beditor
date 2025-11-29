use beditor::{
  Editor,
  brefabs::{NoParams, Prefab},
};
use bevy::prelude::*;
use serde_derive::Deserialize;

fn main() {
  let mut editor = Editor::default();

  editor.prefabs().add_prefab::<SamplePrefab>();

  editor.run();
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
