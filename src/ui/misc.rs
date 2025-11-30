use std::borrow::Borrow;

use crate::{EditorSettings, UiManager, util::storage::LayoutInfo};

use super::{EditorUi, EditorUiBundle, VTable};
use bevy::{
  camera::Viewport,
  ecs::{
    component::Mutable,
    system::{SystemParam, SystemState},
  },
  platform::collections::HashMap,
  prelude::*,
  window::PrimaryWindow,
};
use derive_more::derive::Deref;
use derive_new::new;
use egui::{emath::Numeric, text::LayoutJob};
use egui_dock::DockState;
use persistent_id::PersistentId;
use uuid::{Uuid, uuid};

#[derive(SystemParam)]
pub struct NoParams;

#[derive(Component, Default)]
pub struct UiState {
  pub(super) rendered: bool,
  pub(super) hovered: bool,
}

impl UiState {
  pub fn rendered(&self) -> bool {
    self.rendered
  }

  pub fn hovered(&self) -> bool {
    self.hovered
  }
}

pub(crate) trait EditorUiExtensions {
  const VTABLE: VTable;
}

impl<T> EditorUiExtensions for T
where
  T: EditorUiBundle,
{
  const VTABLE: VTable = VTable::new::<Self>();
}

type UiParams<'w, 's, T> = UiComponentState<<T as EditorUi>::Params<'w, 's>>;

/// # Safety
/// Cannot access the world mutably in the system params
/// Though it is on the user to not query for a mutable reference to themselves when they also have a self reference
pub unsafe trait UiExtensions: EditorUi {
  fn get_entity<T>(
    entity: Entity,
    world: &mut World,
    f: impl FnOnce(&Self, Self::Params<'_, '_>) -> T,
  ) -> T {
    let mut q = world.query::<(&Self, &mut UiParams<Self>)>();
    let world_cell = world.as_unsafe_world_cell();
    let Ok((this, mut params)) = q.get_mut(unsafe { world_cell.world_mut() }, entity) else {
      panic!("Failed to query {}", <Self as EditorUi>::NAME);
    };

    let items = params.get_mut(unsafe { world_cell.world_mut() });
    let result = f(this, items);
    unsafe { params.apply(world_cell.world_mut()) };
    result
  }

  fn get_entity_mut<T>(
    entity: Entity,
    world: &mut World,
    f: impl FnOnce(&mut Self, Self::Params<'_, '_>) -> T,
  ) -> T
  where
    Self: Component<Mutability = Mutable>,
  {
    let mut q = world.query::<(&mut Self, &mut UiParams<Self>)>();
    let world_cell = world.as_unsafe_world_cell();
    let Ok((mut this, mut params)) = q.get_mut(unsafe { world_cell.world_mut() }, entity) else {
      panic!("Failed to query {}", <Self as EditorUi>::NAME);
    };

    let items = params.get_mut(unsafe { world_cell.world_mut() });
    let result = f(this.as_mut(), items);
    unsafe { params.apply(world_cell.world_mut()) };
    result
  }

  fn register_params(entity: Entity, world: &mut World) {
    if !world.entity(entity).contains::<UiParams<Self>>() {
      let state = SystemState::<<Self as EditorUi>::Params<'_, '_>>::new(world);
      world.entity_mut(entity).insert(UiComponentState(state));
    }
  }

  fn with_params<T>(
    entity: Entity,
    world: &mut World,
    f: impl FnOnce(Self::Params<'_, '_>) -> T,
  ) -> T {
    let world_cell = world.as_unsafe_world_cell();
    let mut entity = unsafe { world_cell.world_mut() }.entity_mut(entity);
    let mut params = entity.get_mut::<UiParams<Self>>().unwrap();
    let params = params.get_mut(unsafe { world_cell.world_mut() });
    f(params)
  }
}

unsafe impl<T> UiExtensions for T where T: EditorUi {}

#[derive(Component, Deref, DerefMut)]
struct UiComponentState<P>(SystemState<P>)
where
  P: SystemParam + 'static;

#[derive(new, Resource, Deref, DerefMut)]
pub struct UiResourceState<P>(SystemState<P>)
where
  P: SystemParam + 'static;

#[derive(Component, Reflect, Default)]
pub struct MissingUi {
  message: String,
  id: PersistentId,
  name: String,
}

impl MissingUi {
  pub fn new(name: impl Into<String>, id: impl Into<PersistentId>) -> Self {
    let id = id.into();
    let name = name.into();
    Self {
      message: format!("Failed to find ui component {name} with uuid: {}", *id),
      id,
      name,
    }
  }
  pub fn id(&self) -> &PersistentId {
    &self.id
  }
}

impl EditorUi for MissingUi {
  const NAME: &str = "Missing Ui";
  const ID: Uuid = uuid!("d0f32ae1-2851-4bcd-a0c9-f83ae030d85f");

  type Params<'w, 's> = NoParams;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
  }

  const HIDDEN: bool = true;

  const UNIQUE: bool = true;

  fn render(&mut self, ui: &mut egui::Ui, _params: Self::Params<'_, '_>) {
    let mut job = LayoutJob::single_section(self.message.to_owned(), egui::TextFormat::default());
    job.wrap = egui::text::TextWrapping::default();
    ui.label(job);
  }
}

