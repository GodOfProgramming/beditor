use bevy::{ecs::system::SystemParam, prelude::*};
use brefabs::{Prefabs, StaticPrefab};
use uuid::{Uuid, uuid};

const CUBE_MESH_UUID: Uuid = uuid!("feee5079-0028-4558-be58-d0a4d343245a");
const RECT_MESH_UUID: Uuid = uuid!("c4b4b530-ff0a-4d49-8000-f48bf7a1cd99");

const STANDARD_MATERIAL_UUID: Uuid = uuid!("04e6554c-596c-4f76-b851-682a86539e71");
const COLOR_MATERIAL_UUID: Uuid = uuid!("47cd28d6-d14f-40a7-be9f-8af97577694b");

const BASE_COLOR: Color = Color::srgb_u8(124, 144, 255);

pub fn add_primitives(world: &mut World) {
	world.resource_scope(|world, mut prefabs: Mut<Prefabs>| {
		prefabs.register_static_prefab::<Cube>(world);
	});

	let mut meshes = world.resource_mut::<Assets<Mesh>>();

	for (id, mesh) in [
		(CUBE_MESH_UUID, Cuboid::default().into()),
		(RECT_MESH_UUID, Rectangle::default().into()),
	] {
		meshes.insert(AssetId::from(id), mesh).ok();
	}

	let mut mats = world.resource_mut::<Assets<StandardMaterial>>();

	mats
		.insert(
			AssetId::Uuid {
				uuid: STANDARD_MATERIAL_UUID,
			},
			StandardMaterial {
				base_color: BASE_COLOR,
				..default()
			},
		)
		.ok();

	let mut mats = world.resource_mut::<Assets<ColorMaterial>>();

	mats
		.insert(
			AssetId::Uuid {
				uuid: STANDARD_MATERIAL_UUID,
			},
			ColorMaterial::from(BASE_COLOR),
		)
		.ok();
}

#[derive(SystemParam)]
struct SharedParams<'w> {
	meshes: ResMut<'w, Assets<Mesh>>,
	std_mats: ResMut<'w, Assets<StandardMaterial>>,
	color_mats: ResMut<'w, Assets<ColorMaterial>>,
}

#[derive(Bundle, Reflect, Clone)]
#[reflect(Clone)]
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
					.get_strong_handle(AssetId::from(CUBE_MESH_UUID))
					.unwrap(),
			),
			material: MeshMaterial3d(
				params
					.std_mats
					.get_strong_handle(AssetId::from(STANDARD_MATERIAL_UUID))
					.unwrap(),
			),
		}
	}
}

#[derive(Bundle, Reflect, Clone)]
#[reflect(Clone)]
struct Square {
	mesh: Mesh2d,
	material: MeshMaterial2d<ColorMaterial>,
}

impl StaticPrefab for Square {
	type Params<'w, 's> = SharedParams<'w>;

	fn spawn(_entity: Entity, _name: Option<Name>, mut params: Self::Params<'_, '_>) -> Self {
		Self {
			mesh: Mesh2d(
				params
					.meshes
					.get_strong_handle(AssetId::from(RECT_MESH_UUID))
					.unwrap(),
			),
			material: MeshMaterial2d(
				params
					.color_mats
					.get_strong_handle(AssetId::from(COLOR_MATERIAL_UUID))
					.unwrap(),
			),
		}
	}
}
