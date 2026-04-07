use beditor::prelude::*;
use bevy::prelude::*;
use mimalloc::MiMalloc;
use serde::{Deserialize, Serialize};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
	App::new()
		.add_plugins(EditorPlugin::new().register_game_camera::<GameCamera>())
		.add_systems(Startup, startup)
		.run();
}

#[derive(Component, Reflect, Default, Identifiable)]
#[id("d0d75fdd-e1b9-4eac-86fc-6eaab8865bad")]
struct GameCamera;

fn startup(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
) {
	commands.spawn((
		Name::new("Base"),
		Mesh3d(meshes.add(Circle::new(4.0))),
		MeshMaterial3d(materials.add(Color::WHITE)),
		Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
	));
	commands.spawn((
		Name::new("Cube"),
		Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
		MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
		Transform::from_xyz(0.0, 0.5, 0.0),
	));
	commands.spawn((
		Name::new("Light"),
		PointLight {
			shadows_enabled: true,
			..default()
		},
		Transform::from_xyz(4.0, 8.0, 4.0),
	));
	commands.spawn((
		Name::new("Game Camera"),
		GameCamera,
		Camera3d::default(),
		Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
	));
}

#[derive(Reflect, Serialize, Deserialize, EditorAsset)]
#[ns("example")]
enum ExampleContent {
	Cube {
		mesh: AssetRef,
		material: AssetRef,
		name: String,
	},
}

impl ContentHandlers for ExampleContent {
	fn insert(&self, entity: Entity, world: &mut World) {
		match self {
			ExampleContent::Cube {
				mesh,
				material,
				name,
			} => {
				let mesh = mesh.get_handle::<Mesh>(world);
				let material = material.get_handle::<StandardMaterial>(world);

				world.entity_mut(entity).insert((
					Name::new(name.clone()),
					Transform::IDENTITY,
					Mesh3d(mesh),
					MeshMaterial3d(material),
				));
			}
		}
	}
}
