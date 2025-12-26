pub mod builtin;
pub mod events;
pub mod misc;
pub mod notifications;
mod systems;
pub mod widgets;

use crate::{
	DataTable, EditorState, PersistentData, ProjectSettings, RuntimeSettings,
	inspector::ui::hierarchy::{Selected, SelectedEntities, SelectedEntitiesChangedEvent},
	settings::CurrentLayoutSetting,
	ui::{
		builtin::{
			editor_view::EditorViewUi,
			settings::{EditorSettingsUi, ProjectSettingsUi},
		},
		events::{OpenSingleUiMessage, OpenUiMessage},
	},
	util::make_singleton,
	view::cam::EditorCamera,
};
use bevy::{
	asset::UntypedAssetId,
	camera::visibility::{Layer, RenderLayers},
	ecs::{
		component::Mutable,
		system::{SystemParam, SystemState, entity_command},
	},
	picking::pointer::PointerId,
	platform::collections::HashMap,
	prelude::*,
};
use bevy_egui::{EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext};
use bevy_mesh_outline::MeshOutline;
use builtin::{
	assets::AssetsUi, components::ComponentsUi, diagnostics::DiagnosticsUi, hierarchy::HierarchyUi,
	inspector::InspectorUi, logs::LogUi, menu_bar, prefabs::PrefabsUi, resources::ResourcesUi,
	type_editor::TypeEditorUi,
};
use derive_new::new;
use egui_dock::{DockArea, DockState, NodeIndex, SurfaceIndex};
use events::AppendUiMessage;
use events::RemoveUiEvent;
use itertools::Itertools;
use misc::{DockExtensions, EditorUiExtensions, UiResourceState};
use misc::{MissingUi, UiExtensions, UiState};
use notifications::NotificationPlugin;
use persistent_id::PersistentId;
use serde::{Deserialize, Serialize};
use std::{any::TypeId, cell::RefCell, collections::BTreeSet};
use transform_gizmo_bevy::GizmoTarget;
use uuid::Uuid;

pub const EDITOR_UI_LAYER: Layer = 31;

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct EditorUiSystems;

#[derive(Default, Component, Reflect)]
#[require(
  PrimaryEguiContext = PrimaryEguiContext,
  Camera = Camera {
    order: isize::MAX,
    ..default()
  },
  Camera2d,
  RenderLayers = RenderLayers::layer(EDITOR_UI_LAYER))]
pub struct EditorUiCamera;

#[derive(Component)]
pub struct EditorUiHitCaptureNode;

pub(crate) struct UiPlugin;

impl Plugin for UiPlugin {
	fn build(&self, app: &mut App) {
		debug!("Building UI Plugin");

		let egui_settings = EguiGlobalSettings {
			auto_create_primary_context: false,
			..default()
		};

		app
			.insert_resource(egui_settings)
			.init_resource::<InspectorSelection>()
			.init_resource::<LayoutManager>()
			.init_state::<KeyboardFocus>()
			.add_message::<AppendUiMessage>()
			.add_message::<OpenUiMessage>()
			.add_message::<OpenSingleUiMessage>()
			.configure_sets(EguiPrimaryContextPass, EditorUiSystems)
			.add_plugins((EguiPlugin::default(), NotificationPlugin))
			.add_observer(systems::on_new_ctx)
			.add_observer(RemoveUiEvent::on_event)
			.add_observer(handle_click_events)
			.add_observer(handle_selected)
			.add_observer(handle_deselected)
			.add_systems(Startup, (systems::startup, UiManager::init))
			.add_systems(OnEnter(EditorState::Exiting), systems::on_app_exit)
			.add_systems(
				FixedUpdate,
				(
					AppendUiMessage::handle,
					OpenUiMessage::handle,
					OpenSingleUiMessage::handle,
				),
			)
			.add_systems(
				EguiPrimaryContextPass,
				(
					KeyboardFocus::set_state,
					(systems::reset_ui_state, systems::render)
						.chain()
						.run_if(should_render_ui),
				)
					.in_set(EditorUiSystems),
			);
	}
}

pub trait EditorUiBundle: Bundle + Send + Sync + Sized {
	type PrimaryComponent: Component;

