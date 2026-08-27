pub mod events;
pub mod input;
pub mod misc;
pub mod persistence;
pub mod view;

use crate::{
	EditorUiWorld,
	inspector::add_single,
	private::{
		EditorInternalFilter, EditorInternalQuery, EditorInternalSingle, EditorOwned, EditorScene,
		UserHidden,
		cam::EDITOR_VIEW_RENDER_LAYER,
		ext::{
			assets, components, content, diagnostics, hierarchy, inspector, menu_bar, resources,
			scene_view,
		},
		ui::persistence::SavedLayout,
		util::extensions::WorldMutExtensions,
	},
	storage::{
		GlobalEditorSettings, ProjectSettings,
		settings::{CurrentLayoutSetting, EditorEguiSettings, EditorUiScale},
	},
	ui::OpenUi,
};
use bevy::{
	camera::visibility::RenderLayers,
	ecs::{schedule::ScheduleLabel, system::SystemState},
	platform::collections::HashMap,
	prelude::*,
};
use bevy_egui::{EguiContext, EguiGlobalSettings, EguiPlugin, EguiSchedule};
use bevy_mod_outline::{OutlineMode, OutlineRenderLayers, OutlineVolume};
use derive_new::new;
use egui_dock::{DockArea, DockState, NodeIndex, SurfaceIndex, TabIndex};
use events::{AppendUiMessage, RemoveUiEvent};
use itertools::Itertools;
use misc::{DockExtensions, EditorUiExtensions, UiResourceState};
use misc::{MissingUi, UiState};
use notify::Notification;
use persistence::{LayoutInfo, LayoutsTable};
use persistent_id::PersistentId;
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
			.add_plugins((
				EguiPlugin::default(),
				view::EditorUiViewPlugin,
				input::EditorUiInputPlugin,
				persistence::PersistencePlugin,
			))
			.insert_resource(egui_settings)
			.init_resource::<HighlightOptions>()
			.init_resource::<LayoutManager>()
			.init_resource::<UiDockState>()
			.init_resource::<UiVTables>()
			.init_resource::<NewTabs>()
			.add_message::<AppendUiMessage>()
			.add_observer(on_new_scene)
			.add_observer(on_new_ctx)
			.add_observer(RemoveUiEvent::on_event)
			.add_systems(Startup, create_ui)
			.add_systems(
				FixedUpdate,
				(
					AppendUiMessage::handle,
					handle_open_ui_requests,
					reparent_editor_ui,
				),
			)
			.add_systems(EditorUiEguiContextPass, (reset_ui_state, ui).chain());

		let type_registry = app.world().resource::<AppTypeRegistry>();
		let mut type_registry = type_registry.write();

		add_single::<UiState>(&mut type_registry);
	}
}

fn create_ui(world: &mut World) -> Result {
	let state = SystemState::<menu_bar::Params<'_, '_>>::new(world);
	world.insert_resource(UiResourceState::new(state));
	world.resource_scope(|world, mut state: Mut<UiDockState>| state.load(world))
}

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EditorUiEguiContextPass;

#[derive(Component, Default, Clone)]
#[require(EguiContext, EguiSchedule::new(EditorUiEguiContextPass))]
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

#[derive(Resource, Deref, DerefMut)]
pub struct UiDockState(DockState<TabState>);

impl Default for UiDockState {
	fn default() -> Self {
		Self(DockState::new(Vec::new()))
	}
}

impl UiDockState {
	fn load(&mut self, world: &mut World) -> Result<()> {
		let mut sys_state = SystemState::<ProjectSettings>::new(world);
		let mut project_settings = sys_state.get_mut(world)?;

		let current_layout_name = project_settings.get(CurrentLayoutSetting).ok();

		let mut dock = match current_layout_name {
			Some(name) => {
				if let Ok(layout) = project_settings.get(SavedLayout::new(name)) {
					DockState::restore(layout, world)
				} else {
					Self::try_make_default(world)?
				}
			}
			None => Self::try_make_default(world)?,
		};

		// resets any surfaces that have an active
		// tab that does not not reopen on startup
		for (_, leaf) in dock.iter_leaves_mut() {
			if leaf.active_focused().is_none() {
				leaf
					.set_active_tab(0)
					.expect("Failed to reset dock state tab");
			}
		}

		self.0 = dock;

		Ok(())
	}
}

