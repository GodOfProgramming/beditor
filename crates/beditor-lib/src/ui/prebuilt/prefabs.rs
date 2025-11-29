use crate::{
  ui::{
    RawUi,
    components::{Card, horizontal_list},
    prebuilt::{HierarchyDnd, type_editor::OpenTypeEditor},
  },
  util::{
    short_name_of_type,
    vfs::{Vfs, VfsNode, VfsPath},
  },
};
use bevy::prelude::*;
use brefabs::{Prefabs, WorldExtensions};
use std::{any::TypeId, borrow::Cow};
use uuid::{Uuid, uuid};

#[derive(Default, Component, Reflect)]
pub struct PrefabsUi;

impl RawUi for PrefabsUi {
  const NAME: &str = stringify!(Prefabs);
  const ID: Uuid = uuid!("fa977fad-ed99-4842-bab4-7c00641b39b0");

  fn init(app: &mut App) {
    app
      .init_resource::<PrefabVfsState>()
      .add_message::<EditPrefabDescriptorMessage>()
      .add_systems(
        FixedUpdate,
        (
          EditPrefabDescriptorMessage::handle,
          rebuild_vfs.run_if(resource_changed::<Prefabs>),
        ),
      );
  }

  fn spawn(_entity: Entity, _world: &mut World) -> Self {
    default()
  }

  fn unique() -> bool {
    true
  }

  fn render(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    world.resource_scope(|world, mut vfs_state: Mut<PrefabVfsState>| {
      let PrefabVfsState {
        vfs,
        current_path,
        filter,
      } = vfs_state.as_mut();

      let current_path = current_path.get_or_insert_with(|| vfs.root().clone());

      ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut *filter);

        if current_path.has_parent(vfs)
          && ui
            .button(egui_phosphor_icons::icons::ARROW_U_UP_LEFT.regular())
            .clicked()
          && let Some(parent) = current_path.parent(vfs)
        {
          *current_path = parent.clone();
        }
      });

      ui.label(current_path.full_path());

      let prefabs = vfs.iter(current_path).filter(|path| {
        filter.is_empty() || {
          path
            .basename()
            .to_lowercase()
            .contains(filter.to_lowercase().as_str())
        }
      });

      let mut next_path = None;
      let num_columns = 20;

      horizontal_list(ui, num_columns, prefabs, |ui, i, path| {
        let card_width = ui.available_width();
        let card_height = card_width;

        let Some(node) = vfs.read(path) else {
          return;
        };

        match node {
          VfsNode::Dir => {
            if ui_for_dir(ui, (card_width, card_height), path.basename(), i) {
              next_path = Some(path.clone());
            }
          }
          VfsNode::Item { value } => {
            ui_for_item(ui, (card_width, card_height), path, value, world);
          }
        }
      });

      if let Some(path) = next_path {
        *current_path = path;
      }
    });
  }
}

#[derive(Resource, Default, Deref, DerefMut)]
struct PrefabVfsState {
  #[deref]
  vfs: Vfs<PrefabData>,
  current_path: Option<VfsPath>,
  filter: String,
}

#[derive(Clone)]
struct PrefabData {
  type_id: TypeId,
  variant: Option<Name>,
}

fn rebuild_vfs(
  mut prefab_vfs: ResMut<PrefabVfsState>,
  prefabs: Res<Prefabs>,
  app_type_regsitry: Res<AppTypeRegistry>,
) {
  let mut vfs = Vfs::default();

  let type_registry = app_type_regsitry.0.read();
  for (type_id, variants) in prefabs.iter() {
    let Some(type_registration) = type_registry.get(type_id) else {
      warn!("Failed to get type registration for prefab. It will not be found in the editor.");
      continue;
    };

    for variant in variants.map(|(variant, _)| variant) {
      let type_name = short_name_of_type(type_registration);
      let Some(module_path) = type_registration
        .type_info()
        .type_path_table()
        .module_path()
      else {
        unreachable!("Every type should have a module path");
      };

      let name = match variant {
        Some(name) => Cow::Owned(format!("{type_name}#{name}")),
        None => Cow::Borrowed(type_name),
      };

      let Some(path) = vfs.mkdir_p(module_path.split("::"), true) else {
        error!(type_name, "Already registered ");
        return;
      };

      vfs.new_item(
        path,
        Name::new(name),
        PrefabData {
          type_id,
          variant: variant.clone(),
        },
      );
    }
  }

  prefab_vfs.vfs = vfs;
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
  path: &VfsPath,
  prefab_data: &PrefabData,
  world: &mut World,
) {
  let size = size.into();

  let id = egui::Id::new(path.full_path());

  let response = ui
    .dnd_drag_source(
      id,
      HierarchyDnd::AddPrefab(prefab_data.type_id, prefab_data.variant.clone()),
      |ui| {
        Card::new(size).with_label(path.basename()).show(ui, |ui| {
          ui.label(egui_phosphor_icons::icons::CUBE.regular());
        });
      },
    )
    .response;

  let response = ui.interact(response.rect, id, egui::Sense::click());

  response.context_menu(|ui| {
    if ui.button("Edit").clicked() {
      world.write_message(EditPrefabDescriptorMessage(
        prefab_data.type_id,
        prefab_data.variant.clone(),
      ));
    }
  });
}

#[derive(Message)]
struct EditPrefabDescriptorMessage(TypeId, Option<Name>);

impl EditPrefabDescriptorMessage {
  fn handle(mut messages: MessageReader<Self>, mut commands: Commands) {
    for msg in messages.read() {
      let type_id = msg.0.clone();
      let variant = msg.1.clone();

      commands.queue(move |world: &mut World| {
        if let Some(desc) = world.spawn_prefab_descriptor(type_id, variant) {
          world.commands().queue(OpenTypeEditor::new(desc));
        }
      });
    }
  }
}
