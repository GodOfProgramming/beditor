use bevy::prelude::*;
use std::{
  any::{Any, TypeId},
  sync::Arc,
};

pub mod assets;
pub mod components;
pub mod debug;
pub mod editor_view;
pub mod game_view;
pub mod hierarchy;
pub mod inspector;
pub mod logs;
pub mod menu_bar;
pub mod prefabs;
pub mod resources;

pub enum InspectorDnd {
  AddComponent(TypeId),
}

pub enum HierarchyDnd {
  AddPrefab(TypeId, Option<Name>),
}

fn dnd_drop_ui<P, R>(
  ui: &mut egui::Ui,
  render_fn: impl FnOnce(&mut egui::Ui) -> R,
) -> (egui::InnerResponse<R>, Option<Arc<P>>)
where
  P: Any + Send + Sync,
{
  // makes the whole pane droppable
  let frame = egui::Frame::default();
  let available_size = ui.available_size();

  // fixes weird highlighting on background
  let bg_fill = ui.style().visuals.window_fill();
  ui.style_mut().visuals.widgets.inactive.bg_fill = bg_fill;

  ui.dnd_drop_zone::<P, R>(frame, |ui| {
    ui.set_min_size(available_size);
    render_fn(ui)
  })
}
