use crate::{
	EditorState, SimulationState,
	inspector::ui::InspectorSelection,
	private::{
		EditorInternalSingle, EditorOwned, SimulationOwned, UserHidden,
		cam::EditorCamera,
		reflection::{TypeNameDisplayCache, TypeNameDisplayInfo},
		ui::{EditorEguiContext, EditorUiEguiContextPass},
	},
	util::entity::one_of,
};
use bevy::{
	ecs::{entity::EntityHashSet, entity_disabling::Disabled, system::SystemParam},
	prelude::*,
	scene::{SceneInstance, SceneInstanceReady, serde::SceneSerializer},
	utils::TypeIdMap,
};
use bevy_egui::EguiContext;
use bevy_infinite_grid::InfiniteGrid;
use convert_case::{Case, Casing};
use derive_new::new;
use egui_file_dialog::FileDialog;
use notify::Notification;
use ron::ser::PrettyConfig;
use serde::Serialize;
use singleton::{SingletonBehavior, SingletonPlugin};
use smallvec::SmallVec;
use std::{
	any::TypeId,
	env::current_dir,
	path::{Path, PathBuf},
};
use strum::VariantArray;
use strum_macros::{Display, VariantArray};

pub struct EditorScenePlugin;

impl Plugin for EditorScenePlugin {
	fn build(&self, app: &mut App) {
		app
			.add_plugins(SingletonPlugin::<EditorSceneRoot>::new(
				SingletonBehavior::RemoveOther,
			))
			.add_message::<ShowSceneSettings>()
			.init_resource::<RelationshipRegistry>()
			.add_observer(one_of::<ActiveScene>)
			.add_observer(on_new_camera)
			.add_observer(on_new_scene)
			.add_systems(
				OnEnter(EditorState::Editing),
				(show_infinite_grid, restore_scene),
			)
			.add_systems(OnExit(EditorState::Editing), remove_infinite_grid)
			.add_systems(OnEnter(EditorState::SimulationPrep), on_sim_prep)
			.add_systems(First, mark_entities)
			.add_systems(EditorUiEguiContextPass, show_scene_editing_modal);
	}
}

#[derive(Component, Default)]
#[require(Transform, Visibility, Node, ActiveScene, Name::new("New Scene"))]
pub struct EditorSceneRoot;

/// Add this component to any entity to give it the qualifications of becoming a scene
/// Also make it the currently active scene
#[derive(Component, Reflect, Default, Clone, Copy)]
#[reflect(Component, Clone, Default)]
#[require(Transform, Visibility, Node)]
pub struct ActiveScene;

#[derive(Component)]
struct ComponentFilter(SceneFilter);

#[derive(Bundle)]
struct EditableScene {
	scene: ActiveScene,
	components: ComponentFilter,
}

fn on_new_camera(
	event: On<Add, EditorCamera>,
	mut commands: Commands,
	target_scene_root: Option<Single<Entity, With<EditorSceneRoot>>>,
) {
	match target_scene_root {
		Some(ts) => {
			commands
				.entity(*ts)
				.insert(UiTargetCamera(event.event_target()));
		}
		None => {
			commands.spawn(EditorSceneRoot);
		}
	}
}

