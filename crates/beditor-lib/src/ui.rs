pub mod components;
pub mod events;
pub mod managers;
pub mod misc;
pub mod prebuilt;

use crate::{
  EditorSettings, EditorState, Settings,
  ui::{
    events::AddUiMessage,
    managers::{CurrentLayoutSetting, SaveLayoutOnExitSetting},
  },
  util::storage::Layouts,
  view::mouse_hovered_in_editor_view,
};
use bevy::{
  asset::UntypedAssetId,
  camera::visibility::{Layer, RenderLayers},
  ecs::{component::Mutable, query::QueryFilter, system::SystemParam},
  platform::collections::HashMap,
  prelude::*,
  reflect::GetTypeRegistration,
};
use bevy_egui::{EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext};
use bevy_inspector_egui::{DefaultInspectorConfigPlugin, bevy_inspector};
use egui_dock::{NodeIndex, SurfaceIndex};
use events::RemoveUiEvent;
use itertools::{Either, Itertools};
use managers::{LayoutManager, UiManager};
use misc::{MissingUi, UiExtensions, UiState};
use persistent_id::PersistentId;
use std::{any::TypeId, cell::RefCell, marker::PhantomData};
use uuid::Uuid;

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct EditorUiSystems;

#[derive(Default, Component, Reflect)]
#[require(
  PrimaryEguiContext = default_primary_context(),
  Camera = editor_camera(),
  Camera2d,
  RenderLayers = RenderLayers::layer(EDITOR_UI_LAYER))]
pub struct EditorUiCamera;

fn default_primary_context() -> PrimaryEguiContext {
  PrimaryEguiContext
}

fn editor_camera() -> Camera {
  Camera {
    order: 1,
    ..default()
  }
}

pub const EDITOR_UI_LAYER: Layer = 31;

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
      .add_plugins((
        EguiPlugin::default(),
        DefaultInspectorConfigPlugin,
        InspectorIntegrationPlugin::<Or<(With<Sprite>, With<Mesh2d>, With<Mesh3d>, With<Node>)>>::default(),
      ))
      .init_resource::<InspectorSelection>()
      .init_resource::<LayoutManager>()
      .init_state::<KeyboardFocus>()
      .add_message::<AddUiMessage>()
      .configure_sets(EguiPrimaryContextPass, EditorUiSystems)
      .add_observer(RemoveUiEvent::on_event)
      .add_systems(Startup, Self::init_resources)
      .add_systems(OnEnter(EditorState::Exiting), Self::on_app_exit)
      .add_systems(First, Self::setup_ctx)
      .add_systems(FixedUpdate, AddUiMessage::handle)
      .add_systems(
        EguiPrimaryContextPass,
        (
          KeyboardFocus::set_state,
          (
            Self::dispatch_render_events,
            Self::reset_ui_state,
            Self::render,
          )
            .chain()
            .run_if(should_render_ui),
        )
          .in_set(EditorUiSystems),
      );

    prebuilt::menu_bar::init(app);
  }
}

impl UiPlugin {
  fn init_resources(world: &mut World) -> Result {
    world.spawn((Name::new("Editor UI Camera"), EditorUiCamera));
    world.spawn((Name::new("Editor Ui Panels"), UiPanels));
    world.resource_scope(|world, mut ui_manager: Mut<UiManager>| ui_manager.restore_or_init(world))
  }

  fn setup_ctx(
    mut q_ctx: Query<
      (
        &mut bevy_egui::EguiContext,
        &mut bevy_egui::EguiContextSettings,
      ),
      Added<PrimaryEguiContext>,
    >,
  ) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor_icons::add_fonts(&mut fonts);

