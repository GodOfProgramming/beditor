mod private;

use bevy::prelude::*;

#[derive(Reflect, Default)]
pub struct WorldManifest {
	#[reflect(ignore)]
	state: Vec<Box<dyn 'static + PartialReflect>>,
	entries: Vec<Handle<Scene>>,
}

impl WorldManifest {}

pub struct WorldManifestPlugin;

impl Plugin for WorldManifestPlugin {
	fn build(&self, app: &mut App) {}
}
