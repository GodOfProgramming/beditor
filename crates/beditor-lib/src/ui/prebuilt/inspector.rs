use std::any::TypeId;

use crate::{
  registry::components::ComponentRegistry,
  ui::{InspectorSelection, RawUi},
};
use bevy::prelude::*;
use bevy_inspector_egui::bevy_inspector::{
  by_type_id::{ui_for_asset, ui_for_resource},
  ui_for_entities_shared_components, ui_for_entity,
};
use uuid::{Uuid, uuid};

use super::InspectorDnd;

#[derive(Default, Component, Reflect)]
pub struct Inspector;

impl Inspector {
  fn dnd_drop_ui<F>(
    entities: impl AsRef<[Entity]>,
    world: &mut World,
    ui: &mut egui::Ui,
    render_fn: F,
  ) where
    F: FnOnce(&mut World, &mut egui::Ui),
  {
    // makes the whole pane droppable
    let frame = egui::Frame::default();
    let available_size = ui.available_size();

    // fixes weird highlighting on background
    let bg_fill = ui.style().visuals.window_fill();
    ui.style_mut().visuals.widgets.inactive.bg_fill = bg_fill;

    let (_, component_id) = ui.dnd_drop_zone::<InspectorDnd, ()>(frame, |ui| {
      ui.set_min_size(available_size);
      render_fn(world, ui);
    });

    if let Some(dnd) = component_id {
      match &*dnd {
        InspectorDnd::AddComponent(type_id) => {
          Self::spawn_component_on(type_id, entities.as_ref(), world);
        }
        InspectorDnd::Custom(f) => {
          (f)(world, entities.as_ref());
        }
      }
    }
  }

  fn spawn_component_on(component_id: &TypeId, entities: &[Entity], world: &mut World) {
    let cr = world.resource::<ComponentRegistry>();
    let Some(component) = cr.get(component_id).cloned() else {
      warn!("Failed to lookup component");
      return;
    };

    let component_id = component.id();

    for entity in entities {
      if world.get_by_id(*entity, component_id).is_none() {
        component.spawn(*entity, world);
      }
    }
  }
}

impl RawUi for Inspector {
  const NAME: &str = stringify!(Inspector);
  const ID: Uuid = uuid!("10bb68b8-c247-4792-89e9-61d1b9682a72");

  fn spawn(_entity: Entity, _world: &mut World) -> Self {
    default()
  }

  fn unique() -> bool {
    true
  }

  fn render(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    let type_registry = world.resource::<AppTypeRegistry>().0.clone();
    let type_registry = type_registry.read();

    world.resource_scope(
      |world, selection: Mut<InspectorSelection>| match selection.as_ref() {
        InspectorSelection::Entities(selected_entities) => match selected_entities.as_slice() {
          &[entity] => {
            Self::dnd_drop_ui([entity], world, ui, |world, ui| {
              ui_for_entity(world, entity, ui);
            });
          }
          entities => {
            Self::dnd_drop_ui(entities, world, ui, |world, ui| {
              ui_for_entities_shared_components(world, entities, ui);
            });
          }
        },
        InspectorSelection::Resource(type_id, name) => {
          ui.label(name);
          ui_for_resource(world, *type_id, ui, name, &type_registry)
        }
        InspectorSelection::Asset(type_id, name, handle) => {
          ui.label(name);
          ui_for_asset(world, *type_id, *handle, ui, &type_registry);
        }
      },
    );
  }
}
