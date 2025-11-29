use crate::{EditorUiBundle, ui::managers::UiManager, util::reflection};
use bevy::{
  platform::collections::HashMap,
  prelude::*,
  reflect::{TypeRegistry, serde::TypedReflectDeserializer},
};
use derive_new::new;
use egui_file_dialog::{DialogState, FileDialog};
use parking_lot::Mutex;
use std::{
  any::TypeId,
  borrow::Cow,
  cell::RefCell,
  ffi::OsStr,
  path::{Path, PathBuf},
  sync::Arc,
};
use uuid::{Uuid, uuid};

#[derive(Bundle, Reflect, Default)]
pub struct TypeEditor {
  state: TypeEditorState,
  _marker: TypeEditorMarker,
}

#[derive(Component, Reflect, Default)]
pub struct TypeEditorMarker;

impl EditorUiBundle for TypeEditor {
  type PrimaryComponent = TypeEditorMarker;

  const NAME: &str = stringify!(TypeEditor);

  const ID: Uuid = uuid!("2b01d041-d8b3-4cbe-8ca7-f6ae8e8ef7dd");

  const REOPEN_ON_STARTUP: bool = false;

  fn init(app: &mut App) {
    app
      .add_message::<OpenFileMessage>()
      .init_resource::<TypeLoaders>()
      .add_systems(FixedUpdate, OpenFileMessage::handle)
      .add_systems(bevy_egui::EguiPrimaryContextPass, show_dialogs);
  }

  fn spawn(_entity: Entity, _world: &mut World) -> Self {
    default()
  }

  fn render(entity: Entity, ui: &mut egui::Ui, world: &mut World) {
    let mut entity_mut = world.entity_mut(entity);
    let Some(mut state) = entity_mut.get_mut::<TypeEditorState>() else {
      return;
    };

    let can_open_file_dialog = matches!(
      state.dialog.state(),
      DialogState::Closed | DialogState::Cancelled
    );

    let Some(arc) = state.value.as_ref().map(Arc::clone) else {
      if can_open_file_dialog && ui.button("Open").clicked() {
        state.dialog.pick_file();
      }
      return;
    };

    ui.horizontal(|ui| {
      ui.heading(&state.label);

      if can_open_file_dialog {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
          if ui.button("Save As").clicked() {
            state.dialog.save_file();
          }

          if let Some(_opened_file) = &state.opened_file
            && ui.button("Save").clicked()
          {
            // TODO save file
          }
        });
      }
    });

    ui.separator();

    let m = arc.lock();
    let mut value = m.borrow_mut();

    bevy_inspector_egui::bevy_inspector::ui_for_value(&mut **value, ui, world);
  }
}

#[derive(Component, Reflect, Default)]
struct TypeEditorState {
  label: String,

  opened_file: Option<PathBuf>,

  #[reflect(ignore)]
  value: Option<Arc<Mutex<RefCell<Box<dyn Reflect>>>>>,

  #[reflect(ignore)]
  dialog: FileDialog,
}

impl TypeEditorState {
  fn new(label: String, value: Box<dyn Reflect>) -> Self {
    Self {
      label,
      opened_file: None,
      dialog: FileDialog::default(),
      value: Some(Arc::new(Mutex::new(RefCell::new(value)))),
    }
  }

  fn set_value(&mut self, value: Box<dyn Reflect>) {
    self.value = Some(Arc::new(Mutex::new(RefCell::new(value))));
  }
}

#[derive(new, Message)]
pub struct OpenTypeEditor(String, Box<dyn Reflect>);

impl Command for OpenTypeEditor {
  fn apply(self, world: &mut World) {
    world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
      let entity = ui_manager.spawn_type::<TypeEditor>(world);
      world
        .entity_mut(entity)
        .insert(TypeEditorState::new(self.0, self.1));
      ui_manager.add_tab_to_focused(entity);
    });
  }
}

type LoadFn = fn(bytes: &[u8], type_registry: &TypeRegistry) -> Result<Box<dyn Reflect>>;
type SaveFn = fn(value: &dyn Reflect, type_id: TypeId, type_registry: &TypeRegistry) -> Result;

#[derive(Resource)]
pub struct TypeLoaders {
  unknown: Option<IoVtable>,
  mapping: HashMap<Cow<'static, OsStr>, IoVtable>,
}

