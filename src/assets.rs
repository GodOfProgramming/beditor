use bevy::{ecs::system::SystemParam, prelude::*};
use brefabs::{Prefabs, StaticPrefab};
use uuid::{Uuid, uuid};

const MATERIAL_BASIC_UUID: Uuid = uuid!("04e6554c-596c-4f76-b851-682a86539e71");
const MESH_CUBE_UUID: Uuid = uuid!("feee5079-0028-4558-be58-d0a4d343245a");

pub fn add_primitives(world: &mut World) {
	world.resource_scope(|world, mut prefabs: Mut<Prefabs>| {
		prefabs.register_static_prefab::<Cube>(world);
	});

	let mut meshes = world.resource_mut::<Assets<Mesh>>();

	meshes
		.insert(
			AssetId::from(MESH_CUBE_UUID),
			Cuboid::new(1.0, 1.0, 1.0).into(),
		)
		.ok();

	let mut materials = world.resource_mut::<Assets<StandardMaterial>>();

	materials
		.insert(
			AssetId::Uuid {
				uuid: MATERIAL_BASIC_UUID,
			},
			StandardMaterial::default(),
		)
		.ok();
}

#[derive(SystemParam)]
struct SharedParams<'w> {
	meshes: ResMut<'w, Assets<Mesh>>,
	materials: ResMut<'w, Assets<StandardMaterial>>,
}

#[derive(Bundle, Reflect)]
struct Cube {
	mesh: Mesh3d,
	material: MeshMaterial3d<StandardMaterial>,
}

impl StaticPrefab for Cube {
	type Params<'w, 's> = SharedParams<'w>;

	fn spawn(_entity: Entity, _name: Option<Name>, mut params: Self::Params<'_, '_>) -> Self {
		Self {
			mesh: Mesh3d(
				params
					.meshes
					.get_strong_handle(AssetId::from(MESH_CUBE_UUID))
					.unwrap(),
			),
			material: MeshMaterial3d(
				params
					.materials
					.add(StandardMaterial::from_color(Color::srgb_u8(124, 144, 255))),
			),
		}
	}
}
