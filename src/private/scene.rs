use crate::{
	EditorState, SimulationState,
	inspector::ui::InspectorSelection,
	private::{
		EditorInternalSingle, EditorOwned, SimulationOwned, UserHidden,
		cam::EditorCamera,
		reflection::{CachedTypeInfo, TypeInfoCache},
		ui::{EditorEguiContext, EditorUiEguiContextPass},
	},
	util::entity::one_of,
};
use bevy::{
	ecs::{entity::EntityHashSet, entity_disabling::Disabled, system::SystemParam},
	platform::collections::HashSet,
	prelude::*,
	scene::{SceneInstance, SceneInstanceReady, serde::SceneSerializer},
	utils::TypeIdMap,
};
use bevy_egui::EguiContext;
use bevy_infinite_grid::InfiniteGrid;
use convert_case::{Case, Casing};
use derive_more::derive::Deref;
use derive_new::new;
use egui_autocomplete::AutoCompleteTextEdit;
use egui_file_dialog::FileDialog;
use notify::Notification;
use ron::ser::PrettyConfig;
use serde::Serialize;
use singleton::{SingletonBehavior, SingletonPlugin};
use smallvec::SmallVec;
use std::{
	any::TypeId,
	env::current_dir,
	ops::DerefMut,
	path::{Path, PathBuf},
};
use strum::{IntoEnumIterator, VariantArray};
use strum_macros::{Display, EnumIter, VariantArray};

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
#[require(
	Transform,
	Visibility,
	Node,
	ActiveScene,
	Name::new("New Scene"),
	ComponentFilter,
	ResourceFilter
)]
pub struct EditorSceneRoot;

/// Add this component to any entity to give it the qualifications of becoming a scene
/// Also make it the currently active scene
#[derive(Component, Reflect, Default, Clone, Copy)]
#[reflect(Component, Clone, Default)]
#[require(Transform, Visibility, Node)]
pub struct ActiveScene;

#[derive(Component, Default, Deref, DerefMut)]
struct ComponentFilter(SceneFilter);

#[derive(Component, Default, Deref, DerefMut)]
struct ResourceFilter(SceneFilter);

