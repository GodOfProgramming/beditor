pub mod assets;
pub mod cam;
pub mod input;
pub mod reflection;
pub mod scene;
pub mod ui;
pub mod util;

use bevy::prelude::*;

pub struct PrivatePlugins;

impl Plugin for PrivatePlugins {
	fn build(&self, app: &mut App) {
		app.add_plugins((
			scene::EditorScenePlugin,
			cam::EditorCamPlugin,
			input::EditorInputPlugin,
			ui::EditorUiPlugin,
			reflection::ReflectionExtensionsPlugin,
			assets::AssetsPlugin,
		));
	}
}
