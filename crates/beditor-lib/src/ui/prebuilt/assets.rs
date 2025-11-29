use crate::ui::{InspectorSelection, Ui};
use bevy::{asset::ReflectAsset, ecs::system::SystemParam, prelude::*};
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct Assets;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
  set: ParamSet<'w, 's, (&'w World, ResMut<'w, InspectorSelection>)>,
  filter: Local<'s, String>,
}

impl Ui for Assets {
  const NAME: &str = stringify!(Assets);
  const ID: uuid::Uuid = uuid!("4bfee754-f9bc-4695-b215-2a88d9377dfb");

  const UNIQUE: bool = true;

  type Params<'w, 's> = Params<'w, 's>;

  fn spawn(_params: Self::Params<'_, '_>) -> Self {
    default()
  }

  fn render(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
    let world = params.set.p0();
    let type_registry = world.resource::<AppTypeRegistry>().0.clone();
    let type_registry = type_registry.read();

    let mut assets = type_registry
      .iter()
      .filter_map(|registration| {
        let reflect_asset = registration.data::<ReflectAsset>()?;
        let name = registration.type_info().type_path_table().short_path();
        (params.filter.is_empty() || name.to_lowercase().contains(params.filter.as_str()))
          .then(|| (name, registration.type_id(), reflect_asset))
      })
      .collect::<Vec<_>>();

    assets.sort_by(|(name_a, ..), (name_b, ..)| name_a.cmp(name_b));

    let mut selection = None;
    let current_selection = world.resource::<InspectorSelection>();

    ui.text_edit_singleline(&mut *params.filter).changed();

    for (asset_name, asset_type_id, reflect_asset) in assets {
      let handles = reflect_asset.ids(world).collect::<Vec<_>>();

      ui.collapsing(format!("{asset_name} ({})", handles.len()), |ui| {
        for handle in handles {
          let selected = match current_selection {
            InspectorSelection::Asset(_, _, selected_id) => *selected_id == handle,
            _ => false,
          };

          if ui
            .selectable_label(selected, format!("{:?}", handle))
            .clicked()
          {
            selection = Some(InspectorSelection::Asset(
              asset_type_id,
              asset_name.to_string(),
              handle,
            ));
          }
        }
      });
    }

    if let Some(selection) = selection {
      *params.set.p1() = selection;
    }
  }
}