fn on_new_scene(
	event: On<Add, EditorSceneRoot>,
	mut commands: Commands,
	editor_camera: EditorInternalSingle<Entity, With<EditorCamera>>,
) {
	commands
		.entity(event.event_target())
		.insert(UiTargetCamera(*editor_camera));
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

fn mark_entities(
	mut commands: Commands,
	q_unowned_entities: Query<Entity, (Without<SimulationOwned>, Without<EditorOwned>)>,
	state: Res<State<EditorState>>,
) {
	match state.get() {
		EditorState::Editing => {
			for entity in &q_unowned_entities {
				commands.entity(entity).insert(EditorOwned);
			}
		}
		EditorState::SimulationPrep | EditorState::Simulating(_) => {
			for entity in &q_unowned_entities {
				commands.entity(entity).insert(SimulationOwned);
			}
		}
		_ => {}
	}
}

fn on_sim_prep(
	mut commands: Commands,
	q_user_scenes: Query<Entity, (With<SceneRoot>, Without<UserHidden>)>,
	mut next_state: ResMut<NextState<EditorState>>,
	mut selected_entities: ResMut<InspectorSelection>,
) {
	for entity in &q_user_scenes {
		commands
			.entity(entity)
			.clone_and_spawn_with_opt_out(|builder| {
				builder.add_observers(true);
				builder.linked_cloning(true);
			})
			.insert(SimulationOwned);

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
	q_simulated_entities: Query<Entity, With<SimulationOwned>>,
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

#[derive(new, Message)]
pub struct ShowSceneSettings(Entity);

impl Command for ShowSceneSettings {
	fn apply(self, world: &mut World) {
		world.write_message(self);
	}
}

const MODAL_FILE_DIALOG_ID: &str = "beditor-file-dialog";

#[derive(Deref, DerefMut)]
struct SceneFileDialog(FileDialog);

impl Default for SceneFileDialog {
	fn default() -> Self {
		Self(
			FileDialog::default()
				.as_modal(true)
				.anchor(egui::Align2::CENTER_CENTER, egui::Vec2::default())
				.add_save_extension("Scenes", "scn.ron")
				.default_save_extension("scn.ron")
				.id(MODAL_FILE_DIALOG_ID),
		)
	}
}

#[derive(Deref, DerefMut)]
struct SceneOptionsModal(widgets::MenuModal);

impl Default for SceneOptionsModal {
	fn default() -> Self {
		Self(widgets::MenuModal::new().order(egui::Order::Middle))
	}
}

#[derive(SystemParam)]
struct Params<'s, 'w> {
	type_name_cache: Res<'w, TypeNameDisplayCache>,
	type_name_list: Local<'s, widgets::SelectableList<widgets::MultiSelect<TypeNameDisplayInfo>>>,
	file_dialog: Local<'s, SceneFileDialog>,
}

impl From<SceneOptions> for egui::WidgetText {
	fn from(value: SceneOptions) -> Self {
		Self::Text(value.to_string().to_case(Case::Title))
	}
}

impl From<&SceneOptions> for egui::WidgetText {
	fn from(value: &SceneOptions) -> Self {
		Self::Text(value.to_string().to_case(Case::Title))
	}
}

fn show_scene_editing_modal(
	mut commands: Commands,
	mut messages: MessageReader<ShowSceneSettings>,
	mut contexts: EditorInternalSingle<&mut EguiContext, With<EditorEguiContext>>,
	mut modal: Local<SceneOptionsModal>,
	mut menu: Local<widgets::CategoryMenu<SceneOptions>>,
	mut params: Params,
	mut selected_scene: Local<Option<Entity>>,
) {
	modal.open |= !messages.is_empty();
	for msg in messages.read() {
		*selected_scene = Some(msg.0);
	}

	let ctx = contexts.get_mut();

	if let Some((file, selected_scene)) = params.file_dialog.take_picked().zip(*selected_scene) {
		commands.queue(SerializeScene(selected_scene, file));
	}

	let id = egui::Id::new("beditor-scene-modal");
	modal.show(ctx, id, |ui| {
		ui.heading("Scene Settings");

		ui.separator();

		menu.ui(ui, SceneOptions::VARIANTS, |ui, selected_category| {
			if let Some(category) = selected_category {
				category.ui(ui, params);
			} else {
				ui.label("Select a category");
			}
		});
	});
}

#[derive(Reflect, Default, Clone, Copy, Display, PartialEq, Eq, VariantArray)]
enum SceneOptions {
	#[default]
	SaveAndExport,
	SerializableTypes,
}

impl SceneOptions {
	fn ui(self, ui: &mut egui::Ui, params: Params<'_, '_>) {
		let Params {
			type_name_cache,
			mut type_name_list,
			mut file_dialog,
		} = params;

		file_dialog.update(ui.ctx());

		match self {
			Self::SaveAndExport => {
				if ui.button("Export").clicked() {
					file_dialog.save_file();
				}
			}
			Self::SerializableTypes => {
				ui.heading("Select Scene Components");

				ui.separator();

				type_name_list.ui(ui, type_name_cache.as_slice());
			}
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

struct SerializeScene(Entity, PathBuf);

impl Command for SerializeScene {
	fn apply(self, world: &mut World) {
		const ERR_MSG: &str = "Failed to serialize scene";
		let Self(entity, path) = self;
		world.resource_scope(|world, registry: Mut<RelationshipRegistry>| {
			let app_type_registry = world.resource::<AppTypeRegistry>().clone();
			let type_registry = app_type_registry.read();

			let all_entities = entity_with_relatives(entity, world, &registry);
			let scene = DynamicSceneBuilder::from_world(world)
				.allow_all()
				.extract_entities(all_entities.into_iter())
				.build();

			let scene_ser = SceneSerializer::new(&scene, &type_registry);

			let mut buf = String::new();

			let ser_result = ron::Serializer::new(
				&mut buf,
				Some(
					PrettyConfig::default()
						.struct_names(true)
						.escape_strings(true),
				),
			);

			let mut ron_ser = crate::match_else!(ser_result; else err => {
				world.trigger(Notification::error(ERR_MSG).with_context(err));
				return;
			});

			if let Err(err) = scene_ser.serialize(&mut ron_ser) {
				world.trigger(Notification::error(ERR_MSG).with_context(err));
				return;
			}

			if let Err(err) = std::fs::write(path, buf) {
				world.trigger(Notification::error(ERR_MSG).with_context(err));
				return;
			}

			world.trigger(Notification::success("Saved Scene"));
		});
	}
}

fn entity_with_relatives(
	entity: Entity,
	world: &mut World,
	registry: &RelationshipRegistry,
) -> Vec<Entity> {
	let mut entities_to_check = SmallVec::<[_; 24]>::from_iter(std::iter::once(entity));
	let mut found_entities = EntityHashSet::new();

	while let Some(entity) = entities_to_check.pop() {
		if found_entities.contains(&entity) {
			continue;
		}

		let relatives = registry
			.values()
			.fold(Vec::with_capacity(registry.len()), |mut list, extractor| {
				list.push((extractor)(entity, world));
				list
			})
			.into_iter()
			.flatten()
			.collect::<EntityHashSet>();

		found_entities.insert(entity);

		let unchecked = relatives.difference(&found_entities).copied();
		entities_to_check.extend(unchecked);
	}

	Vec::from_iter(found_entities)
}

#[derive(new)]
pub struct LoadScene(PathBuf);

impl Command for LoadScene {
	fn apply(self, world: &mut World) {
		world.resource_scope(|world, assets: Mut<AssetServer>| {
			let path = current_dir()
				.ok()
				.and_then(|cwd| {
					self
						.0
						.strip_prefix(cwd.join("assets"))
						.map(Path::to_path_buf)
						.ok()
				})
				.unwrap_or(self.0);

			let name = path
				.file_stem()
				.map(|f| format!("Scene Root ({})", f.display()))
				.unwrap_or_else(|| String::from("Scene Root"));
			let scene = assets.load(path);

			world
				.spawn((Name::new(name), DynamicSceneRoot(scene)))
				.observe(on_scene_ready);
		});
	}
}

fn on_scene_ready(
	event: On<SceneInstanceReady>,
	mut commands: Commands,
	q_scene_instances: Query<(Entity, &SceneInstance)>,
) {
	let id = event.instance_id;

	let Some(entity) = q_scene_instances
		.iter()
		.find_map(|(e, i)| (**i == id).then_some(e))
	else {
		return;
	};

	commands.entity(entity).insert(EditorSceneRoot);
}
