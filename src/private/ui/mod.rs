pub mod events;
pub mod misc;

use crate::{
	DataTable, EditorState, EditorUiWorld, PersistentData, ProjectSettings,
	inspector::{
		add_single,
		ui::hierarchy::{Selected, SelectedEntities, SelectedEntitiesChangedEvent},
	},
	panels::{
		assets, components, diagnostics, editor_view, hierarchy, inspector, menu_bar, prefabs,
		resources,
	},
	private::{
		EditorInternal, EditorInternalFilter, EditorInternalQuery, EditorInternalSingle, EditorOwned,
		cam::{EDITOR_VIEW_RENDER_LAYER, EditorCamera},
	},
	settings::{CurrentLayoutSetting, EditorEguiSettings, EditorUiScale, SaveLayoutOnExitSetting},
	util::{entity::insert_bundle_from_world, storage::GlobalEditorSettings},
};
use bevy::{
	asset::UntypedAssetId,
	camera::visibility::RenderLayers,
	ecs::{
		schedule::ScheduleLabel,
		system::{SystemState, entity_command},
	},
	picking::pointer::PointerId,
	platform::collections::HashMap,
	prelude::*,
};
use bevy_egui::{
	EguiContext, EguiContextSettings, EguiGlobalSettings, EguiMultipassSchedule, EguiPlugin,
};
use bevy_mod_outline::{OutlineMode, OutlineRenderLayers, OutlineVolume};
use derive_new::new;
use egui_dock::{DockArea, DockState, NodeIndex, SurfaceIndex};
use events::AppendUiMessage;
use events::RemoveUiEvent;
use events::{OpenSingleUiMessage, OpenUiMessage, ShowUiMessage};
use itertools::Itertools;
use misc::{DockExtensions, EditorUiExtensions, UiResourceState};
use misc::{MissingUi, UiState};
use persistent_id::PersistentId;
use serde::{Deserialize, Serialize};
use std::{any::TypeId, cell::RefCell, collections::BTreeSet};
use transform_gizmo_bevy::GizmoTarget;

pub(crate) struct EditorUiPlugin;

impl Plugin for EditorUiPlugin {
	fn build(&self, app: &mut App) {
		debug!("Building UI Plugin");

		let egui_settings = EguiGlobalSettings {
			auto_create_primary_context: false,
			..default()
		};

		app
			.insert_resource(egui_settings)
			.init_resource::<HighlightOptions>()
			.init_resource::<UiManager>()
			.init_resource::<InspectorSelection>()
			.init_resource::<LayoutManager>()
			.init_state::<KeyboardFocus>()
			.add_message::<AppendUiMessage>()
			.add_message::<OpenUiMessage>()
			.add_message::<OpenSingleUiMessage>()
			.add_message::<ShowUiMessage>()
			.add_plugins(EguiPlugin::default())
			.add_observer(on_new_ctx)
			.add_observer(RemoveUiEvent::on_event)
			.add_observer(handle_click_events)
			.add_observer(handle_selected)
			.add_observer(handle_deselected)
			.add_systems(Startup, UiManager::init)
			.add_systems(
				OnEnter(EditorState::Exiting),
				(save_context_options, save_scale_factor, save_layouts),
			)
			.add_systems(
				FixedUpdate,
				(
					AppendUiMessage::handle,
					OpenUiMessage::handle,
					OpenSingleUiMessage::handle,
					ShowUiMessage::handle,
				),
			)
			.add_systems(
				EditorUiEguiContextPass,
				(
					KeyboardFocus::set_state,
					(reset_ui_state, editor_ui).chain(),
				),
			);

		let type_registry = app.world().resource::<AppTypeRegistry>();
		let mut type_registry = type_registry.write();

		add_single::<UiState>(&mut type_registry);
	}
}

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EditorUiEguiContextPass;

#[derive(Component, Default, Clone)]
#[require(EguiContext, EguiMultipassSchedule::new(EditorUiEguiContextPass))]
pub struct EditorEguiContext;

#[derive(Resource)]
pub struct UiManager {
	state: DockState<TabState>,
	vtables: HashMap<PersistentId, &'static VTable>,
	id: egui::Id,
}

