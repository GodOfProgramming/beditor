pub mod events;
pub mod misc;

use crate::{
	EditorState, EditorUiWorld,
	inspector::{
		add_single,
		ui::{InspectorSelection, Selected},
	},
	private::{
		EditorInternalFilter, EditorInternalQuery, EditorInternalSingle, EditorOwned, EditorScene,
		UserHidden,
		cam::{EDITOR_VIEW_RENDER_LAYER, EditorCamera},
		ext::{
			assets, components, content, diagnostics,
			editor_view::{self, GizmoOptions},
			hierarchy, inspector, menu_bar, resources,
		},
		util::{WorldExtensions, entity::insert_bundle_from_world},
	},
	storage::{
		DataTable, GlobalEditorSettings, PersistentData, ProjectSettings,
		settings::{CurrentLayoutSetting, EditorEguiSettings, EditorUiScale, SaveLayoutOnExitSetting},
	},
	ui::OpenUi,
};
use bevy::{
	camera::visibility::RenderLayers,
	ecs::{
		schedule::ScheduleLabel,
		system::{SystemState, entity_command},
	},
	picking::{
		hover::{PickingInteraction, update_interactions},
		pointer::PointerId,
	},
	platform::collections::HashMap,
	prelude::*,
	ui::ui_focus_system,
};
use bevy_egui::{EguiContext, EguiGlobalSettings, EguiMultipassSchedule, EguiPlugin};
use bevy_mod_outline::{OutlineMode, OutlineRenderLayers, OutlineVolume};
use derive_new::new;
use egui_dock::{DockArea, DockState, NodeIndex, SurfaceIndex, TabIndex};
use events::{AppendUiMessage, RemoveUiEvent};
use itertools::Itertools;
use misc::{DockExtensions, EditorUiExtensions, UiResourceState};
use misc::{MissingUi, UiState};
use persistent_id::PersistentId;
use serde::{Deserialize, Serialize};
use std::{any::TypeId, cell::RefCell, collections::BTreeSet};

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
			.init_resource::<LayoutManager>()
			.init_resource::<NewTabs>()
			.init_state::<KeyboardFocus>()
			.add_message::<AppendUiMessage>()
			.add_plugins(EguiPlugin::default())
			.add_observer(on_new_scene)
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
				PreUpdate,
				forward_interactions
					.after(ui_focus_system)
					.after(update_interactions),
			)
			.add_systems(
				FixedUpdate,
				(
					AppendUiMessage::handle,
					handle_open_ui_requests,
					reparent_editor_ui,
				),
			)
			.add_systems(
				EditorUiEguiContextPass,
				(reset_ui_state, editor_ui, KeyboardFocus::set_state).chain(),
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

#[derive(Component)]
#[require(
  UserHidden,
  GlobalTransform,
  UiTransform,
  Visibility,
  Name = Name::new("Editor Ui"),
  InheritedVisibility,
)]
struct EditorUiContainer;

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

	pub fn append_tab(&mut self, surface: SurfaceIndex, node: NodeIndex, tab: TabState) -> bool {
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

	pub fn insert_and_focus(
		&mut self,
		surface: SurfaceIndex,
		node: NodeIndex,
		neighbor: TabIndex,
		tab: TabState,
	) -> bool {
		let Some(surface) = self.state.get_surface_mut(surface) else {
			return false;
		};

		let Some(nodes) = surface.node_tree_mut() else {
			return false;
		};

		let node = &mut nodes[node];

		node.insert_tab(neighbor, tab);

		true
	}

	pub fn add_tab_to_focused(&mut self, tab: TabState) -> bool {
		let Some(np) = self.state.focused_leaf() else {
			return false;
		};

		self.append_tab(np.surface, np.node, tab)
	}

	fn init(world: &mut World) -> Result {
		let state = SystemState::<menu_bar::Params<'_, '_>>::new(world);
		world.insert_resource(UiResourceState::new(state));
		world.resource_scope(|world, mut this: Mut<Self>| this.restore_or_init(world))
	}

	fn restore_or_init(&mut self, world: &mut World) -> Result {
		let mut sys_state = SystemState::<ProjectSettings>::new(world);
		let mut project_settings = sys_state.get_mut(world)?;

		let current_layout_name = project_settings.get(CurrentLayoutSetting).ok();

		let layouts = BTreeSet::from_iter(project_settings.list_keys::<LayoutsTable>()?);

		let mut dock = match current_layout_name {
			Some(name) => {
				if let Ok(layout) = project_settings.get(SavedLayout(name)) {
					DockState::restore(layout, &self.vtables, world)
				} else {
					self.default_dock_state(world)?
				}
			}
			None => self.default_dock_state(world)?,
		};

		// resets any surfaces that have an active
		// tab that does not not reopen on startup
		for (_, leaf) in dock.iter_leaves_mut() {
			if leaf.active_focused().is_none() {
				leaf.set_active_tab(0)?;
			}
		}

		self.state = dock;

		world.insert_resource(LayoutManager::new(layouts));

		Ok(())
	}

	fn ui(&mut self, world: &mut World) -> Result<()> {
		let Ok(ctx) = world
			.query_filtered::<&mut EguiContext, EditorInternalFilter<With<EditorEguiContext>>>()
			.single_mut(world)
			.map(|mut ctx| ctx.get_mut().clone())
		else {
			return Err(BevyError::error("No egui context to render to"));
		};

		let mut ui = egui::Ui::new(ctx.clone(), "BEDITOR_UI".into(), egui::UiBuilder::new());

		let style = ui.style();

		let dock_style = egui_dock::Style::from_egui(style);

		egui::CentralPanel::default()
			.frame(
				egui::Frame::central_panel(style)
					.inner_margin(0)
					.fill(dock_style.tab.tab_body.bg_fill),
			)
			.show(&mut ui, |ui| -> Result {
				// menu bar
				world.resource_scope(
					|world, mut state: Mut<UiResourceState<menu_bar::Params>>| -> Result {
						let params = state.get_mut(world)?;
						menu_bar::render(ui, params);
						state.apply(world);
						Ok(())
					},
				)?;

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

				Ok(())
			})
			.inner?;

		Ok(())
	}

	fn save_state(
		&self,
		q_uuids: &EditorInternalQuery<&PersistentId, Without<MissingUi>>,
		q_missing: &EditorInternalQuery<&MissingUi>,
	) -> DockState<LayoutInfo> {
		self.state.decouple(self, q_uuids, q_missing)
	}

	pub(crate) fn switch_state(
		&mut self,
		new_state: DockState<TabState>,
		world: &mut World,
	) -> Result {
		for (_, tab) in self.state.iter_all_tabs() {
			(tab.vtable.despawn)(tab.entity, world)?;
		}
		self.state = new_state;
		Ok(())
	}

	pub(crate) fn default_dock_state(&self, world: &mut World) -> Result<DockState<TabState>> {
		let mut state = DockState::new(vec![TabState::new::<editor_view::EditorViewUi>(world)?]);

		let tree = state.main_surface_mut();

		let root = NodeIndex::root();

		let tabs = vec![
			TabState::new::<hierarchy::HierarchyUi>(world)?,
			TabState::new::<diagnostics::DiagnosticsUi>(world)?,
		];
		let [central_panel, _left_panel] = tree.split_left(root, 1.0 / 6.0, tabs);

		let tabs = vec![TabState::new::<inspector::InspectorUi>(world)?];
		let [central_panel, _right_panel] = tree.split_right(central_panel, 4.0 / 5.0, tabs);

		let tabs = vec![
			TabState::new::<content::ContentUi>(world)?,
			TabState::new::<components::ComponentsUi>(world)?,
			TabState::new::<resources::ResourcesUi>(world)?,
			TabState::new::<assets::AssetsUi>(world)?,
		];
		tree.split_below(central_panel, 0.7, tabs);

		Ok(state)
	}

	pub(crate) fn state(&self) -> &DockState<TabState> {
		&self.state
	}
}

