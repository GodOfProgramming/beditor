use crate::{
  ui::{RawUi, components::horizontal_list},
  util::short_name_of_type,
};
use bevy::prelude::*;
use brefabs::Prefabs;
use uuid::{Uuid, uuid};

#[derive(Default, Component, Reflect)]
pub struct PrefabsUi;

impl RawUi for PrefabsUi {
  const NAME: &str = stringify!(Prefabs);
  const ID: Uuid = uuid!("fa977fad-ed99-4842-bab4-7c00641b39b0");

  fn spawn(_entity: Entity, _world: &mut World) -> Self {
    default()
  }

  fn unique() -> bool {
    true
  }

  fn render(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    world.resource_scope(|world, prefabs: Mut<Prefabs>| {
      let app_type_registry = world.resource::<AppTypeRegistry>().0.clone();
      let type_registry = app_type_registry.read();

      let iter = prefabs
        .iter_all_types()
        .filter_map(|(type_id, variants)| {
          type_registry
            .get(type_id)
            .map(|registration| (registration, variants))
        })
        .map(|(registration, variants)| {
          variants.map(|name| {
            let type_name = short_name_of_type(registration);
            match name {
              Some(name) => {
                format!("{type_name}#{name}")
              }
              None => String::from(type_name),
            }
          })
        })
        .flatten();

      horizontal_list(ui, 5, iter, |ui, _i, id| {
        ui.label(id);
      });
    });
  }
}