impl Default for UiManager {
	fn default() -> Self {
		Self {
			state: DockState::new(Vec::new()),
			vtables: default(),
			id: egui::Id::new(TypeId::of::<Self>()),
		}
	}
}

impl UiManager {
	pub fn register<T: EditorUiWorld>(&mut self) {
		let key = PersistentId(T::ID);
		if self.vtables.contains_key(&key) {
			panic!("Already registered Ui {}", std::any::type_name::<T>());
		}

		self.vtables.insert(key, &T::VTABLE);
	}

	pub fn add_detached(&mut self, tabs: Vec<TabState>) -> SurfaceIndex {
		self.state.add_window(tabs)
	}

	pub fn add_tab(&mut self, surface: SurfaceIndex, node: NodeIndex, tab: TabState) -> bool {
		let Some(surface) = self.state.get_surface_mut(surface) else {
			return false;
		};

		let Some(nodes) = surface.node_tree_mut() else {
			return false;
		};

		let node = &mut nodes[node];

		node.append_tab(tab);

		true
	}

	pub fn add_tab_to_focused(&mut self, tab: TabState) -> bool {
		let Some((surface, node)) = self.state.focused_leaf() else {
			return false;
		};

		self.add_tab(surface, node, tab)
	}

	fn init(world: &mut World) -> Result {
		let state = SystemState::<menu_bar::Params<'_, '_>>::new(world);
		world.insert_resource(UiResourceState::new(state));
		world.spawn((Name::new("Editor Ui Panels"), UiPanels));
		world.resource_scope(|world, mut this: Mut<Self>| this.restore_or_init(world))
	}

	fn restore_or_init(&mut self, world: &mut World) -> Result {
		let mut sys_state = SystemState::<ProjectSettings>::new(world);
		let mut project_settings = sys_state.get_mut(world);

		let current_layout_name = project_settings.get(CurrentLayoutSetting).ok();

		let layouts = BTreeSet::from_iter(project_settings.list_keys::<LayoutsTable>()?);

		let mut dock = match current_layout_name {
			Some(name) => {
				if let Ok(layout) = project_settings.get(SavedLayout(name)) {
					DockState::restore(layout, &self.vtables, world)
				} else {
					self.default_dock_state(world)
				}
			}
			None => self.default_dock_state(world),
		};

		// resets any surfaces that have an active
		// tab that does not not reopen on startup
		for (_, leaf) in dock.iter_leaves_mut() {
			if leaf.active_focused().is_none() {
				leaf.set_active_tab(0);
			}
		}

		self.state = dock;

		world.insert_resource(LayoutManager::new(layouts));

		Ok(())
	}

	fn ui(&mut self, world: &mut World) {
		let Ok(ctx) = world
			.query_filtered::<&mut EguiContext, EditorInternalFilter<With<EditorEguiContext>>>()
			.single_mut(world)
			.map(|mut ctx| ctx.get_mut().clone())
		else {
			error!("No egui context to render to");
			return;
		};

		let style = ctx.style();

		let dock_style = egui_dock::Style::from_egui(&style);

		egui::CentralPanel::default()
			.frame(
				egui::Frame::central_panel(&style)
					.inner_margin(0)
					.fill(dock_style.tab.tab_body.bg_fill),
			)
			.show(&ctx, |ui| {
				// menu bar
				world.resource_scope(|world, mut state: Mut<UiResourceState<menu_bar::Params>>| {
					let params = state.get_mut(world);
					menu_bar::render(ui, params);
					state.apply(world);
				});

				let mut tab_viewer = TabViewer {
					vtables: &mut self.vtables,
					world: RefCell::new(world),
				};

				DockArea::new(&mut self.state)
					.id(self.id)
					.style(dock_style)
					.show_add_buttons(true)
					.show_add_popup(true)
					.show_inside(ui, &mut tab_viewer);
			});
	}

	fn save_state(
		&self,
		q_uuids: &EditorInternalQuery<&PersistentId, Without<MissingUi>>,
		q_missing: &EditorInternalQuery<&MissingUi>,
	) -> DockState<LayoutInfo> {
		self.state.decouple(self, q_uuids, q_missing)
	}