    for (mut ctx, mut settings) in &mut q_ctx {
      let ctx = ctx.get_mut();
      ctx.set_fonts(fonts.clone());
      settings.capture_pointer_input = false;
    }
  }

  pub fn reset_ui_state(mut q_ui_infos: Query<&mut UiState>) {
    q_ui_infos.par_iter_mut().for_each(|mut ui_info| {
      ui_info.rendered = false;
    });
  }

  pub fn render(world: &mut World) {
    world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
      ui_manager.render(world);
    });
  }

  pub fn dispatch_render_events(world: &mut World) {
    let mut q_entities = world.query::<(Entity, &UiState)>();
    let (rendered, unrendered): (Vec<Entity>, Vec<Entity>) =
      q_entities.iter(world).partition_map(|(entity, ui_info)| {
        if ui_info.rendered {
          Either::Left(entity)
        } else {
          Either::Right(entity)
        }
      });

    world.resource_scope(|world, ui_manager: Mut<UiManager>| {
      for entity in rendered {
        let Some(vtable) = ui_manager.vtable_of(entity, world) else {
          continue;
        };
        (vtable.when_rendered)(entity, world);
      }

      for entity in unrendered {
        let Some(vtable) = ui_manager.vtable_of(entity, world) else {
          continue;
        };
        (vtable.when_not_rendered)(entity, world);
      }
    });
  }

  pub fn on_app_exit(
    ui_manager: Res<UiManager>,
    q_uuids: Query<&PersistentId, Without<MissingUi>>,
    q_missing: Query<&MissingUi>,
    mut params: ParamSet<(Settings, Layouts)>,
  ) -> Result {
    let current_layout = {
      let mut settings = params.p0();

      let save_on_exit = settings.get_or_default::<bool>(SaveLayoutOnExitSetting);
      if save_on_exit {
        let name = match settings.get::<String>(CurrentLayoutSetting).ok() {
          Some(opt) => opt,
          None => {
            let default_layout = String::from("default");
            settings.set(CurrentLayoutSetting, &default_layout)?;
            default_layout
          }
        };

        Some(name)
      } else {
        None
      }
    };

    if let Some(name) = current_layout {
      let mut layouts = params.p1();
      let new_state = ui_manager.save_current_layout(&q_uuids, &q_missing);
      layouts.save_layout(name, new_state)?;
    }

    Ok(())
  }
}

pub trait EditorUiBundle: Bundle + GetTypeRegistration + Send + Sync + Sized {
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
  #[allow(unused_variables)]
  fn init(app: &mut App) {}

  fn spawn(entity: Entity, world: &mut World) -> Self;

  #[allow(unused_variables)]
  fn on_despawn(entity: Entity, world: &mut World) {}

  #[allow(unused_variables)]
  fn title(entity: Entity, world: &mut World) -> egui::WidgetText {
    Self::NAME.into()
  }

  fn render(entity: Entity, ui: &mut egui::Ui, world: &mut World);

  #[allow(unused_variables)]
  fn when_rendered(entity: Entity, world: &mut World) {}

  #[allow(unused_variables)]
  fn when_not_rendered(entity: Entity, world: &mut World) {}

  #[allow(unused_variables)]
  fn context_menu(
    entity: Entity,
    ui: &mut egui::Ui,
    world: &mut World,
    surface: SurfaceIndex,
    node: NodeIndex,
  ) {
  }