pub(super) trait DockExtensions:
  Borrow<DockState<Entity>> + From<DockState<Entity>>
{
  fn decouple(
    &self,
    ui_manager: &UiManager,
    q_persistent_ids: &Query<&PersistentId, Without<MissingUi>>,
    q_missing: &Query<&MissingUi>,
  ) -> DockState<LayoutInfo> {
    self.borrow().map_tabs(|tab| {
      let id;
      let name;

      if let Ok(missing_uuid) = q_missing.get(*tab) {
        id = *missing_uuid.id();
        name = missing_uuid.name.clone();
      } else {
        id = *q_persistent_ids.get(*tab).unwrap();
        name = ui_manager
          .get_vtable_by_id(&id)
          .map(|vt| vt.name.to_string())
          .unwrap_or_default();
      }

      LayoutInfo::new(id, name)
    })
  }

  fn restore(
    dock: &DockState<LayoutInfo>,
    vtables: &HashMap<PersistentId, VTable>,
    world: &mut World,
  ) -> Self {
    dock
      .filter_map_tabs(|layout_info| {
        let Some(vtable) = vtables.get(&layout_info.id()) else {
          let name = layout_info.name();
          let state = SystemState::<<MissingUi as EditorUi>::Params<'_, '_>>::new(world);

          warn!(
            "Failed to find ui component {name} with uuid {}",
            *layout_info.id()
          );

          let entity = world
            .spawn((
              Name::new(<MissingUi as EditorUiBundle>::NAME),
              MissingUi::new(name, layout_info.id()),
              PersistentId(<MissingUi as EditorUiBundle>::ID),
              UiState::default(),
              UiComponentState(state),
            ))
            .id();

          return Some(entity);
        };

        if vtable.reopen_on_startup {
          Some((vtable.spawn)(world))
        } else {
          None
        }
      })
      .into()
  }
}

impl DockExtensions for DockState<Entity> {}

pub(crate) trait ShrinkableViewport: Component + Sized {
  type Marker: Component;

  fn viewport(&self) -> egui::Rect;

  fn set_viewport(
    window: Single<&Window, With<PrimaryWindow>>,
    egui_settings: Single<&bevy_egui::EguiContextSettings>,
    q_views: Query<(&Self, &UiState)>,
    mut q_cameras: Query<&mut Camera, With<<Self as ShrinkableViewport>::Marker>>,
    editor_settings: Res<EditorSettings>,
  ) {
    if !editor_settings.render_ui {
      for mut camera in q_cameras {
        camera.viewport = None; // set viewport to max if editor ui is being hidden to reflect actual game views
      }
      return;
    }

    for (view, ui_info) in &q_views {
      if ui_info.rendered() {
        for mut camera in &mut q_cameras {
          let scale_factor = window.scale_factor() * egui_settings.scale_factor;

          let viewport = view.viewport();
          let viewport_pos = viewport.left_top().to_vec2() * scale_factor;
          let viewport_size = viewport.size() * scale_factor;

          let physical_position = UVec2::new(viewport_pos.x as u32, viewport_pos.y as u32);
          let physical_size = UVec2::new(viewport_size.x as u32, viewport_size.y as u32);

          // The desired viewport rectangle at its offset in "physical pixel space"
          let rect = physical_position + physical_size;

          let window_size = window.physical_size();
          // wgpu will panic if trying to set a viewport rect which has coordinates extending
          // past the size of the render target, i.e. the physical window in our case.
          // Typically this shouldn't happen- but during init and resizing etc. edge cases might occur.
          // Simply do nothing in those cases.
          if rect.x <= window_size.x && rect.y <= window_size.y {
            let depth = camera
              .viewport
              .as_ref()
              .map(|vp| vp.depth.clone())
              .unwrap_or(0.0..1.0);
            camera.viewport = Some(Viewport {
              physical_position,
              physical_size,
              depth,
            });
          }
        }
      }
    }
  }
}

pub fn apply_dock_style_to_egui_style(dock: &egui_dock::Style, es: &mut egui::Style) {
  let visuals = &mut es.visuals;
  let spacing = &mut es.spacing;

  visuals.panel_fill = dock.tab_bar.bg_fill;
  visuals.window_fill = dock.tab_bar.bg_fill;

  visuals.widgets.inactive.bg_fill = dock.tab.inactive.bg_fill;
  visuals.widgets.active.bg_fill = dock.tab.active.bg_fill;
  visuals.widgets.hovered.bg_fill = dock.tab.hovered.bg_fill;

  visuals.widgets.inactive.bg_stroke = egui::Stroke::new(
    visuals.widgets.inactive.bg_stroke.width.max(0.0),
    dock.tab.inactive.outline_color,
  );
  visuals.widgets.active.bg_stroke = egui::Stroke::new(
    visuals.widgets.active.bg_stroke.width.max(0.0),
    dock.tab.active.outline_color,
  );
  visuals.window_stroke = dock.tab.tab_body.stroke;

  visuals.override_text_color = Some(dock.tab.inactive.text_color);

  visuals.window_corner_radius = dock.tab.inactive.corner_radius;
  visuals.menu_corner_radius = visuals.window_corner_radius;
  visuals.widgets.inactive.corner_radius = visuals.window_corner_radius;
  visuals.widgets.active.corner_radius = visuals.window_corner_radius;
  visuals.widgets.hovered.corner_radius = visuals.window_corner_radius;

  spacing.window_margin = dock.tab.tab_body.inner_margin;
  spacing.item_spacing = egui::Vec2::new(
    dock.tab.spacing,
    dock.tab.tab_body.inner_margin.top.to_f64() as f32,
  );

  if dock.tab_bar.hline_color.a() != 0 {
    visuals.window_stroke.color = dock.tab_bar.hline_color;
  }
}
