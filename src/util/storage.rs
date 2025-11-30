use bevy::{ecs::system::SystemParam, prelude::*};
use derive_new::new;
use egui_dock::DockState;
use include_dir::{Dir, include_dir};
use parking_lot::Mutex;
use persistent_id::PersistentId;
use rusqlite::Connection;
use rusqlite_migration::Migrations;
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, path::PathBuf, sync::LazyLock};

use crate::{
  EditorSettings,
  util::log::LogLevel,
  view::{cam::ActiveEditorCamera, view2d, view3d},
};

static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations");

static MIGRATIONS: LazyLock<Migrations<'static>> =
  LazyLock::new(|| Migrations::from_directory(&MIGRATIONS_DIR).unwrap());

#[derive(Resource)]
pub struct Storage {
  db: Mutex<Connection>,
}

impl Storage {
  pub fn new() -> Result<Self> {
    let mut db = Connection::open(Self::path())?;

    MIGRATIONS.to_latest(&mut db)?;

    Ok(Self { db: Mutex::new(db) })
  }

  fn path() -> PathBuf {
    let current_exe = std::env::current_exe().expect("failed to get current executable");
    let stem = current_exe
      .file_stem()
      .expect("failed to get current executable file stem");

    let filename = format!("{}.{}.sqlite", stem.display(), env!("CARGO_PKG_NAME"));
    current_exe.parent().unwrap().to_path_buf().join(filename)
  }
}

#[derive(SystemParam, new)]
pub struct Settings<'w> {
  storage: ResMut<'w, Storage>,
}

impl Settings<'_> {
  pub fn set<S>(&mut self, value: impl Borrow<S::Type>) -> Result
  where
    S: Setting,
  {
    let serialized = ron::to_string(value.borrow())?;

    let key = S::field();

    self.storage.db.lock().execute(
      "INSERT INTO [settings]([key], [value]) VALUES(?1, ?2) ON CONFLICT([key]) DO UPDATE SET [value]=?2",
      (&key, &serialized),
    )?;

    debug!("Updated setting {key} to {serialized}");

    Ok(())
  }

  pub fn get<S>(&mut self) -> Result<S::Type>
  where
    S: Setting,
  {
    let key = S::field();

    let result: String = {
      self.storage.db.lock().query_one(
        "SELECT [value] FROM [settings] WHERE [key] == ?1",
        (&key,),
        |row| row.get(0).inspect_err(|err| error!("{err}")),
      )?
    };

    let value = ron::de::from_str(&result)?;

    debug!("Queried setting {key} = {result}");

    Ok(value)
  }

  pub fn get_or_default<S>(&mut self) -> S::Type
  where
    S: Setting,
    S::Type: Default,
  {
    self.get::<S>().unwrap_or_default()
  }
}

#[derive(SystemParam, new)]
pub struct Layouts<'w> {
  storage: ResMut<'w, Storage>,
}

impl Layouts<'_> {
  pub fn list(&mut self) -> Result<Vec<String>> {
    let db = self.storage.db.lock();
    let mut stmt = db.prepare("SELECT [name] FROM [layouts]")?;
    let names = stmt
      .query_map((), |row| row.get(0))?
      .filter_map(Result::ok)
      .filter(|name: &String| !name.is_empty())
      .collect::<Vec<String>>();
    Ok(names)
  }

  pub fn save_layout(&mut self, name: impl AsRef<str>, layout: DockState<LayoutInfo>) -> Result {
    let name = name.as_ref();
    let bytes = postcard::to_stdvec(&layout)?;
    self.storage.db.lock().execute(
      "INSERT INTO [layouts]([name], [data]) VALUES(?1, ?2) ON CONFLICT([name]) DO UPDATE SET [data]=?2",
      (name, &bytes))?;
    Ok(())
  }

  pub fn get_layout(&mut self, name: impl AsRef<str>) -> Result<DockState<LayoutInfo>> {
    let name = name.as_ref();

    let result: Vec<u8> = {
      self.storage.db.lock().query_one(
        "SELECT [data] FROM [layouts] WHERE [name] == ?1",
        [name],
        |row| row.get(0).inspect_err(|err| error!("{err}")),
      )?
    };

    let value = postcard::from_bytes(&result)?;

    Ok(value)
  }
}