#[derive(Resource, Default, Deref, DerefMut)]
pub struct NewTabs {
	requests: Vec<OpenUi>,
}

#[derive(Clone, Copy)]
pub struct TabState {
	entity: Entity,
	pub(crate) vtable: &'static VTable,
}

impl TabState {
	pub(crate) fn new<T: EditorUiWorld>(world: &mut World) -> Result<Self> {
		Ok(Self {
			entity: (T::VTABLE.spawn)(world)?,
			vtable: &T::VTABLE,
		})
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
	type Out = ();
	fn apply(self, world: &mut World) {
		world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
			let new_state = DockState::restore(self.0, &ui_manager.vtables, world);
			world
				.notify_on_error(
					|world| ui_manager.switch_state(new_state, world),
					|_, err| ("Failed to switch to layout", Some(err)),
				)
				.ok();
		})
	}
}

#[derive(Clone)]
pub(crate) struct VTable {
	pub(crate) name: &'static str,
	pub(crate) closeable: bool,
	pub(crate) hidden: bool,
	pub(crate) can_clear: bool,
	pub(crate) scroll_bars: [bool; 2],
	pub(crate) unique: bool,
	pub(crate) popout: bool,
	pub(crate) reopen_on_startup: bool,
	pub(crate) spawn: fn(&mut World) -> Result<Entity>,
	pub(crate) despawn: fn(Entity, &mut World) -> Result,
	pub(crate) title: fn(Entity, &mut World) -> Result<egui::WidgetText>,
	pub(crate) render: fn(Entity, &mut egui::Ui, &mut World) -> Result,
	pub(crate) context_menu: fn(Entity, &mut egui::Ui, &mut World, SurfaceIndex, NodeIndex) -> Result,
	pub(crate) handle_tab_response: fn(Entity, &mut World, &egui::Response) -> Result,
	pub(crate) on_panel_changed: fn(Entity, &mut World) -> Result,
	pub(crate) count: fn(&mut World) -> usize,
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

