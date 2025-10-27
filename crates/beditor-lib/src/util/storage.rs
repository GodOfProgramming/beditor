use bevy::{ecs::system::SystemParam, prelude::*};
use derive_new::new;
use egui_dock::DockState;
use include_dir::{Dir, include_dir};
use parking_lot::Mutex;
use persistent_id::PersistentId;
use rusqlite::Connection;
use rusqlite_migration::Migrations;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::LazyLock};

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
    const FILE: &str = concat!(env!("CARGO_PKG_NAME"), ".sqlite");
    std::env::current_exe()
      .unwrap()
      .parent()
      .unwrap()
      .to_path_buf()
      .join(FILE)
  }
}

#[derive(SystemParam, new)]
pub struct Settings<'w> {
  storage: ResMut<'w, Storage>,
}

impl Settings<'_> {
  pub fn set(&mut self, key: impl AsRef<str>, value: impl Serialize) -> Result {
    let key = key.as_ref();
    let serialized = ron::to_string(&value)?;

    self.storage.db.lock().execute(
      "INSERT INTO [settings]([key], [value]) VALUES(?1, ?2) ON CONFLICT([key]) DO UPDATE SET [value]=?2",
      (key, &serialized),
    )?;

    debug!("Updated setting {key} to {serialized}");

    Ok(())
  }

  pub fn get<T>(&mut self, key: impl AsRef<str>) -> Result<T>
  where
    T: for<'de> Deserialize<'de>,
  {
    let key = key.as_ref();

    let result: String = {
      self.storage.db.lock().query_one(
        "SELECT [value] FROM [settings] WHERE [key] == ?1",
        [key],
        |row| row.get(0).inspect_err(|err| error!("{err}")),
      )?
    };

    let value = ron::de::from_str(&result)?;

    debug!("Queried setting {key} = {result}");

    Ok(value)
  }

  pub fn get_or_default<T>(&mut self, key: impl AsRef<str>) -> T
  where
    T: for<'de> Deserialize<'de> + Default,
  {
    self.get::<T>(key).unwrap_or_default()
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