	const NAME: &str;
	const ID: Uuid;

	/// Used to prevent this Ui from appearing in the view menu
	const HIDDEN: bool = false;

	const CLOSEABLE: bool = true;

	const CAN_CLEAR: bool = true;

	const SCROLL_BARS: [bool; 2] = [true, true];

	const UNIQUE: bool = false;

	const POPOUT: bool = true;

	const REOPEN_ON_STARTUP: bool = true;

	/// Add systems or resources that this UI needs in order to function
	fn init(app: &mut App) {
		let _ = app;
	}

	fn spawn(entity: Entity, world: &mut World) -> Self;

	fn on_despawn(entity: Entity, world: &mut World) {
		let _ = entity;
		let _ = world;
	}

	fn title(entity: Entity, world: &mut World) -> egui::WidgetText {
		let _ = entity;
		let _ = world;
		Self::NAME.into()
	}

	fn ui(entity: Entity, ui: &mut egui::Ui, world: &mut World);

	fn context_menu(
		entity: Entity,
		ui: &mut egui::Ui,
		world: &mut World,
		surface: SurfaceIndex,
		node: NodeIndex,
	) {
		let _ = entity;
		let _ = ui;
		let _ = world;
		let _ = surface;
		let _ = node;
	}

	fn on_panel_changed(entity: Entity, world: &mut World) {
		let _ = entity;
		let _ = world;
	}

	fn handle_tab_response(entity: Entity, world: &mut World, response: &egui::Response) {
		let _ = entity;
		let _ = world;
		let _ = response;
	}
}

#[derive(SystemParam)]
pub struct NoParams;

pub trait EditorUi: EditorUiBundle + Component {
	const NAME: &str;
	const ID: Uuid;

	const HIDDEN: bool = false;

	const CLOSEABLE: bool = true;

	const CAN_CLEAR: bool = true;

	const SCROLL_BARS: [bool; 2] = [true, true];

	const UNIQUE: bool = false;

	const POPOUT: bool = true;

	const REOPEN_ON_STARTUP: bool = true;

	type Params<'w, 's>: for<'world, 'system> SystemParam<
		Item<'world, 'system> = Self::Params<'world, 'system>,
	>;

	/// Add systems or resources that this UI needs in order to function
	fn init(app: &mut App) {
		let _ = app;
	}

	fn spawn(params: Self::Params<'_, '_>) -> Self;

	fn title(&mut self, params: Self::Params<'_, '_>) -> egui::WidgetText {
		let _ = params;
		<Self as EditorUi>::NAME.into()
	}

	fn ui(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>);

	fn context_menu(
		&mut self,
		ui: &mut egui::Ui,
		params: Self::Params<'_, '_>,
		surface: SurfaceIndex,
		node: NodeIndex,
	) {
		let _ = ui;
		let _ = params;
		let _ = surface;
		let _ = node;
	}

	fn handle_tab_response(&mut self, params: Self::Params<'_, '_>, response: &egui::Response) {
		let _ = params;
		let _ = response;
	}

	fn on_panel_changed(&mut self, params: Self::Params<'_, '_>) {
		let _ = params;
	}

	fn on_despawn(&mut self, params: Self::Params<'_, '_>) {
		let _ = params;
	}
}

