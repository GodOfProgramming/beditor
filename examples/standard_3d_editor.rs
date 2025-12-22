use beditor::{brefabs::StaticPrefab, prelude::*};
use bevy::{ecs::system::SystemParam, prelude::*};
use brefabs::PrefabPlugin;

fn main() {
	App::new()
		.add_plugins((
			EditorPlugin::new().register_game_camera::<GameCamera>(),
			PrefabPlugin::default().with_static_prefab::<Cube>(),
		))
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

#[derive(Bundle, Reflect)]
struct Cube {
	mesh: Mesh3d,
	material: MeshMaterial3d<StandardMaterial>,
	transform: Transform,
}

struct Spiral {
	theta: f32,
	r: f32,
	h: f32,
}

impl Default for Spiral {
	fn default() -> Self {
		Self {
			theta: 0.0,
			r: 2.0,
			h: 0.0,
		}
	}
}

#[derive(SystemParam)]
struct CubeParams<'w, 's> {
	meshes: ResMut<'w, Assets<Mesh>>,
	materials: ResMut<'w, Assets<StandardMaterial>>,
	spiral: Local<'s, Spiral>,
}

impl StaticPrefab for Cube {
	type Params<'w, 's> = CubeParams<'w, 's>;

	fn spawn(_entity: Entity, _name: Option<Name>, mut params: Self::Params<'_, '_>) -> Self {
		let offset = Vec2::new(
			params.spiral.r * params.spiral.theta.cos(),
			params.spiral.r * params.spiral.theta.sin(),
		);

		params.spiral.theta += 30.0f32.to_radians();
		params.spiral.h += 0.5;

		Self {
			mesh: Mesh3d(params.meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
			material: MeshMaterial3d(params.materials.add(Color::srgb_u8(124, 144, 255))),
			transform: Transform::from_xyz(offset.x, params.spiral.h, offset.y),
		}
	}
}