  #[allow(unused_variables)]
  fn handle_tab_response(entity: Entity, world: &mut World, response: &egui::Response) {}
}

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
  #[allow(unused_variables)]
  fn init(app: &mut App) {}

  fn spawn(params: Self::Params<'_, '_>) -> Self;

  #[allow(unused_variables)]
  fn title(&mut self, params: Self::Params<'_, '_>) -> egui::WidgetText {
    <Self as EditorUi>::NAME.into()
  }

  fn render(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>);

  #[allow(unused_variables)]
  fn when_rendered(&mut self, params: Self::Params<'_, '_>) {}

  #[allow(unused_variables)]
  fn when_not_rendered(&mut self, params: Self::Params<'_, '_>) {}

  #[allow(unused_variables)]
  fn context_menu(
    &mut self,
    ui: &mut egui::Ui,
    params: Self::Params<'_, '_>,
    surface: SurfaceIndex,
    node: NodeIndex,
  ) {
  }

  #[allow(unused_variables)]
  fn handle_tab_response(&mut self, params: Self::Params<'_, '_>, response: &egui::Response) {}

  #[allow(unused_variables)]
  fn on_despawn(&mut self, params: Self::Params<'_, '_>) {}
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
    <Self as EditorUi>::init(app)
  }

  fn spawn(entity: Entity, world: &mut World) -> Self {
    Self::register_params(entity, world);
    Self::with_params(entity, world, EditorUi::spawn)
  }

  fn title(entity: Entity, world: &mut World) -> egui::WidgetText {
    Self::get_entity_mut(entity, world, EditorUi::title)
  }

  fn render(entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    Self::get_entity_mut(entity, world, |this, params| {
      this.render(ui, params);
    })
  }

  fn when_rendered(entity: Entity, world: &mut World) {
    Self::get_entity_mut(entity, world, <Self as EditorUi>::when_rendered)
  }

  fn when_not_rendered(entity: Entity, world: &mut World) {
    Self::get_entity_mut(entity, world, <Self as EditorUi>::when_not_rendered)
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

  fn on_despawn(entity: Entity, world: &mut World) {
    Self::get_entity_mut(entity, world, <Self as EditorUi>::on_despawn)
  }

  fn handle_tab_response(entity: Entity, world: &mut World, response: &egui::Response) {
    Self::get_entity_mut(entity, world, |this, params| {
      this.handle_tab_response(params, response);
    });
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
  when_rendered: fn(Entity, &mut World),
  when_not_rendered: fn(Entity, &mut World),
  context_menu: fn(Entity, &mut egui::Ui, &mut World, SurfaceIndex, NodeIndex),
  handle_tab_response: fn(Entity, &mut World, &egui::Response),
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
      render: T::render,
      when_rendered: T::when_rendered,
      when_not_rendered: T::when_not_rendered,
      context_menu: T::context_menu,
      handle_tab_response: T::handle_tab_response,
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
  vtables: &'a mut HashMap<PersistentId, VTable>,
}

impl TabViewer<'_> {
  fn vtable_of(&self, entity: Entity) -> VTable {
    let mut world = self.world.borrow_mut();
    let mut q_ids = world.query::<&PersistentId>();
    let id = q_ids.get(&world, entity).unwrap();
    self.vtables[id].clone()
  }

  fn ui_info(&self, entity: Entity, f: impl FnOnce(&mut UiState)) {
    let mut world = self.world.borrow_mut();
    let mut q_ids = world.query::<&mut UiState>();
    let ui_info = q_ids.get_mut(&mut world, entity).ok();
    if let Some(mut ui_info) = ui_info {
      f(&mut ui_info);
    }
  }
}