impl<T> EditorUiBundle for T
where
	Self: Component<Mutability = Mutable> + EditorUi + 'static,
{
	type PrimaryComponent = Self;

	const NAME: &str = <Self as EditorUi>::NAME;
	const ID: Uuid = <T as EditorUi>::ID;

	const HIDDEN: bool = <Self as EditorUi>::HIDDEN;

	const CLOSEABLE: bool = <Self as EditorUi>::CLOSEABLE;

	const CAN_CLEAR: bool = <Self as EditorUi>::CAN_CLEAR;

	const SCROLL_BARS: [bool; 2] = <Self as EditorUi>::SCROLL_BARS;

	const UNIQUE: bool = <Self as EditorUi>::UNIQUE;

	const POPOUT: bool = <Self as EditorUi>::POPOUT;

	const REOPEN_ON_STARTUP: bool = <Self as EditorUi>::REOPEN_ON_STARTUP;

	fn init(app: &mut App) {
		<Self as EditorUi>::init(app);
		if <Self as EditorUi>::UNIQUE {
			app.add_observer(make_singleton::<Self>);
		}
	}

	fn spawn(entity: Entity, world: &mut World) -> Self {
		Self::register_params(entity, world);
		Self::with_params(entity, world, EditorUi::spawn)
	}

	fn title(entity: Entity, world: &mut World) -> egui::WidgetText {
		Self::get_entity_mut(entity, world, EditorUi::title)
	}

	fn ui(entity: Entity, ui: &mut egui::Ui, world: &mut World) {
		Self::get_entity_mut(entity, world, |this, params| {
			this.ui(ui, params);
		})
	}

	fn context_menu(
		entity: Entity,
		ui: &mut egui::Ui,
		world: &mut World,
		surface: SurfaceIndex,
		node: NodeIndex,
	) {
		Self::get_entity_mut(entity, world, |this, params| {
			this.context_menu(ui, params, surface, node);
		})
	}

	fn handle_tab_response(entity: Entity, world: &mut World, response: &egui::Response) {
		Self::get_entity_mut(entity, world, |this, params| {
			this.handle_tab_response(params, response);
		});
	}

	fn on_panel_changed(entity: Entity, world: &mut World) {
		Self::get_entity_mut(entity, world, <Self as EditorUi>::on_panel_changed)
	}

	fn on_despawn(entity: Entity, world: &mut World) {
		Self::get_entity_mut(entity, world, <Self as EditorUi>::on_despawn)
	}
}

#[derive(Resource)]
pub struct UiManager {
	state: DockState<TabState>,
	vtables: HashMap<PersistentId, &'static VTable>,
	id: egui::Id,
}

impl UiManager {
	pub(crate) fn new(app: &mut App) -> Self {
		let mut this = Self {
			state: DockState::new(Vec::new()),
			vtables: default(),
			id: egui::Id::new(TypeId::of::<Self>()),
		};

		this.register::<MissingUi>(app);

		this.register::<AssetsUi>(app);
		this.register::<ComponentsUi>(app);
		this.register::<DiagnosticsUi>(app);
		this.register::<EditorSettingsUi>(app);
		this.register::<EditorViewUi>(app);
		this.register::<HierarchyUi>(app);
		this.register::<InspectorUi>(app);
		this.register::<LogUi>(app);
		this.register::<PrefabsUi>(app);
		this.register::<ProjectSettingsUi>(app);
		this.register::<ResourcesUi>(app);
		this.register::<TypeEditorUi>(app);

		let state = SystemState::<menu_bar::Params<'_, '_>>::new(app.world_mut());
		app.insert_resource(UiResourceState::new(state));

		this
	}

	pub fn register<T: EditorUiBundle>(&mut self, app: &mut App) {
		T::init(app);
		self.vtables.insert(PersistentId(T::ID), &T::VTABLE);
	}

	pub fn save_state(
		&self,
		q_uuids: &Query<&PersistentId, Without<MissingUi>>,
		q_missing: &Query<&MissingUi>,
	) -> DockState<LayoutInfo> {
		self.state.decouple(self, q_uuids, q_missing)
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
				let layout = project_settings.get(SavedLayout(name))?;
				DockState::restore(&layout, &self.vtables, world)
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
			.query::<&mut bevy_egui::EguiContext>()
			.single_mut(world)
			.map(|mut ctx| ctx.get_mut().clone())
		else {
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

	fn vtables(&self) -> &HashMap<PersistentId, &'static VTable> {
		&self.vtables
	}

	fn get_vtable_by_id(&self, id: &PersistentId) -> Option<&'static VTable> {
		self.vtables.get(id).cloned()
	}

	fn switch_state(&mut self, new_state: DockState<TabState>, world: &mut World) {
		for (_, tab) in self.state.iter_all_tabs() {
			(tab.vtable.despawn)(tab.entity, world);
		}
		self.state = new_state;
	}