pub trait SettingsGroup {
  const GROUP: &str;
}

pub trait SettingKey {
  type Type: Serialize + for<'de> Deserialize<'de>;
  type Group: SettingsGroup;
  const KEY: &str;
}

pub trait Setting {
  type Type: Serialize + for<'de> Deserialize<'de>;
  const GROUP: &str;
  const KEY: &str;

  fn field() -> String {
    format!("{}.{}", Self::GROUP, Self::KEY)
  }
}

impl<T> Setting for T
where
  Self: SettingKey,
{
  type Type = <Self as SettingKey>::Type;
  const GROUP: &str = <Self as SettingKey>::Group::GROUP;
  const KEY: &str = <Self as SettingKey>::KEY;
}

#[derive(Clone, Serialize, Deserialize, new)]
pub struct LayoutInfo {
  id: PersistentId,
  name: String,
}

impl LayoutInfo {
  pub fn id(&self) -> PersistentId {
    self.id
  }

  pub fn name(&self) -> &str {
    &self.name
  }
}

pub struct EditorSettingsGroup;

impl SettingsGroup for EditorSettingsGroup {
  const GROUP: &str = "editor";
}

pub struct EditorSettingsSetting;

impl SettingKey for EditorSettingsSetting {
  type Type = EditorSettings;
  type Group = EditorSettingsGroup;
  const KEY: &str = "settings";
}

pub struct StartEditorInTestingSetting;

impl SettingKey for StartEditorInTestingSetting {
  type Type = bool;
  type Group = EditorSettingsGroup;
  const KEY: &str = "start_in_testing";
}

pub struct WindowSettingsGroup;

impl SettingsGroup for WindowSettingsGroup {
  const GROUP: &str = "window";
}

pub struct WindowMaximizedSetting;

impl SettingKey for WindowMaximizedSetting {
  type Type = bool;
  type Group = WindowSettingsGroup;
  const KEY: &str = "maximized";
}

pub struct UiSettingsGroup;

impl SettingsGroup for UiSettingsGroup {
  const GROUP: &str = "ui";
}

pub struct SaveLayoutOnExitSetting;

impl SettingKey for SaveLayoutOnExitSetting {
  type Type = bool;
  type Group = UiSettingsGroup;
  const KEY: &str = "save_layout_on_exit";
}

pub struct CurrentLayoutSetting;

impl SettingKey for CurrentLayoutSetting {
  type Type = String;
  type Group = UiSettingsGroup;
  const KEY: &str = "current_layout";
}

pub struct LogSettingsGroup;

impl SettingsGroup for LogSettingsGroup {
  const GROUP: &str = "log";
}

pub struct LogLevelSetting;

impl SettingKey for LogLevelSetting {
  type Type = LogLevel;
  type Group = LogSettingsGroup;
  const KEY: &str = "level";
}

pub struct ViewSettingsGroup;

impl SettingsGroup for ViewSettingsGroup {
  const GROUP: &str = "view";
}

pub struct RenderCamerasSetting;

impl SettingKey for RenderCamerasSetting {
  type Type = bool;
  type Group = ViewSettingsGroup;
  const KEY: &str = "render_cameras";
}

pub struct ActiveEditorCameraSetting;

impl SettingKey for ActiveEditorCameraSetting {
  type Type = ActiveEditorCamera;
  type Group = ViewSettingsGroup;
  const KEY: &str = "active_editor_camera";
}

pub struct CamStateSetting2d;

impl SettingKey for CamStateSetting2d {
  type Type = view2d::CameraSaveData;
  type Group = ViewSettingsGroup;
  const KEY: &str = "2d_cam_state";
}

pub struct CamStateSetting3d;

impl SettingKey for CamStateSetting3d {
  type Type = view3d::CameraSaveData;
  type Group = ViewSettingsGroup;
  const KEY: &str = "3d_cam_state";
}