impl Default for TypeLoaders {
  fn default() -> Self {
    Self {
      unknown: default(),
      mapping: default(),
    }
    .with_registration(OsStr::new("ron"), ron_loader, ron_saver)
  }
}

impl TypeLoaders {
  pub fn with_unknown(mut self, loader: LoadFn, saver: SaveFn) -> Self {
    self.unknown = Some(IoVtable::new(loader, saver));
    self
  }

  pub fn add_unknown(&mut self, loader: LoadFn, saver: SaveFn) -> &mut Self {
    self.unknown = Some(IoVtable::new(loader, saver));
    self
  }

  pub fn with_registration(
    mut self,
    extension: impl Into<Cow<'static, OsStr>>,
    loader: LoadFn,
    saver: SaveFn,
  ) -> Self {
    self.add_registration(extension, loader, saver);
    self
  }

  pub fn add_registration(
    &mut self,
    extension: impl Into<Cow<'static, OsStr>>,
    loader: LoadFn,
    saver: SaveFn,
  ) -> &mut Self {
    self
      .mapping
      .insert(extension.into(), IoVtable::new(loader, saver));
    self
  }

  fn loader_for(&self, path: &Path) -> Option<LoadFn> {
    self.vtable_for(path).map(|vtable| vtable.load)
  }

  fn saver_for(&self, path: &Path) -> Option<SaveFn> {
    self.vtable_for(path).map(|vtable| vtable.save)
  }

  fn vtable_for(&self, path: &Path) -> Option<&IoVtable> {
    if let Some(extension) = path.extension() {
      self.mapping.get(extension)
    } else {
      self.unknown.as_ref()
    }
  }
}

#[derive(new)]
struct IoVtable {
  load: LoadFn,
  save: SaveFn,
}

fn show_dialogs(
  mut commands: Commands,
  mut q_states: Query<(Entity, &mut TypeEditorState)>,
  mut contexts: bevy_egui::EguiContexts,
) {
  let Ok(ctx) = contexts.ctx_mut() else {
    return;
  };

  for (entity, mut state) in &mut q_states {
    state.dialog.update(ctx);
    if let Some(file) = state.dialog.take_picked() {
      commands.write_message(OpenFileMessage::new(entity, file.to_path_buf()));
    }
  }
}

#[derive(new, Message)]
struct OpenFileMessage {
  entity: Entity,
  file: PathBuf,
}

impl OpenFileMessage {
  fn handle(
    mut messages: MessageReader<Self>,
    mut q_states: Query<&mut TypeEditorState>,
    loaders: Res<TypeLoaders>,
    app_type_registry: Res<AppTypeRegistry>,
  ) -> Result {
    for msg in messages.read() {
      let Some(loader) = loaders.loader_for(&msg.file) else {
        warn!(
          path = msg.file.display().to_string(),
          "No loader registered for file type"
        );
        continue;
      };

      let Ok(mut state) = q_states.get_mut(msg.entity) else {
        warn!(
          entity = msg.entity.to_string(),
          "Failed to get type editor state for entity"
        );

        continue;
      };

      let type_registry = app_type_registry.read();

      let bytes = std::fs::read(&msg.file)?;

      let value = (loader)(&bytes, &type_registry)?;

      state.opened_file = Some(msg.file.clone());

      state.set_value(value);
    }

    Ok(())
  }
}

fn ron_loader(bytes: &[u8], type_registry: &TypeRegistry) -> Result<Box<dyn Reflect>> {
  use serde::de::DeserializeSeed;
  // have to use short names until this is resolved https://github.com/ron-rs/ron/issues/302

  let Some(output) = reflection::ron::newtype_name(bytes) else {
    return Err(String::from("Name of ron struct not found"))?;
  };

  let Some(registration) = type_registry.get_with_short_type_path(&output) else {
    return Err(format!("Type registration of '{output}' not found"))?;
  };

  let reflect_de = TypedReflectDeserializer::new(registration, type_registry);
  let mut ron_de = ron::Deserializer::from_bytes(bytes)?;

  let partial_reflect = reflect_de.deserialize(&mut ron_de)?;

  let Ok(reflect) = partial_reflect.try_into_reflect() else {
    return Err(format!("'{output}' is not Reflect"))?;
  };

  Ok(reflect)
}

fn ron_saver(value: &dyn Reflect, type_id: TypeId, type_registry: &TypeRegistry) -> Result {
  Ok(())
}