impl egui_dock::TabViewer for TabViewer<'_> {
  type Tab = Entity;

  fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
    let vtable = self.vtable_of(*tab);
    (vtable.title)(*tab, &mut self.world.borrow_mut())
  }

  #[profiling::function]
  fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
    let vtable = self.vtable_of(*tab);
    (vtable.render)(*tab, ui, &mut self.world.borrow_mut());

    self.ui_info(*tab, |ui_info| {
      ui_info.hovered = ui.ui_contains_pointer();
      ui_info.rendered = true;
    });
  }

  #[profiling::function]
  fn add_popup(&mut self, ui: &mut egui::Ui, surface: SurfaceIndex, node: NodeIndex) {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    let unique_tabs = self
      .vtables
      .iter()
      .filter(|(_, vtable)| vtable.unique && !vtable.hidden)
      .map(|(id, vtable)| (id, vtable.name))
      .sorted_by(|(_, a), (_, b)| a.cmp(b));

    for (id, name) in unique_tabs {
      let vtable = &self.vtables[id];
      let mut world = self.world.borrow_mut();
      let count = (vtable.count)(&mut world);

      let mut exists = count > 0;
      let enabled = !exists;

      ui.add_enabled_ui(enabled, |ui| {
        if ui.checkbox(&mut exists, name).clicked() {
          let entity = (vtable.spawn)(&mut world);
          world.write_message(AddUiMessage::new(surface, node, entity));
        }
      });
    }

    let spawnable_tables = self
      .vtables
      .iter()
      .filter(|(_, vtable)| !vtable.unique)
      .map(|(id, vtable)| (id, vtable.name))
      .sorted_by(|(_, a), (_, b)| a.cmp(b));

    if spawnable_tables.len() > 0 {
      for (id, name) in spawnable_tables {
        let vtable = &self.vtables[id];
        if ui.button(name).clicked() {
          let mut world = self.world.borrow_mut();
          let entity = (vtable.spawn)(&mut world);
          world.write_message(AddUiMessage::new(surface, node, entity));
        }
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
    let vtable = self.vtable_of(*tab);
    (vtable.context_menu)(*tab, ui, &mut self.world.borrow_mut(), surface, node);
  }

  fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &egui::Response) {
    let vtable = self.vtable_of(*tab);
    (vtable.handle_tab_response)(*tab, &mut self.world.borrow_mut(), response)
  }

  fn closeable(&mut self, tab: &mut Self::Tab) -> bool {
    let vtable = self.vtable_of(*tab);
    vtable.closeable
  }

  fn on_close(&mut self, tab: &mut Self::Tab) -> egui_dock::tab_viewer::OnCloseResponse {
    let vtable = self.vtable_of(*tab);
    (vtable.despawn)(*tab, &mut self.world.borrow_mut());
    egui_dock::tab_viewer::OnCloseResponse::Close
  }

  fn clear_background(&self, tab: &Self::Tab) -> bool {
    let vtable = self.vtable_of(*tab);
    vtable.can_clear
  }

  fn allowed_in_windows(&self, tab: &mut Self::Tab) -> bool {
    let vtable = self.vtable_of(*tab);
    vtable.popout
  }

  fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
    egui::Id::new(tab)
  }

  fn scroll_bars(&self, tab: &Self::Tab) -> [bool; 2] {
    let vtable = self.vtable_of(*tab);
    vtable.scroll_bars
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
  pub fn add_selected(&mut self, entity: Entity, add: bool) {
    if let InspectorSelection::Entities(selected_entities) = self {
      selected_entities.select_maybe_add(entity, add);
    } else {
      let mut selected_entities = SelectedEntities::default();
      selected_entities.select_replace(entity);
      *self = Self::Entities(selected_entities);
    }
  }
}

pub struct InspectorIntegrationPlugin<F: QueryFilter>(PhantomData<F>);

impl<F: QueryFilter> Default for InspectorIntegrationPlugin<F> {
  fn default() -> Self {
    Self(default())
  }
}

impl<F: QueryFilter + Send + Sync + 'static> Plugin for InspectorIntegrationPlugin<F> {
  fn build(&self, app: &mut App) {
    app.add_systems(
      FixedUpdate,
      (
        auto_register_picking_targets::<F>,
        handle_click_events::<F>.run_if(mouse_hovered_in_editor_view),
      ),
    );
  }
}

fn auto_register_picking_targets<F: QueryFilter>(
  mut commands: Commands,
  q_entities: Query<(Entity, Option<&Name>), (Without<Pickable>, F)>,
) {
  for (entity, name) in &q_entities {
    if let Some(name) = name {
      debug!("Registered picking on object: {name}");
    } else {
      debug!("Registered picking on entity: {entity}");
    }

    commands.entity(entity).insert(Pickable {
      is_hoverable: true,
      should_block_lower: true,
    });
  }
}

fn handle_click_events<F: QueryFilter>(
  mut events: MessageReader<Pointer<Click>>,
  mut selection: ResMut<InspectorSelection>,
  keyboard: Res<ButtonInput<KeyCode>>,
  q_pickables: Query<(), F>,
) {
  for event in events
    .read()
    .filter(|event| q_pickables.contains(event.event_target()))
  {
    selection.add_selected(
      event.event_target(),
      keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight),
    );
  }
}

#[derive(Default, Deref, DerefMut, Debug)]
pub struct SelectedEntities(bevy_inspector::hierarchy::SelectedEntities);

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

fn should_render_ui(editor_settings: Res<EditorSettings>) -> bool {
  editor_settings.render_ui
}
