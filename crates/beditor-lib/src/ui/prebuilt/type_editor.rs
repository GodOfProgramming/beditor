use crate::{EditorUiBundle, ui::managers::UiManager, util::reflection};
use bevy::{platform::collections::HashMap, prelude::*, reflect::TypeRegistry};
use derive_new::new;
use egui_file_dialog::{DialogState, FileDialog};
use parking_lot::Mutex;
use std::{
  borrow::Cow,
  cell::RefCell,
  ffi::OsStr,
  io::Write,
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
      .add_message::<SaveFileMessage>()
      .add_message::<OpenFileMessage>()
      .init_resource::<SerdeRegistry>()
      .add_systems(
        FixedUpdate,
        (SaveFileMessage::handle, OpenFileMessage::handle),
      )
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

    let mut message = None;

    ui.horizontal(|ui| {
      ui.heading(&state.label);

      if can_open_file_dialog {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
          if ui.button("Save As").clicked() {
            state.dialog.save_file();
          }

          if let Some(opened_file) = &state.opened_file
            && ui.button("Save").clicked()
          {
            message = Some(SaveFileMessage {
              entity,
              file: opened_file.clone(),
            });
          }
        });
      }
    });

    ui.separator();

    let m = arc.lock();
    let mut value = m.borrow_mut();

    bevy_inspector_egui::bevy_inspector::ui_for_value(&mut **value, ui, world);

    if let Some(msg) = message {
      world.write_message(msg);
    }
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

type DeserializeFn = fn(bytes: &[u8], type_registry: &TypeRegistry) -> Result<Box<dyn Reflect>>;
type SerializeFn = fn(value: &dyn Reflect, type_registry: &TypeRegistry) -> Result<Vec<u8>>;

#[derive(Resource)]
pub struct SerdeRegistry {
  unknown: Option<SerdeVtable>,
  mapping: HashMap<Cow<'static, OsStr>, SerdeVtable>,
}

impl Default for SerdeRegistry {
  fn default() -> Self {
    Self {
      unknown: default(),
      mapping: default(),
    }
    .with_registration(
      OsStr::new("ron"),
      reflection::serde::ron_serializer,
      reflection::serde::ron_deserializer,
    )
  }
}

impl SerdeRegistry {
  pub fn with_unknown(mut self, ser: SerializeFn, de: DeserializeFn) -> Self {
    self.unknown = Some(SerdeVtable::new(ser, de));
    self
  }

  pub fn add_unknown(&mut self, ser: SerializeFn, de: DeserializeFn) -> &mut Self {
    self.unknown = Some(SerdeVtable::new(ser, de));
    self
  }

  pub fn with_registration(
    mut self,
    extension: impl Into<Cow<'static, OsStr>>,
    ser: SerializeFn,
    de: DeserializeFn,
  ) -> Self {
    self.add_registration(extension, ser, de);
    self
  }

  pub fn add_registration(
    &mut self,
    extension: impl Into<Cow<'static, OsStr>>,
    ser: SerializeFn,
    de: DeserializeFn,
  ) -> &mut Self {
    self
      .mapping
      .insert(extension.into(), SerdeVtable::new(ser, de));
    self
  }

  fn serializer_for(&self, path: &Path) -> Option<SerializeFn> {
    self.vtable_for(path).map(|vtable| vtable.ser)
  }

  fn deserializer_for(&self, path: &Path) -> Option<DeserializeFn> {
    self.vtable_for(path).map(|vtable| vtable.de)
  }

  fn vtable_for(&self, path: &Path) -> Option<&SerdeVtable> {
    if let Some(extension) = path.extension() {
      self.mapping.get(extension)
    } else {
      self.unknown.as_ref()
    }
  }
}

#[derive(new)]
struct SerdeVtable {
  ser: SerializeFn,
  de: DeserializeFn,
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
      match state.dialog.mode() {
        egui_file_dialog::DialogMode::PickFile => {
          commands.write_message(OpenFileMessage::new(entity, file.to_path_buf()));
        }
        egui_file_dialog::DialogMode::SaveFile => {
          commands.write_message(SaveFileMessage::new(entity, file.to_path_buf()));
        }
        _ => (),
      }
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
    loaders: Res<SerdeRegistry>,
    app_type_registry: Res<AppTypeRegistry>,
  ) -> Result {
    for msg in messages.read() {
      let Some(de) = loaders.deserializer_for(&msg.file) else {
        warn!(
          path = msg.file.display().to_string(),
          "No deserializer registered for file type"
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

      let value = (de)(&bytes, &type_registry)?;

      state.opened_file = Some(msg.file.clone());

      state.set_value(value);
    }

    Ok(())
  }
}

#[derive(new, Message)]
struct SaveFileMessage {
  entity: Entity,
  file: PathBuf,
}

impl SaveFileMessage {
  fn handle(
    mut messages: MessageReader<Self>,
    mut q_states: Query<&mut TypeEditorState>,
    loaders: Res<SerdeRegistry>,
    app_type_registry: Res<AppTypeRegistry>,
  ) -> Result {
    for msg in messages.read() {
      let Some(ser) = loaders.serializer_for(&msg.file) else {
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

      state.opened_file = Some(msg.file.clone());

      let Some(value) = &state.value else {
        warn!("Tried to save None value");
        continue;
      };

      let type_registry = app_type_registry.read();

      let value = value.lock();
      let value = value.borrow();
      let value = &**value;

      let bytes = (ser)(value, &type_registry)?;
      let path = msg.file.clone();

      let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;

      file.write_all(&bytes)?;
    }

    Ok(())
  }
}
