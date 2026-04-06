use bevy::prelude::*;
use uuid::{Uuid, uuid};

const CUBE_MESH_UUID: Uuid = uuid!("feee5079-0028-4558-be58-d0a4d343245a");
const RECT_MESH_UUID: Uuid = uuid!("c4b4b530-ff0a-4d49-8000-f48bf7a1cd99");

const STANDARD_MATERIAL_UUID: Uuid = uuid!("04e6554c-596c-4f76-b851-682a86539e71");
const COLOR_MATERIAL_UUID: Uuid = uuid!("47cd28d6-d14f-40a7-be9f-8af97577694b");

const BASE_COLOR: Color = Color::srgb_u8(124, 144, 255);

#[derive(Default)]
pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(Startup, add_assets);
	}
}

pub fn add_assets(world: &mut World) {
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
				uuid: COLOR_MATERIAL_UUID,
			},
			ColorMaterial::from(BASE_COLOR),
		)
		.ok();
}