	pub(crate) fn switch_state(&mut self, new_state: DockState<TabState>, world: &mut World) {
		for (_, tab) in self.state.iter_all_tabs() {
			(tab.vtable.despawn)(tab.entity, world);
		}
		self.state = new_state;
	}

	pub(crate) fn default_dock_state(&self, world: &mut World) -> DockState<TabState> {
		let mut state = DockState::new(vec![TabState::new::<editor_view::EditorViewUi>(world)]);

		let tree = state.main_surface_mut();

		let root = NodeIndex::root();

		let tabs = vec![
			TabState::new::<hierarchy::HierarchyUi>(world),
			TabState::new::<diagnostics::DiagnosticsUi>(world),
		];
		let [central_panel, _left_panel] = tree.split_left(root, 1.0 / 6.0, tabs);

		let tabs = vec![TabState::new::<inspector::InspectorUi>(world)];
		let [central_panel, _right_panel] = tree.split_right(central_panel, 4.0 / 5.0, tabs);

		let tabs = vec![
			TabState::new::<prefabs::PrefabsUi>(world),
			TabState::new::<components::ComponentsUi>(world),
			TabState::new::<resources::ResourcesUi>(world),
			TabState::new::<assets::AssetsUi>(world),
		];
		tree.split_below(central_panel, 0.7, tabs);

		state
	}

	pub(crate) fn state(&self) -> &DockState<TabState> {
		&self.state
	}
}

#[derive(Clone)]
pub struct TabState {
	entity: Entity,
	vtable: &'static VTable,
}

impl TabState {
	pub(crate) fn new<T: EditorUiWorld>(world: &mut World) -> Self {
		Self {
			entity: (T::VTABLE.spawn)(world),
			vtable: &T::VTABLE,
		}
	}

	pub fn entity(&self) -> Entity {
		self.entity
	}
}

#[derive(new, Resource, Default, Deref, DerefMut)]
pub struct LayoutManager(BTreeSet<String>);

pub struct LayoutsTable;

impl DataTable for LayoutsTable {
	type DataType = Vec<u8>;
	const TABLE: &str = "layouts";
	const KEY_COLUMN: &str = "name";
	const VALUE_COLUMN: &str = "data";
}

#[derive(new)]
pub struct SavedLayout(String);

impl PersistentData for SavedLayout {
	type Table = LayoutsTable;
	type Type = DockState<LayoutInfo>;

	fn to_key(self) -> String {
		self.0
	}

	fn serialize(value: Self::Type) -> Result<Vec<u8>> {
		let bytes = postcard::to_stdvec(&value)?;
		Ok(bytes)
	}

	fn deserialize(input: Vec<u8>) -> Result<Self::Type> {
		let value = postcard::from_bytes(&input)?;
		Ok(value)
	}
}

#[derive(Clone, Serialize, Deserialize, new)]
pub struct LayoutInfo {
	id: PersistentId,
	name: String,
}

impl LayoutInfo {
	pub fn id(&self) -> PersistentId {
		self.id
	}

	pub fn name(&self) -> &str {
		&self.name
	}
}

pub struct LoadLayout(pub DockState<LayoutInfo>);

impl Command for LoadLayout {
	fn apply(self, world: &mut World) {
		world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
			let new_state = DockState::restore(self.0, &ui_manager.vtables, world);
			ui_manager.switch_state(new_state, world);
		})
	}
}

#[derive(Clone)]
pub(crate) struct VTable {
	name: &'static str,
	closeable: bool,
	hidden: bool,
	can_clear: bool,
	scroll_bars: [bool; 2],
	unique: bool,
	popout: bool,
	reopen_on_startup: bool,
	spawn: fn(&mut World) -> Entity,
	despawn: fn(Entity, &mut World),
	title: fn(Entity, &mut World) -> egui::WidgetText,
	render: fn(Entity, &mut egui::Ui, &mut World),
	context_menu: fn(Entity, &mut egui::Ui, &mut World, SurfaceIndex, NodeIndex),
	handle_tab_response: fn(Entity, &mut World, &egui::Response),
	on_panel_changed: fn(Entity, &mut World),
	count: fn(&mut World) -> usize,
}

