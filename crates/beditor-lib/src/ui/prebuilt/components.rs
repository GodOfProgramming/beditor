use super::InspectorDnd;
use crate::{
  EditorUi,
  registry::components::{ComponentRegistry, RegisteredComponent},
  ui::components::{Card, horizontal_list},
  util::vfs::{VfsNode, VfsPath},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use std::marker::PhantomData;
use uuid::uuid;

#[derive(Component, Reflect)]
pub struct Components {
  components_per_row: usize,
}

impl Default for Components {
  fn default() -> Self {
    Self {
      components_per_row: 20,
    }
  }
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  component_registry: Res<'w, ComponentRegistry>,

  current_path: Local<'s, Option<VfsPath>>,

  filter: Local<'s, String>,

  _pd: PhantomData<&'s ()>,
}

impl EditorUi for Components {
  const NAME: &str = "Components";

  const ID: uuid::Uuid = uuid!("5b376389-2acf-4945-807b-94ee16c09088");

  const UNIQUE: bool = true;

  const SCROLL_BARS: [bool; 2] = [false, true];

  type Params<'w, 's> = Params<'w, 's>;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
  }

  fn render(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
    let Self::Params {
      component_registry,
      mut current_path,
      mut filter,
      ..
    } = params;

    let current_path = current_path.get_or_insert_with(|| component_registry.vfs().root().clone());

    ui.horizontal(|ui| {
      ui.text_edit_singleline(&mut *filter);

      if current_path.has_parent(component_registry.vfs())
        && ui
          .button(egui_phosphor_icons::icons::ARROW_U_UP_LEFT.regular())
          .clicked()
        && let Some(parent) = current_path.parent(component_registry.vfs())
      {
        *current_path = parent.clone();
      }
    });

    ui.label(current_path.display());

    let components = component_registry.vfs().iter(current_path).filter(|path| {
      filter.is_empty() || {
        path
          .basename()
          .to_lowercase()
          .contains(filter.to_lowercase().as_str())
      }
    });

    let mut next_path = None;
    let num_columns = self.components_per_row.max(1);

    horizontal_list(ui, num_columns, components, |ui, i, path| {
      let card_width = ui.available_width();
      let card_height = card_width;

      let Some(node) = component_registry.vfs().read(path) else {
        return;
      };

      match node {
        VfsNode::Dir => {
          if ui_for_dir(ui, (card_width, card_height), path.basename(), i) {
            next_path = Some(path.clone());
          }
        }
        VfsNode::Item { value } => {
          if let Some(component) = component_registry.get(value) {
            ui_for_item(ui, (card_width, card_height), path.basename(), component);
          }
        }
      }
    });

    if let Some(path) = next_path {
      *current_path = path;
    }
  }
}

fn ui_for_dir(ui: &mut egui::Ui, size: impl Into<egui::Vec2>, label: &str, i: usize) -> bool {
  let size = size.into();
  Card::new(size)
    .with_label(label)
    .show(ui, |ui| {
      ui.label(egui_phosphor_icons::icons::FOLDER.regular());

      ui.interact(ui.min_rect(), ui.id().with(i), egui::Sense::click())
    })
    .inner
    .on_hover_cursor(egui::CursorIcon::PointingHand)
    .double_clicked()
}

fn ui_for_item(
  ui: &mut egui::Ui,
  size: impl Into<egui::Vec2>,
  label: &str,
  component: &RegisteredComponent,
) {
  let size = size.into();
  let id = component.type_id();
  ui.dnd_drag_source(egui::Id::new(id), InspectorDnd::AddComponent(id), |ui| {
    Card::new(size).with_label(label).show(ui, |ui| {
      ui.label(egui_phosphor_icons::icons::PUZZLE_PIECE.regular());
    });
  });
}