/// Mirrored from [`SceneFilter`]
#[derive(Default, EnumIter, Display, PartialEq, Eq, Clone, Copy)]
enum FilterMode {
	#[default]
	Allow,
	Deny,
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
struct Params<'w, 's> {
	app_type_registry: Res<'w, AppTypeRegistry>,
	file_dialog: Local<'s, SceneFileDialog>,
	component_params: ComponentParams<'w, 's>,
	resource_params: ResourceParams<'w, 's>,
}

#[derive(SystemParam)]
struct ComponentParams<'w, 's> {
	q_filters: Query<'w, 's, &'static mut ComponentFilter>,
	cache: Local<'s, Vec<CachedTypeInfo>>,
	list: Local<'s, widgets::SelectableList<widgets::MultiSelect<CachedTypeInfo>>>,
	search_text: Local<'s, String>,
	mode: Local<'s, FilterMode>,
}

#[derive(SystemParam)]
struct ResourceParams<'w, 's> {
	q_filters: Query<'w, 's, &'static mut ResourceFilter>,
	cache: Local<'s, Vec<CachedTypeInfo>>,
	list: Local<'s, widgets::SelectableList<widgets::MultiSelect<CachedTypeInfo>>>,
	search_text: Local<'s, String>,
	mode: Local<'s, FilterMode>,
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
	app_type_registry: Res<AppTypeRegistry>,
	type_name_cache: Res<TypeInfoCache>,
	mut modal: Local<SceneOptionsModal>,
	mut menu: Local<widgets::CategoryMenu<SceneOptions>>,
	mut selected_scene: Local<Option<Entity>>,
	mut params: Params,
) {
	modal.open |= !messages.is_empty();
	for msg in messages.read() {
		*selected_scene = Some(msg.0);
	}

	let ctx = contexts.get_mut();

	if let Some((file, selected_scene)) = params.file_dialog.take_picked().zip(*selected_scene) {
		commands.queue(SerializeScene(selected_scene, file));
	}

	if type_name_cache.is_changed() {
		let type_registry = app_type_registry.read();
		*params.component_params.cache = type_name_cache
			.iter()
			.filter(|c| {
				type_registry
					.get_type_data::<ReflectComponent>(c.type_info.type_id())
					.is_some()
			})
			.cloned()
			.collect();

		*params.resource_params.cache = type_name_cache
			.iter()
			.filter(|c| {
				type_registry
					.get_type_data::<ReflectResource>(c.type_info.type_id())
					.is_some()
			})
			.cloned()
			.collect();
	}

	let id = egui::Id::new("beditor-scene-modal");
	modal.show(ctx, id, |ui| {
		ui.heading("Scene Settings");

		ui.separator();

		menu.ui(ui, SceneOptions::VARIANTS, |ui, selected_category| {
			if let Some(category) = selected_category {
				category.ui(ui, *selected_scene, params);
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
	SceneComponents,
	SceneResources,
}

impl SceneOptions {
	fn ui(self, ui: &mut egui::Ui, selected_scene: Option<Entity>, params: Params<'_, '_>) {
		let Params {
			app_type_registry,
			mut file_dialog,
			component_params,
			resource_params,
		} = params;

		file_dialog.update(ui.ctx());

		match self {
			Self::SaveAndExport => {
				ui.add_enabled_ui(false, |ui| {
					if ui.button("Save").clicked() {
						// TODO
					}
				});

				if ui.button("Export").clicked() {
					file_dialog.save_file();
				}

				if let Some(entity) = selected_scene {
					ui.separator();

					let half = ui.available_width() / 2.0;

					let type_registry = app_type_registry.read();

					widgets::VerticalSplit::new(half).show(
						ui,
						|ui| {
							let Ok(filter) = component_params.q_filters.get(entity) else {
								ui.heading("All Components Allowed");
								return;
							};

							let type_list = match &**filter {
								SceneFilter::Unset => {
									ui.heading("All Components Allowed");
									return;
								}
								SceneFilter::Allowlist(hash_set) => {
									ui.heading("Allowed Components");
									hash_set
								}
								SceneFilter::Denylist(hash_set) => {
									ui.heading("Denied Components");
									hash_set
								}
							};

							let items = type_list.iter().cloned().collect::<Vec<_>>();
							widgets::vertical_list(ui, items, |ui, _, slice| {
								for tr in slice
									.iter()
									.filter_map(|&type_id| type_registry.get(type_id))
								{
									ui.label(tr.type_info().type_path());
								}
							});
						},
						|ui| {
							let Ok(filter) = resource_params.q_filters.get(entity) else {
								ui.heading("All Resources Allowed");
								return;
							};

							let type_list = match &**filter {
								SceneFilter::Unset => {
									ui.heading("All Resources Allowed");
									return;
								}
								SceneFilter::Allowlist(hash_set) => {
									ui.heading("Allowed Resources");
									hash_set
								}
								SceneFilter::Denylist(hash_set) => {
									ui.heading("Denied Resources");
									hash_set
								}
							};

							let items = type_list.iter().cloned().collect::<Vec<_>>();
							widgets::vertical_list(ui, items, |ui, _, slice| {
								for tr in slice
									.iter()
									.filter_map(|&type_id| type_registry.get(type_id))
								{
									ui.label(tr.type_info().type_path());
								}
							});
						},
					);
				}
			}
			Self::SceneComponents => {
				let ComponentParams {
					mut q_filters,
					cache,
					mut list,
					mut search_text,
					mut mode,
				} = component_params;
				type_filter_ui(
					ui,
					"Select Scene Components",
					&cache,
					&mut search_text,
					&mut list,
					q_filters.iter_mut(),
					&mut mode,
				);
			}
			Self::SceneResources => {
				let ResourceParams {
					mut q_filters,
					cache,
					mut list,
					mut search_text,
					mut mode,
				} = resource_params;
				type_filter_ui(
					ui,
					"Select Scene Resources",
					&cache,
					&mut search_text,
					&mut list,
					q_filters.iter_mut(),
					&mut mode,
				);
			}
		}
	}
}

fn type_filter_ui<'w, I, T>(
	ui: &mut egui::Ui,
	heading: &str,
	list: &[CachedTypeInfo],
	search_text: &mut String,
	selectable_list: &mut widgets::SelectableList<widgets::MultiSelect<CachedTypeInfo>>,
	mut filters: I,
	mode: &mut FilterMode,
) where
	I: Iterator<Item = Mut<'w, T>>,
	T: 'w + DerefMut<Target = SceneFilter>,
{
	let mut selection_changed = false;
	let mut filter_mod: Option<fn(filter: &mut SceneFilter)> = None;

	ui.heading(heading);

	ui.separator();

	ui.horizontal(|ui| {
		let response = ui.add(AutoCompleteTextEdit::new(search_text, list));

		let keyboard_submit = response.lost_focus() && !response.clicked_elsewhere();

		if (ui.button("Toggle Searched").clicked() || keyboard_submit)
			&& !search_text.is_empty()
			&& let Some(cached_info) = list.iter().find(|c| c.as_ref() == search_text.as_str())
		{
			selection_changed |= true;
			selectable_list.select(cached_info.clone());
		}

		if ui.button("Select All").clicked() {
			selection_changed |= true;
			for item in list {
				selectable_list.select(item.clone());
			}
		}

		if ui.button("Reset All").clicked() {
			*selectable_list = default();
			*mode = FilterMode::default();
			filter_mod = Some(|filter| {
				*filter = SceneFilter::Unset;
			});
		}

		ui.horizontal(|ui| {
			if ui.radio_value(mode, FilterMode::Allow, "Allow").clicked() {
				*mode = FilterMode::Allow;
				filter_mod = Some(|filter| {
					*filter = SceneFilter::Allowlist(HashSet::from_iter(filter.iter().cloned()));
				});
			}

			if ui.radio_value(mode, FilterMode::Deny, "Deny").clicked() {
				*mode = FilterMode::Deny;
				filter_mod = Some(|filter| {
					*filter = SceneFilter::Denylist(HashSet::from_iter(filter.iter().cloned()));
				});
			}
		});
	});

	selection_changed |= selectable_list
		.ui(ui, list)
		.map(|r| r.response.clicked())
		.unwrap_or(false);

	if selection_changed {
		let selected = selectable_list.selected();
		let selected = selected.iter().map(|c| c.type_info.type_id());
		let filter_mod = filter_mod.unwrap_or(|_| {});

		for mut filter in &mut filters {
			(filter_mod)(&mut filter);

			match &mut **filter {
				SceneFilter::Unset => {
					// do nothing
				}
				SceneFilter::Allowlist(hash_set) | SceneFilter::Denylist(hash_set) => {
					*hash_set = HashSet::from_iter(selected.clone());
				}
			}
		}
	} else {
		let filter_mod = filter_mod.unwrap_or(|_| {});

		for mut filter in &mut filters {
			(filter_mod)(&mut filter);
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
			let scene = scene_builder_for(entity, world)
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

fn scene_builder_for<'w>(entity: Entity, world: &'w World) -> DynamicSceneBuilder<'w> {
	let mut scene_builder = DynamicSceneBuilder::from_world(world).allow_all();

	if let Some(component_filter) = world.get::<ComponentFilter>(entity)
		&& **component_filter != SceneFilter::Unset
	{
		scene_builder = scene_builder.with_component_filter(SceneFilter::clone(component_filter))
	}

	if let Some(resource_filter) = world.get::<ResourceFilter>(entity)
		&& **resource_filter != SceneFilter::Unset
	{
		scene_builder = scene_builder.with_resource_filter(SceneFilter::clone(resource_filter))
	}

	scene_builder
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
