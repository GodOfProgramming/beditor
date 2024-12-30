use std::marker::PhantomData;

use super::{InspectorSelection, Ui};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::egui;
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct Resources;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  type_registry: Res<'w, AppTypeRegistry>,
  selection: ResMut<'w, InspectorSelection>,

  #[system_param(ignore)]
  _pd: PhantomData<&'s ()>,
}

impl Ui for Resources {
  const NAME: &str = stringify!(Resources);
  const UUID: uuid::Uuid = uuid!("54248a54-9544-4e93-9382-3677b8722952");

  type Params<'w, 's> = Params<'w, 's>;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
  }

  fn closeable(&mut self, _params: Self::Params<'_, '_>) -> bool {
    false
  }

  fn render(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
    let type_registry = params.type_registry.read();

    let mut resources: Vec<_> = type_registry
      .iter()
      .filter(|registration| registration.data::<ReflectResource>().is_some())
      .map(|registration| {
        (
          registration.type_info().type_path_table().short_path(),
          registration.type_id(),
        )
      })
      .collect();
    resources.sort_by(|(name_a, _), (name_b, _)| name_a.cmp(name_b));

    for (resource_name, type_id) in resources {
      let selected = match *params.selection {
        InspectorSelection::Resource(selected, _) => selected == type_id,
        _ => false,
      };

      if ui.selectable_label(selected, resource_name).clicked() {
        *params.selection = InspectorSelection::Resource(type_id, resource_name.to_string());
      }
    }
  }
}
