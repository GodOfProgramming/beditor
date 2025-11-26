use crate::{assets, ui::RawUi};
use bevy::prelude::*;
use uuid::{Uuid, uuid};

#[derive(Default, Component, Reflect)]
pub struct Prefabs;

impl RawUi for Prefabs {
  const NAME: &str = stringify!(Prefabs);
  const ID: Uuid = uuid!("fa977fad-ed99-4842-bab4-7c00641b39b0");

  fn spawn(_entity: Entity, _world: &mut World) -> Self {
    default()
  }

  fn unique() -> bool {
    true
  }

  fn render(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    world.resource_scope(|world, mut prefabs: Mut<assets::Prefabs>| {
      let mut prefab_ids = prefabs.keys().cloned().collect::<Vec<_>>();

      prefab_ids.sort();

      for id in prefab_ids {
        ui.horizontal(|ui| {
          ui.label(&id);
          if ui.button("Spawn").clicked() {
            prefabs.spawn(id, world);
          }
        });
      }
    });
  }
}