impl VTable {
	const fn new<T>() -> Self
	where
		T: EditorUiWorld,
	{
		Self {
			name: T::NAME,
			closeable: T::CLOSEABLE,
			hidden: T::HIDDEN,
			can_clear: T::CAN_CLEAR,
			scroll_bars: T::SCROLL_BARS,
			unique: T::UNIQUE,
			popout: T::POPOUT,
			reopen_on_startup: T::REOPEN_ON_STARTUP,
			spawn: Self::spawn::<T>,
			despawn: Self::despawn::<T>,
			title: T::title,
			render: T::ui,
			context_menu: T::context_menu,
			handle_tab_response: T::handle_tab_response,
			on_panel_changed: T::on_panel_changed,
			count: Self::count::<T>,
		}
	}

	fn spawn<T: EditorUiWorld>(world: &mut World) -> Entity {
		info!("Spawning UI component {}", T::NAME);
		let entity = world
			.spawn((
				Name::new(T::NAME),
				EditorOwned,
				PersistentId(T::ID),
				UiState::default(),
			))
			.id();

		let ui_scene = world
			.query_filtered::<Entity, EditorInternalFilter<With<UiPanels>>>()
			.iter(world)
			.next()
			.unwrap();
		world.entity_mut(ui_scene).add_child(entity);

		let instance = T::spawn(entity, world);
		world.entity_mut(entity).insert(instance).id()
	}

	fn despawn<T: EditorUiWorld>(entity: Entity, world: &mut World) {
		info!("Despawning UI component {}", T::NAME);
		<T as EditorUiWorld>::on_despawn(entity, world);
		world.trigger(RemoveUiEvent::new(entity));
	}

	fn count<T: EditorUiWorld>(world: &mut World) -> usize {
		let mut q_uis = world.query_filtered::<&T::MarkerComponent, EditorInternalFilter>();
		q_uis.iter(world).count()
	}
}

struct TabViewer<'a> {
	/// RefCell so that functions with &self can access a mut World
	world: RefCell<&'a mut World>,
	vtables: &'a mut HashMap<PersistentId, &'static VTable>,
}

impl TabViewer<'_> {
	fn ui_state_mut(&self, entity: Entity, f: impl FnOnce(&mut UiState)) {
		let mut world = self.world.borrow_mut();
		let mut q_ids = world.query_filtered::<&mut UiState, EditorInternalFilter>();
		let ui_info = q_ids.get_mut(&mut world, entity).ok();
		if let Some(mut ui_info) = ui_info {
			f(&mut ui_info);
		}
	}
}

