use super::{
  RawUi, TabViewer, VTable,
  misc::{DockExtensions, MissingUi, UiComponentExtensions},
  prebuilt::{
    assets::Assets, components::Components, debug::DebugMenu, editor_view::EditorView,
    hierarchy::Hierarchy, inspector::Inspector, menu_bar::MenuBar, prefabs::Prefabs,
    resources::Resources,
  },
};
use crate::{
  Settings,
  util::storage::{LayoutInfo, Layouts},
};
use bevy::{ecs::system::SystemState, platform::collections::HashMap, prelude::*};
use derive_new::new;
use egui_dock::{DockArea, DockState, NodeIndex, Surface, SurfaceIndex};
use persistent_id::PersistentId;
use std::{any::TypeId, cell::RefCell, collections::BTreeSet};

#[derive(Resource)]
pub(crate) struct UiManager {
  state: DockState<Entity>,

  vtables: HashMap<PersistentId, VTable>,

  id: egui::Id,
}

impl UiManager {
  pub fn new(app: &mut App) -> Self {
    let mut this = Self {
      state: DockState::new(Vec::new()),
      vtables: default(),
      id: egui::Id::new(TypeId::of::<Self>()),
    };

    this.register::<MissingUi>(app);

    this.register::<Assets>(app);
    this.register::<Components>(app);
    this.register::<DebugMenu>(app);
    this.register::<EditorView>(app);
    this.register::<Hierarchy>(app);
    this.register::<Inspector>(app);
    this.register::<MenuBar>(app);
    this.register::<Prefabs>(app);
    this.register::<Resources>(app);

    this
  }

  pub fn restore_or_init(&mut self, world: &mut World) -> Result {
    let mut sys_state = SystemState::<ParamSet<(Settings, Layouts)>>::new(world);
    let mut params = sys_state.get_mut(world);

    let current_layout_name = {
      let mut settings = params.p0();
      settings.get::<String>(CurrentLayoutSetting).ok()
    };

    let layouts = {
      let mut layouts = params.p1();
      BTreeSet::from_iter(layouts.list()?)
    };

    let dock = match current_layout_name {
      Some(name) => {
        let mut layouts = params.p1();
        let layout = layouts.get_layout(name)?;
        DockState::restore(&layout, &self.vtables, world)
      }
      None => self.default_dock_state(world),
    };

    self.state = dock;

    world.insert_resource(LayoutManager::new(layouts));

    Ok(())
  }

  pub fn register<T: RawUi>(&mut self, app: &mut App) {
    T::init(app);
    app.register_type::<T>();
    self.vtables.insert(PersistentId(T::ID), T::VTABLE);
  }

  pub fn render(&mut self, world: &mut World) {
    let Ok(ctx) = world
      .query::<&mut bevy_egui::EguiContext>()
      .single_mut(world)
      .map(|mut ctx| ctx.get_mut().clone())
    else {
      return;
    };

    egui::CentralPanel::default()
      .frame(
        egui::Frame::central_panel(&ctx.style())
          // this makes it so the egui dock panels all surround the window's edges
          .inner_margin(0)
          // this allows the game to be rendered behind egui
          .fill(egui::Color32::TRANSPARENT),
      )
      .show(&ctx, |ui| {
        let mut tab_viewer = TabViewer {
          vtables: &mut self.vtables,
          world: RefCell::new(world),
        };

        DockArea::new(&mut self.state)
          .id(self.id)
          .show_add_buttons(true)
          .show_add_popup(true)
          .show_inside(ui, &mut tab_viewer);
      });
  }

  pub fn save_current_layout(
    &self,
    q_uuids: &Query<&PersistentId, Without<MissingUi>>,
    q_missing: &Query<&MissingUi>,
  ) -> DockState<LayoutInfo> {
    self.state.decouple(self, q_uuids, q_missing)
  }

  pub fn surface_mut(&mut self, index: SurfaceIndex) -> Option<&mut Surface<Entity>> {
    self.state.get_surface_mut(index)
  }

  pub(crate) fn vtables(&self) -> &HashMap<PersistentId, VTable> {
    &self.vtables
  }

  pub(super) fn vtable_of(&self, entity: Entity, world: &mut World) -> Option<&VTable> {
    let mut q_ids = world.query::<&PersistentId>();
    let id = q_ids.get(world, entity).unwrap();
    self.get_vtable_by_id(id)
  }

  pub(super) fn get_vtable_by_id(&self, id: &PersistentId) -> Option<&VTable> {
    self.vtables.get(id)
  }

  pub(crate) fn switch_state(&mut self, new_state: DockState<Entity>, world: &mut World) {
    for entity in self.state.iter_all_tabs().map(|(_, entity)| *entity) {
      if let Some(vtable) = self.vtable_of(entity, world) {
        (vtable.despawn)(entity, world);
      } else {
        world.despawn(entity);
      }
    }
    self.state = new_state;
  }

  pub(crate) fn default_dock_state(&self, world: &mut World) -> DockState<Entity> {
    let mut state = DockState::new(vec![self.spawn_type::<EditorView>(world)]);

    let tree = state.main_surface_mut();

    let root = NodeIndex::root();

    let tabs = vec![self.spawn_type::<MenuBar>(world)];
    let [central_panel, _top_bar] = tree.split_above(root, 0.1, tabs);

    let tabs = vec![
      self.spawn_type::<Hierarchy>(world),
      self.spawn_type::<DebugMenu>(world),
    ];
    let [central_panel, _left_panel] = tree.split_left(central_panel, 1.0 / 6.0, tabs);

    let tabs = vec![self.spawn_type::<Inspector>(world)];
    let [central_panel, _right_panel] = tree.split_right(central_panel, 4.0 / 5.0, tabs);

    let tabs = vec![
      self.spawn_type::<Prefabs>(world),
      self.spawn_type::<Components>(world),
      self.spawn_type::<Resources>(world),
      self.spawn_type::<Assets>(world),
    ];
    tree.split_below(central_panel, 0.7, tabs);

    state
  }

  fn spawn_type<T: RawUi>(&self, world: &mut World) -> Entity {
    self.spawn(PersistentId(T::ID), world)
  }

  fn spawn(&self, id: PersistentId, world: &mut World) -> Entity {
    (self.vtables[&id].spawn)(world)
  }

  pub fn state(&self) -> &DockState<Entity> {
    &self.state
  }
}

#[derive(new, Resource, Default, Deref, DerefMut)]
pub struct LayoutManager(BTreeSet<String>);

pub struct SaveLayoutOnExitSetting;

impl AsRef<str> for SaveLayoutOnExitSetting {
  fn as_ref(&self) -> &str {
    "ui.save_layout_on_exit"
  }
}

pub struct CurrentLayoutSetting;

impl AsRef<str> for CurrentLayoutSetting {
  fn as_ref(&self) -> &str {
    "ui.current_layout"
  }
}
