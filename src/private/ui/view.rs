use super::EditorEguiContext;
use crate::private::{EditorInternalFilter, EditorScene, UserHidden};
use bevy::{
	camera::visibility::{Layer, RenderLayers},
	prelude::*,
};
use serde::{Deserialize, Serialize};
use singleton::{SingletonBehavior, SingletonPlugin};

pub const EDITOR_UI_RENDER_LAYER: Layer = 31;

pub struct EditorUiViewPlugin;

impl Plugin for EditorUiViewPlugin {
	fn build(&self, app: &mut App) {
		app
			.add_plugins(
				SingletonPlugin::<EditorWindowCamera, EditorInternalFilter>::new(
					SingletonBehavior::RemoveOther,
				),
			)
			.add_observer(on_new_editor_scene);
	}
}

/// Camera for the entire editor window, including all egui views
#[derive(Default, Component, Reflect)]
#[require(
  UserHidden,
  EditorEguiContext,
  Camera2d,
  Camera,
  RenderLayers = RenderLayers::layer(EDITOR_UI_RENDER_LAYER),
  Name = Name::new("Editor Window Camera"),
  InheritedVisibility,
)]
pub struct EditorWindowCamera;

#[derive(Resource, Reflect, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActiveEditorCamera {
	Cam2D,
	#[default]
	Cam3D,
}

fn on_new_editor_scene(event: On<Add, EditorScene>, mut commands: Commands) {
	commands.spawn((EditorWindowCamera, ChildOf(event.event_target())));
}