impl egui_dock::TabViewer for TabViewer<'_> {
	type Tab = TabState;

	fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
		(tab.vtable.title)(tab.entity, &mut self.world.borrow_mut())
	}

	#[profiling::function]
	fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
		(tab.vtable.render)(tab.entity, ui, &mut self.world.borrow_mut());
		if ui.ui_contains_pointer() {
			self.ui_state_mut(tab.entity, |state| {
				state.mark_hovered();
			});
		}
	}

	#[profiling::function]
	fn add_popup(&mut self, ui: &mut egui::Ui, surface: SurfaceIndex, node: NodeIndex) {
		ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

		for vtable in self
			.vtables
			.values()
			.filter(|v| !v.hidden)
			.sorted_by(|v1, v2| v1.name.cmp(v2.name))
		{
			if vtable.unique {
				let mut world = self.world.borrow_mut();
				let count = (vtable.count)(&mut world);

				let mut exists = count > 0;

				ui.add_enabled_ui(!exists, |ui| {
					if ui.checkbox(&mut exists, vtable.name).clicked() {
						let entity = (vtable.spawn)(&mut world);
						world.write_message(AppendUiMessage::new(
							surface,
							node,
							TabState { entity, vtable },
						));
					}
				});
			} else if ui.button(vtable.name).clicked() {
				let mut world = self.world.borrow_mut();
				let entity = (vtable.spawn)(&mut world);
				world.write_message(AppendUiMessage::new(
					surface,
					node,
					TabState { entity, vtable },
				));
			}
		}
	}

	fn context_menu(
		&mut self,
		ui: &mut egui::Ui,
		tab: &mut Self::Tab,
		surface: SurfaceIndex,
		node: NodeIndex,
	) {
		(tab.vtable.context_menu)(tab.entity, ui, &mut self.world.borrow_mut(), surface, node);
	}

	fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &egui::Response) {
		(tab.vtable.handle_tab_response)(tab.entity, &mut self.world.borrow_mut(), response)
	}

	fn closeable(&mut self, tab: &mut Self::Tab) -> bool {
		tab.vtable.closeable
	}

	fn on_close(&mut self, tab: &mut Self::Tab) -> egui_dock::tab_viewer::OnCloseResponse {
		(tab.vtable.despawn)(tab.entity, &mut self.world.borrow_mut());
		egui_dock::tab_viewer::OnCloseResponse::Close
	}

	fn clear_background(&self, tab: &Self::Tab) -> bool {
		tab.vtable.can_clear
	}

	fn allowed_in_windows(&self, tab: &mut Self::Tab) -> bool {
		tab.vtable.popout
	}

	fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
		egui::Id::new(tab.entity)
	}

	fn scroll_bars(&self, tab: &Self::Tab) -> [bool; 2] {
		tab.vtable.scroll_bars
	}

	fn on_rect_changed(&mut self, tab: &mut Self::Tab) {
		(tab.vtable.on_panel_changed)(tab.entity, &mut self.world.borrow_mut());
	}
}

#[derive(Resource)]
pub enum InspectorSelection {
	Entities(SelectedEntities),
	Resource(TypeId, String),
	Asset(TypeId, String, UntypedAssetId),
}

impl Default for InspectorSelection {
	fn default() -> Self {
		Self::Entities(default())
	}
}

impl InspectorSelection {
	pub fn clear(&mut self) -> Option<SelectedEntitiesChangedEvent> {
		match self {
			InspectorSelection::Entities(selected_entities) => Some(selected_entities.scoped_clear()),
			InspectorSelection::Resource(_, _) => {
				*self = default();
				None
			}
			InspectorSelection::Asset(_, _, _) => {
				*self = default();
				None
			}
		}
	}

	pub fn add_selected(&mut self, entity: Entity, add: bool) -> SelectedEntitiesChangedEvent {
		if let InspectorSelection::Entities(selected_entities) = self {
			selected_entities.select_maybe_add(entity, add)
		} else {
			let mut selected_entities = SelectedEntities::default();
			let event = selected_entities.select_replace(entity);
			*self = Self::Entities(selected_entities);
			event
		}
	}
}

#[derive(Resource)]
struct HighlightOptions {
	thickness: f32,
	color: Color,
}

impl Default for HighlightOptions {
	fn default() -> Self {
		Self {
			thickness: 16.0,
			color: Color::linear_rgb(0.8, 0.0, 0.7),
		}
	}
}

#[derive(Bundle)]
struct Highlight {
	volume: OutlineVolume,
	layers: OutlineRenderLayers,
	mode: OutlineMode,
}

impl FromWorld for Highlight {
	fn from_world(world: &mut World) -> Self {
		let opts = world.resource::<HighlightOptions>();
		Self {
			volume: OutlineVolume {
				visible: true,
				width: opts.thickness,
				colour: opts.color,
			},
			layers: OutlineRenderLayers(RenderLayers::layer(EDITOR_VIEW_RENDER_LAYER)),
			mode: OutlineMode::FloodFlat,
		}
	}
}

fn handle_click_events(
	mut event: On<Pointer<Click>>,
	mut commands: Commands,
	editor_camera_pointer_id: EditorInternalSingle<&PointerId, With<EditorCamera>>,
	mut selection: ResMut<InspectorSelection>,
	keyboard: Res<ButtonInput<KeyCode>>,
) {
	event.propagate(false);

	if event.pointer_id != **editor_camera_pointer_id || event.button != PointerButton::Primary {
		return;
	}

	let target = event.event_target();

	let maybe_add = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);

	let event = selection.add_selected(target, maybe_add);
	commands.trigger(event);
}

