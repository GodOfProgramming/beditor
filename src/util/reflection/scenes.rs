use bevy::prelude::*;
use derive_new::new;
use ron::ser::PrettyConfig;
use serde::Serialize;

pub fn serialize_to_scene(entity: Entity, world: &mut World) -> Result<Vec<u8>> {
	let scene = DynamicSceneBuilder::from_world(world)
		.extract_entity(entity)
		.build();
	let app_type_registry = world.resource::<AppTypeRegistry>().clone();
	let type_registry = app_type_registry.read();
	let scene_ser = bevy::scene::serde::SceneSerializer::new(&scene, &type_registry);

	let mut buf = String::new();
	let mut ron_ser = ron::Serializer::new(
		&mut buf,
		Some(
			PrettyConfig::default()
				.struct_names(true)
				.escape_strings(true),
		),
	)?;
	scene_ser.serialize(&mut ron_ser)?;

	Ok(buf.into_bytes())
}
