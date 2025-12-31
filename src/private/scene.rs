use crate::{
	EditorState, SimulationState,
	private::{EditorInternalQuery, EditorOwned, Simulated, ui::InspectorSelection},
	util::one_of,
};
use bevy::{
	ecs::{entity::EntityHashSet, entity_disabling::Disabled},
	prelude::*,
	scene::SceneInstance,
	utils::TypeIdMap,
};
use bevy_infinite_grid::{InfiniteGrid, InfiniteGridBundle};
use ron::ser::PrettyConfig;
use serde::Serialize;
use std::any::TypeId;

pub struct EditorScenePlugin;

impl Plugin for EditorScenePlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<RelationshipRegistry>()
			.add_observer(one_of::<PrimaryScene>)
			.add_systems(Startup, startup)
			.add_systems(
				OnEnter(EditorState::Editing),
				(show_infinite_grid, restore_scene),
			)
			.add_systems(OnExit(EditorState::Editing), remove_infinite_grid)
			.add_systems(OnEnter(EditorState::SimulationPrep), on_sim_prep)
			.add_systems(First, mark_entities);
	}
}

#[derive(Component, Reflect, Default, Clone, Copy)]
#[require(
  SceneRoot,
  Transform,
  Visibility,
  Name = Name::new("Scene")
)]
#[reflect(Clone, Default)]
pub struct PrimaryScene;

fn startup(mut commands: Commands) {
	commands.spawn(PrimaryScene);
}

fn show_infinite_grid(mut commands: Commands) {
	commands.spawn((
		EditorOwned,
		Name::new("Infinite Grid"),
		InfiniteGridBundle::default(),
	));
}

fn remove_infinite_grid(
	mut commands: Commands,
	q_grids: EditorInternalQuery<Entity, With<InfiniteGrid>>,
) {
	for entity in &q_grids {
		commands.entity(entity).despawn();
	}
}

fn mark_entities(
	mut commands: Commands,
	q_unmarked_entities: Query<Entity, (Without<Simulated>, Without<EditorOwned>)>,
	state: Res<State<EditorState>>,
) {
	match state.get() {
		EditorState::Editing => {
			for entity in &q_unmarked_entities {
				commands.entity(entity).insert(EditorOwned);
			}
		}
		EditorState::SimulationPrep | EditorState::Simulating(_) => {
			for entity in &q_unmarked_entities {
				commands.entity(entity).insert(Simulated);
			}
		}
		_ => {}
	}
}

fn on_sim_prep(
	mut commands: Commands,
	q_scene_roots: Query<Entity, With<SceneInstance>>,
	mut next_state: ResMut<NextState<EditorState>>,
	mut selected_entities: ResMut<InspectorSelection>,
) {
	for entity in &q_scene_roots {
		commands
			.entity(entity)
			.clone_and_spawn_with_opt_out(|builder| {
				builder.add_observers(true);
				builder.linked_cloning(true);
			})
			.insert(Simulated);

		commands
			.entity(entity)
			.insert_recursive::<Children>(Disabled);
	}

	next_state.set(EditorState::Simulating(SimulationState::Live));

	if let Some(event) = selected_entities.clear() {
		commands.trigger(event);
	}
}

fn restore_scene(
	mut commands: Commands,
	q_simulated_entities: Query<Entity, With<Simulated>>,
	q_roots: Query<(Entity, Has<Disabled>), (With<SceneRoot>, Allow<Disabled>)>,
) {
	for entity in &q_simulated_entities {
		if let Ok(mut entity) = commands.get_entity(entity) {
			entity.despawn();
		}
	}

	for entity in q_roots
		.iter()
		.filter_map(|(entity, disabled)| disabled.then_some(entity))
	{
		if let Ok(mut entity) = commands.get_entity(entity) {
			entity.queue_silenced(move |mut entity: EntityWorldMut| {
				entity.remove_recursive::<Children, Disabled>();
			});
		}
	}
}

////////////////////////////////////////////////////////////////////////////////

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
