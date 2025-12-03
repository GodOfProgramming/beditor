use bevy::{ecs::entity::EntityHashSet, prelude::*, utils::TypeIdMap};
use ron::ser::PrettyConfig;
use serde::Serialize;
use std::any::TypeId;

pub struct SceneSerializationExtensionsPlugin;

impl Plugin for SceneSerializationExtensionsPlugin {
	fn build(&self, app: &mut App) {
		app.init_resource::<RelationshipRegistry>();
	}
}

type RelationshipExtractor = fn(entity: Entity, world: &mut World) -> Vec<Entity>;

#[derive(Resource, Deref)]
struct RelationshipRegistry {
	extractors: TypeIdMap<RelationshipExtractor>,
}

impl Default for RelationshipRegistry {
	fn default() -> Self {
		Self {
			extractors: default(),
		}
		.with_registration::<Children>()
	}
}

impl RelationshipRegistry {
	pub fn with_registration<C>(mut self) -> Self
	where
		C: Component + RelationshipTarget,
	{
		self.add_registration::<C>();
		self
	}

	pub fn add_registration<C>(&mut self) -> &mut Self
	where
		C: Component + RelationshipTarget,
	{
		if self.extractors.contains_key(&TypeId::of::<C>()) {
			return self;
		}

		self.extractors.insert(TypeId::of::<C>(), |entity, world| {
			world
				.entity(entity)
				.get::<C>()
				.map(|component| component.iter().collect::<Vec<_>>())
				.unwrap_or_default()
		});

		self
	}
}

pub fn serialize_to_scene(entity: Entity, world: &mut World) -> Result<Vec<u8>> {
	world.resource_scope(|world, registry: Mut<RelationshipRegistry>| {
		let all_entities = EntityHashSet::from_iter(gather_relatives(entity, world, &registry));
		let scene = DynamicSceneBuilder::from_world(world)
			.extract_entities(all_entities.into_iter())
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
	})
}

fn gather_relatives(
	entity: Entity,
	world: &mut World,
	registry: &RelationshipRegistry,
) -> Vec<Entity> {
	let relative_groups =
		registry
			.values()
			.fold(Vec::with_capacity(registry.len()), |mut list, extractor| {
				list.push((extractor)(entity, world));
				list
			});

	let mut second_relatives = Vec::new();

	for relatives in relative_groups {
		for relative in &relatives {
			if entity == *relative {
				continue;
			}

			let seconds = gather_relatives(*relative, world, registry);
			second_relatives.push(seconds);
		}
		second_relatives.push(relatives);
	}

	std::iter::once(entity)
		.chain(second_relatives.into_iter().flatten())
		.collect()
}