	fn default_dock_state(&self, world: &mut World) -> DockState<TabState> {
		let mut state = DockState::new(vec![TabState::new::<EditorViewUi>(world)]);

		let tree = state.main_surface_mut();

		let root = NodeIndex::root();

		let tabs = vec![
			TabState::new::<HierarchyUi>(world),
			TabState::new::<DiagnosticsUi>(world),
		];
		let [central_panel, _left_panel] = tree.split_left(root, 1.0 / 6.0, tabs);

		let tabs = vec![TabState::new::<InspectorUi>(world)];
		let [central_panel, _right_panel] = tree.split_right(central_panel, 4.0 / 5.0, tabs);

		let tabs = vec![
			TabState::new::<PrefabsUi>(world),
			TabState::new::<ComponentsUi>(world),
			TabState::new::<ResourcesUi>(world),
			TabState::new::<AssetsUi>(world),
		];
		tree.split_below(central_panel, 0.7, tabs);

		state
	}

	fn state(&self) -> &DockState<TabState> {
		&self.state
	}
}

#[derive(Clone)]
pub struct TabState {
	entity: Entity,
	vtable: &'static VTable,
}

impl TabState {
	fn new<T: EditorUiBundle>(world: &mut World) -> Self {
		Self {
			entity: (T::VTABLE.spawn)(world),
			vtable: &T::VTABLE,
		}
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
		T: EditorUiBundle,
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

	fn spawn<T: EditorUiBundle>(world: &mut World) -> Entity {
		info!("Spawning UI component {}", T::NAME);
		let entity = world
			.spawn((Name::new(T::NAME), PersistentId(T::ID), UiState::default()))
			.id();

		let ui_scene = world
			.query_filtered::<Entity, With<UiPanels>>()
			.iter(world)
			.next()
			.unwrap();
		world.entity_mut(ui_scene).add_child(entity);

		let instance = T::spawn(entity, world);
		world.entity_mut(entity).insert(instance).id()
	}

	fn despawn<T: EditorUiBundle>(entity: Entity, world: &mut World) {
		info!("Despawning UI component {}", T::NAME);
		<T as EditorUiBundle>::on_despawn(entity, world);
		world.trigger(RemoveUiEvent::new(entity));
	}

	fn count<T: EditorUiBundle>(world: &mut World) -> usize {
		let mut q_uis = world.query::<&T::PrimaryComponent>();
		q_uis.iter(world).len()
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
		let mut q_ids = world.query::<&mut UiState>();
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

		self.ui_state_mut(tab.entity, |state| {
			state.hovered = ui.ui_contains_pointer();
		});
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

fn handle_click_events(
	event: On<Pointer<Click>>,
	mut commands: Commands,
	editor_camera_pointer_id: Single<&PointerId, With<EditorCamera>>,
	mut selection: ResMut<InspectorSelection>,
	keyboard: Res<ButtonInput<KeyCode>>,
) {
	if event.pointer_id != **editor_camera_pointer_id || event.button != PointerButton::Primary {
		return;
	}

	let target = event.event_target();

	let event = selection.add_selected(
		target,
		keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight),
	);

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
			entity.insert(MeshOutline::new(2.0));
		}
	}
}

fn handle_deselected(event: On<Remove, Selected>, mut commands: Commands) {
	if let Ok(mut entity) = commands.get_entity(event.event_target()) {
		entity.queue_silenced(entity_command::remove::<(GizmoTarget, MeshOutline)>());
	}
}

/// Component that stores all ui components as children for organization
#[derive(Component)]
pub struct UiPanels;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default)]
pub enum KeyboardFocus {
	#[default]
	Unfocused,
	Focused(egui::Id),
}

impl KeyboardFocus {
	fn set_state(
		mut q_contexts: Query<&mut bevy_egui::EguiContext>,
		mut keyboard_focus: ResMut<NextState<Self>>,
	) {
		let focus = q_contexts
			.iter_mut()
			.find_map(|mut ctx| ctx.get_mut().memory(|memory| memory.focused()));

		keyboard_focus.set(focus.map(Self::Focused).unwrap_or(Self::Unfocused))
	}
}

fn should_render_ui(editor_settings: Res<RuntimeSettings>) -> bool {
	editor_settings.render_ui
}
