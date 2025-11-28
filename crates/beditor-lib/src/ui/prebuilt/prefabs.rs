use crate::{
  ui::{
    RawUi,
    components::{Card, horizontal_list},
    prebuilt::HierarchyDnd,
  },
  util::{
    short_name_of_type,
    vfs::{Vfs, VfsDir, VfsNode, VfsPath},
  },
};
use bevy::prelude::*;
use brefabs::Prefabs;
use itertools::Itertools;
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
      .add_systems(FixedUpdate, rebuild_vfs.run_if(resource_changed::<Prefabs>));
  }

  fn spawn(_entity: Entity, _world: &mut World) -> Self {
    default()
  }

  fn unique() -> bool {
    true
  }

  fn render(_entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    world.resource_scope(|_, mut vfs_state: Mut<PrefabVfsState>| {
      ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut vfs_state.filter);

        if vfs_state.current_path.has_parent()
          && ui
            .button(egui_phosphor_icons::icons::ARROW_U_UP_LEFT.regular())
            .clicked()
          && let Some(parent) = vfs_state.current_path.parent()
        {
          vfs_state.current_path = parent;
          vfs_state.current_dir = None;
        }
      });

      let full_path = vfs_state.current_path.iter().join("/");
      ui.label(full_path);

      if vfs_state.current_dir.is_none() {
        vfs_state.set_cur_dir();
      }

      let PrefabVfsState {
        current_dir,
        current_path,
        filter,
        ..
      } = &mut *vfs_state;

      let Some(dir) = &current_dir else {
        return;
      };

      let prefabs = dir.iter().filter(|node| {
        filter.is_empty() || {
          node
            .name()
            .to_lowercase()
            .contains(filter.to_lowercase().as_str())
        }
      });

      let mut clicked = false;

      horizontal_list(ui, 20, prefabs, |ui, i, node| {
        let card_width = ui.available_width();
        let card_height = card_width;

        match node {
          VfsNode::Dir(dir) => {
            clicked |= ui_for_dir(current_path, ui, (card_width, card_height), dir, i);
          }
          VfsNode::Item { name, value } => {
            ui_for_item(ui, (card_width, card_height), name, value);
          }
        }
      });

      if clicked {
        *current_dir = None;
      }
    });
  }
}

#[derive(Resource, Default, Deref, DerefMut)]
struct PrefabVfsState {
  #[deref]
  vfs: Vfs<PrefabData>,
  current_dir: Option<VfsDir<PrefabData>>,
  current_path: VfsPath,
  filter: String,
}

impl PrefabVfsState {
  fn set_cur_dir(&mut self) {
    self.current_dir = self.vfs.get_dir(&self.current_path).cloned();
  }
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
      let Some(module_path) = type_registration
        .type_info()
        .type_path_table()
        .module_path()
      else {
        warn!("Static prefab has no module path. It will not be found in the editor.");
        continue;
      };

      let path = Vec::from_iter(module_path.split("::").map(|p| Cow::Borrowed(p)));

      let type_name = short_name_of_type(type_registration);

      let name = match variant {
        Some(name) => Cow::Owned(format!("{type_name}#{name}")),
        None => Cow::Borrowed(type_name),
      };

      let path: VfsPath = path.into();
      let dir = vfs.open(path);
      dir.add_item(
        name,
        PrefabData {
          type_id,
          variant: variant.clone(),
        },
      );
    }
  }

  prefab_vfs.vfs = vfs;
}

fn ui_for_dir(
  current_path: &mut VfsPath,
  ui: &mut egui::Ui,
  size: impl Into<egui::Vec2>,
  label: &str,
  i: usize,
) -> bool {
  let size = size.into();
  let response = Card::new(size)
    .with_label(label)
    .show(ui, |ui| {
      ui.label(egui_phosphor_icons::icons::FOLDER.regular());

      ui.interact(ui.min_rect(), ui.id().with(i), egui::Sense::click())
    })
    .inner
    .on_hover_cursor(egui::CursorIcon::PointingHand);

  if response.double_clicked() {
    current_path.push(String::from(label));
    true
  } else {
    false
  }
}

fn ui_for_item(
  ui: &mut egui::Ui,
  size: impl Into<egui::Vec2>,
  label: &str,
  prefab_data: &PrefabData,
) {
  let size = size.into();

  ui.dnd_drag_source(
    egui::Id::new(&label),
    HierarchyDnd::AddPrefab(prefab_data.type_id, prefab_data.variant.clone()),
    |ui| {
      Card::new(size).with_label(label).show(ui, |ui| {
        ui.label(egui_phosphor_icons::icons::PUZZLE_PIECE.regular());
      });
    },
  );
}