	fn spawn<T: EditorUiWorld>(world: &mut World) -> Result<Entity> {
		info!("Spawning UI component {}", T::NAME);
		let entity = world
			.spawn((
				Name::new(T::NAME),
				EditorOwned,
				PersistentId(T::ID),
				UiState::default(),
			))
			.id();

		let instance = T::spawn(entity, world)?;
		Ok(world.entity_mut(entity).insert(instance).id())
	}

	fn despawn<T: EditorUiWorld>(entity: Entity, world: &mut World) -> Result {
		info!("Despawning UI component {}", T::NAME);
		<T as EditorUiWorld>::on_despawn(entity, world)?;
		world.trigger(RemoveUiEvent::new(entity));
		Ok(())
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
	fn ui_state_mut(world: &mut World, entity: Entity, f: impl FnOnce(&mut UiState)) {
		let mut q_ids = world.query_filtered::<&mut UiState, EditorInternalFilter>();
		let ui_info = q_ids.get_mut(world, entity).ok();
		if let Some(mut ui_info) = ui_info {
			f(&mut ui_info);
		}
	}

	fn spawn_ui(world: &mut World, vtable: &VTable) -> Result<Entity, ()> {
		world.notify_on_error(
			|world| (vtable.spawn)(world),
			|_, err| (format!("Failed to spawn UI {}", vtable.name,), Some(err)),
		)
	}
}

impl egui_dock::TabViewer for TabViewer<'_> {
	type Tab = TabState;

	fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
		(tab.vtable.title)(tab.entity, &mut self.world.borrow_mut()).unwrap_or_else(|err| {
			let msg = format!("Failed to get title of {}", tab.vtable.name);
			error!(err = err.to_string(), "{msg}");
			egui::WidgetText::Text(msg)
		})
	}

	#[profiling::function]
	fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
		let mut world = self.world.borrow_mut();
		let Ok(_) = world.notify_on_error(
			|world| (tab.vtable.render)(tab.entity, ui, world),
			|_, err| {
				(
					format!("Failed to render ui {}", tab.vtable.name),
					Some(err),
				)
			},
		) else {
			return;
		};
		if ui.ui_contains_pointer() {
			Self::ui_state_mut(&mut world, tab.entity, |state| {
				state.mark_hovered();
			});
		}
	}

	#[profiling::function]
	fn add_popup(&mut self, ui: &mut egui::Ui, path: egui_dock::NodePath) {
		let egui_dock::NodePath { surface, node } = path;

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
						let Ok(entity) = Self::spawn_ui(&mut world, vtable) else {
							return;
						};

						world.write_message(AppendUiMessage::new(
							surface,
							node,
							TabState { entity, vtable },
						));
					}
				});
			} else if ui.button(vtable.name).clicked() {
				let mut world = self.world.borrow_mut();
				let Ok(entity) = Self::spawn_ui(&mut world, vtable) else {
					return;
				};
				world.write_message(AppendUiMessage::new(
					surface,
					node,
					TabState { entity, vtable },
				));
			}
		}
	}

	fn context_menu(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab, path: egui_dock::NodePath) {
		let mut world = self.world.borrow_mut();
		world
			.notify_on_error(
				|world| (tab.vtable.context_menu)(tab.entity, ui, world, path.surface, path.node),
				|_, err| ("Failed to render context menu", Some(err)),
			)
			.ok();
	}

	fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &egui::Response) {
		let mut world = self.world.borrow_mut();
		world
			.notify_on_error(
				|world| (tab.vtable.handle_tab_response)(tab.entity, world, response),
				|_, err| {
					(
						format!("Failed to open tab button menu for {}", tab.vtable.name),
						Some(err),
					)
				},
			)
			.ok();
	}

	fn closeable(&mut self, tab: &mut Self::Tab) -> bool {
		tab.vtable.closeable
	}

	fn on_close(&mut self, tab: &mut Self::Tab) -> egui_dock::tab_viewer::OnCloseResponse {
		let mut world = self.world.borrow_mut();
		world
			.notify_on_error(
				|world| (tab.vtable.despawn)(tab.entity, world),
				|_, err| {
					(
						format!("Error when closing tab {}", tab.vtable.name),
						Some(err),
					)
				},
			)
			.ok();
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
		let mut world = self.world.borrow_mut();
		world
			.notify_on_error(
				|world| (tab.vtable.on_panel_changed)(tab.entity, world),
				|_, err| {
					(
						format!("Error when detecting rect change for {}", tab.vtable.name),
						Some(err),
					)
				},
			)
			.ok();
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
	q_transforms: Query<(), With<Transform>>,
	gizmo_options: Res<GizmoOptions>,
) {
	let entity = event.event_target();
	if q_transforms.contains(entity)
		&& let Ok(mut entity_commands) = commands.get_entity(entity)
	{
		if gizmo_options.enabled() {
			entity_commands.insert(TransformGizmoFocus);
		}

		if q_3d_meshes.contains(entity_commands.id()) {
			entity_commands.queue_handled(insert_bundle_from_world::<Highlight>(), |err, ctx| {
				error!(ctx = ctx.to_string(), "{err}");
			});
		}
	}
}

