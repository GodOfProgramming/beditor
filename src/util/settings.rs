use bevy::prelude::*;
use include_dir::{Dir, include_dir};
use parking_lot::Mutex;
use rusqlite::Connection;
use rusqlite_migration::Migrations;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::LazyLock};

static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations");

static MIGRATIONS: LazyLock<Migrations<'static>> =
  LazyLock::new(|| Migrations::from_directory(&MIGRATIONS_DIR).unwrap());

#[derive(Resource)]
pub struct Settings {
  db: Mutex<Connection>,
}

impl Settings {
  pub fn new() -> Result<Self> {
    let mut db = Connection::open(path())?;

    MIGRATIONS.to_latest(&mut db)?;

    Ok(Self { db: Mutex::new(db) })
  }

  pub fn set(&mut self, key: impl AsRef<str>, value: impl Serialize) -> Result {
    let key = key.as_ref();
    let serialized = ron::to_string(&value)?;

    self.db.lock().execute(
      "INSERT INTO [settings]([key], [value]) VALUES(?1, ?2) ON CONFLICT([key]) DO UPDATE SET [value]=?2",
      [key, &serialized],
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
      self.db.lock().query_one(
        &format!("SELECT [value] FROM [settings] WHERE [key] == ?1"),
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

fn path() -> PathBuf {
  const FILE: &str = concat!(env!("CARGO_PKG_NAME"), ".sqlite");
  std::env::current_exe()
    .unwrap()
    .parent()
    .unwrap()
    .to_path_buf()
    .join(FILE)
}