impl UiDockState {
	pub fn add_detached(&mut self, tabs: Vec<TabState>) -> SurfaceIndex {
		self.add_window(tabs)
	}

	pub fn append_tab(&mut self, surface: SurfaceIndex, node: NodeIndex, tab: TabState) -> bool {
		let Some(surface) = self.get_surface_mut(surface) else {
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
		let Some(surface) = self.get_surface_mut(surface) else {
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
		let Some(np) = self.focused_leaf() else {
			return false;
		};

		self.append_tab(np.surface, np.node, tab)
	}

	pub(crate) fn switch(&mut self, new_state: DockState<TabState>, world: &mut World) -> Result {
		for (_, tab) in self.iter_all_tabs() {
			(tab.vtable.despawn)(tab.entity, world)?;
		}

		**self = new_state;

		Ok(())
	}

	pub fn try_make_default(world: &mut World) -> Result<DockState<TabState>> {
		let mut state = DockState::new(vec![TabState::new::<scene_view::SceneViewUi>(world)?]);

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

	fn save(
		&self,
		vtables: &UiVTables,
		q_uuids: &EditorInternalQuery<&PersistentId, Without<MissingUi>>,
		q_missing: &EditorInternalQuery<&MissingUi>,
	) -> DockState<LayoutInfo> {
		self.decouple(vtables, q_uuids, q_missing)
	}
}

#[derive(Resource, Deref, DerefMut, Default)]
pub struct UiVTables(HashMap<PersistentId, &'static VTable>);

impl UiVTables {
	pub fn register<T: EditorUiWorld>(&mut self) {
		let key = PersistentId(T::ID);
		if self.contains_key(&key) {
			panic!("Already registered Ui {}", std::any::type_name::<T>());
		}

		self.insert(key, &T::VTABLE);
	}
}

#[derive(new, Resource, Deref, DerefMut)]
pub struct LayoutManager(BTreeSet<String>);

impl FromWorld for LayoutManager {
	fn from_world(world: &mut bevy::ecs::world::World) -> Self {
		let mut sys_state = SystemState::<ProjectSettings>::new(world);
		let mut project_settings = sys_state.get_mut(world).unwrap();
		let layouts = BTreeSet::from_iter(project_settings.list_keys::<LayoutsTable>().unwrap());
		Self::new(layouts)
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

#[derive(Clone)]
pub struct VTable {
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

fn ui(world: &mut World) -> Result {
	let mut result = Ok(());
	world.resources_scope::<(UiDockState, UiVTables)>(|world, (mut dock_state, vtables)| {
		let Ok(ctx) = world
			.query_filtered::<&mut EguiContext, EditorInternalFilter<With<EditorEguiContext>>>()
			.single_mut(world)
			.map(|mut ctx| ctx.get_mut().clone())
		else {
			world.trigger(Notification::error("No egui context to render to"));
			return;
		};

		let mut ui = egui::Ui::new(ctx.clone(), "BEDITOR_UI".into(), egui::UiBuilder::new());

		let style = ui.style();

		let dock_style = egui_dock::Style::from_egui(style);

		result = egui::CentralPanel::default()
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
					vtables,
					world: RefCell::new(world),
				};

				DockArea::new(&mut dock_state)
					.id(ui.id().with(TypeId::of::<DockState<TabState>>()))
					.style(dock_style)
					.show_add_buttons(true)
					.show_add_popup(true)
					.show_inside(ui, &mut tab_viewer);

				Ok(())
			})
			.inner;
	});
	result
}

struct TabViewer<'a> {
	/// RefCell so that functions with &self can access a mut World
	world: RefCell<&'a mut World>,
	vtables: Mut<'a, UiVTables>,
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

// ----- scene editing ------

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

	egui_extras::install_image_loaders(ctx);

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