fn handle_deselected(event: On<Remove, Selected>, mut commands: Commands) {
	if let Ok(mut entity) = commands.get_entity(event.event_target()) {
		entity.queue_silenced(entity_command::remove::<(TransformGizmoFocus, Highlight)>());
	}
}

/// This exists as a state because you need to have immutable data in a run_if
/// and egui contexts need mutable access
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default)]
pub enum KeyboardFocus {
	#[default]
	Unfocused,
	Focused,
}

impl KeyboardFocus {
	fn set_state(
		mut q_contexts: EditorInternalQuery<&mut EguiContext>,
		mut keyboard_focus: ResMut<NextState<Self>>,
	) {
		let none_focused = q_contexts
			.iter_mut()
			.all(|mut ctx| !ctx.get_mut().egui_wants_keyboard_input());

		if none_focused {
			keyboard_focus.set(KeyboardFocus::Unfocused);
		} else {
			keyboard_focus.set(KeyboardFocus::Focused);
		}
	}
}

fn on_new_scene(event: On<Add, EditorScene>, mut commands: Commands) {
	commands.spawn((EditorUiContainer, ChildOf(event.event_target())));
}

fn on_new_ctx(
	event: On<Add, EditorEguiContext>,
	mut q_ctx: EditorInternalQuery<&mut EguiContext>,
	mut settings: GlobalEditorSettings,
) {
	let Ok(mut ctx) = q_ctx.get_mut(event.event_target()) else {
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

	if let Ok(zoom) = settings.get(EditorUiScale) {
		ctx.set_zoom_factor(zoom);
	}
}

fn reset_ui_state(mut q_ui_state: EditorInternalQuery<&mut UiState>) {
	q_ui_state.par_iter_mut().for_each(|mut state| {
		state.clear();
	});
}

fn editor_ui(world: &mut World) {
	world
		.notify_on_error(
			|world| world.resource_scope(|world, mut ui_manager: Mut<UiManager>| ui_manager.ui(world)),
			|_, err| ("Failed to render ui", Some(err)),
		)
		.ok();
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
	mut ctx: Single<&mut EguiContext, With<EditorEguiContext>>,
	mut settings: GlobalEditorSettings,
) {
	let ctx = ctx.get_mut();
	settings.set(EditorUiScale, ctx.zoom_factor()).ok();
}

fn save_layouts(
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

fn handle_open_ui_requests(mut commands: Commands, mut new_tabs: ResMut<NewTabs>) {
	for request in new_tabs.requests.drain(..) {
		commands.queue(move |world: &mut World| {
			(request.0)(world);
		});
	}
}

fn reparent_editor_ui(
	mut commands: Commands,
	editor_ui: EditorInternalSingle<Entity, With<EditorUiContainer>>,
	q_uis: EditorInternalQuery<Entity, (With<UiState>, Without<ChildOf>)>,
) {
	for ui in &q_uis {
		commands.entity(*editor_ui).add_child(ui);
	}
}

fn forward_interactions(
	mut q_interactions: EditorInternalQuery<(&mut Interaction, &PickingInteraction)>,
) {
	for (mut entity_interaction, picking_interaction) in &mut q_interactions {
		let interaction = match picking_interaction {
			PickingInteraction::Pressed => Interaction::Pressed,
			PickingInteraction::Hovered => Interaction::Hovered,
			PickingInteraction::None => Interaction::None,
		};

		if *entity_interaction != interaction {
			*entity_interaction = interaction;
		}
	}
}
