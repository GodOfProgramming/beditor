use crate::{AssetRef, EditorState, private::UserHidden};
use bevy::{dev_tools::infinite_grid::InfiniteGrid, ecs::entity_disabling::Disabled, prelude::*};
use serde::{Deserialize, Serialize};

pub struct EditorScenePlugin;

impl Plugin for EditorScenePlugin {
	fn build(&self, app: &mut App) {
		app
			.add_systems(OnEnter(EditorState::Editing), show_infinite_grid)
			.add_systems(OnExit(EditorState::Editing), remove_infinite_grid);
	}
}

fn show_infinite_grid(
	mut commands: Commands,
	q_grids: Query<Entity, (With<InfiniteGrid>, With<UserHidden>, Allow<Disabled>)>,
) {
	for entity in &q_grids {
		commands.entity(entity).remove::<Disabled>();
	}
}

fn remove_infinite_grid(
	mut commands: Commands,
	q_grids: Query<Entity, (With<InfiniteGrid>, With<UserHidden>)>,
) {
	for entity in &q_grids {
		commands.entity(entity).insert(Disabled);
	}
}

#[derive(Component, Default)]
#[require(
	Transform,
	Visibility,
	InheritedVisibility,
	Node,
	Name::new("New Scene")
)]
pub struct GameScene;

#[derive(Reflect, Serialize, Deserialize)]
pub struct Map {
	camera: CameraMode,
	resources: Vec<AssetRef>,
	hierarchy: SceneComponent,
}

#[derive(Reflect, Serialize, Deserialize)]
enum CameraMode {
	Camera3D,
	Camera2D,
}

#[derive(Reflect, Serialize, Deserialize)]
struct SceneComponent {
	value: AssetRef,
	children: Vec<SceneComponent>,
}