fn handle_selected(
	event: On<Add, Selected>,
	mut commands: Commands,
	q_3d_meshes: Query<(), With<Mesh3d>>,
) {
	if let Ok(mut entity) = commands.get_entity(event.event_target()) {
		entity.insert(GizmoTarget::default());
		if q_3d_meshes.contains(entity.id()) {
			entity.queue(insert_bundle_from_world::<Highlight>());
		}
	}
}

fn handle_deselected(event: On<Remove, Selected>, mut commands: Commands) {
	if let Ok(mut entity) = commands.get_entity(event.event_target()) {
		entity.queue_silenced(entity_command::remove::<(GizmoTarget, Highlight)>());
	}
}

/// Component that stores all ui components as children for organization
#[derive(Component)]
#[require(EditorInternal)]
pub struct UiPanels;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default)]
pub enum KeyboardFocus {
	#[default]
	Unfocused,
	Focused(egui::Id),
}

impl KeyboardFocus {
	fn set_state(
		mut q_contexts: Query<&mut EguiContext>,
		mut keyboard_focus: ResMut<NextState<Self>>,
	) {
		let focus = q_contexts
			.iter_mut()
			.find_map(|mut ctx| ctx.get_mut().memory(|memory| memory.focused()));

		keyboard_focus.set(focus.map(Self::Focused).unwrap_or(Self::Unfocused))
	}
}

pub fn on_new_ctx(
	event: On<Add, EditorEguiContext>,
	mut q_ctx: EditorInternalQuery<(&mut EguiContext, &mut bevy_egui::EguiContextSettings)>,
	mut settings: GlobalEditorSettings,
) {
	let Ok((mut ctx, mut ctx_settings)) = q_ctx.get_mut(event.event_target()) else {
		return;
	};

	let ctx = ctx.get_mut();

	let mut fonts = egui::FontDefinitions::default();
	egui_phosphor_icons::add_fonts(&mut fonts);
	ctx.set_fonts(fonts.clone());

	if let Ok(options) = settings.get(EditorEguiSettings) {
		ctx.options_mut(|opts| {
			*opts = options;
		});
	}

	ctx_settings.scale_factor = settings
		.get(EditorUiScale)
		.unwrap_or(ctx_settings.scale_factor);
}

pub fn reset_ui_state(mut q_ui_state: EditorInternalQuery<&mut UiState>) {
	q_ui_state.par_iter_mut().for_each(|mut state| {
		state.clear();
	});
}

pub fn editor_ui(world: &mut World) {
	world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
		ui_manager.ui(world);
	});
}

fn save_context_options(
	mut context: EditorInternalSingle<&mut EguiContext, With<EditorEguiContext>>,
	mut settings: GlobalEditorSettings,
) {
	let ctx = context.get_mut();
	let opts = ctx.options(|opts| opts.clone());
	settings.set(EditorEguiSettings, opts).ok();
}

fn save_scale_factor(
	ctx_settings: Single<&EguiContextSettings, With<EditorEguiContext>>,
	mut settings: GlobalEditorSettings,
) {
	settings.set(EditorUiScale, ctx_settings.scale_factor).ok();
}

pub fn save_layouts(
	ui_manager: Res<UiManager>,
	q_uuids: EditorInternalQuery<&PersistentId, Without<MissingUi>>,
	q_missing: EditorInternalQuery<&MissingUi>,
	mut settings: ProjectSettings,
) -> Result {
	let save_on_exit = settings.get(SaveLayoutOnExitSetting).unwrap_or(true);
	let current_layout = if save_on_exit {
		let name = match settings.get(CurrentLayoutSetting).ok() {
			Some(opt) => opt,
			None => {
				let default_layout = String::from("default");
				settings.set(CurrentLayoutSetting, default_layout.clone())?;
				default_layout
			}
		};

		Some(name)
	} else {
		None
	};

	if let Some(name) = current_layout {
		let new_state = ui_manager.save_state(&q_uuids, &q_missing);
		settings.set(SavedLayout(name), new_state)?;
	}

	Ok(())
}
