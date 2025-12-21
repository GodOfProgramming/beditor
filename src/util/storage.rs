mod settings;

use bevy::{ecs::system::SystemParam, prelude::*};
use derive_more::derive::DerefMut;
use derive_new::new;
use egui_dock::DockState;
use include_dir::{Dir, include_dir};
use parking_lot::Mutex;
use persistent_id::PersistentId;
use rusqlite::Connection;
use rusqlite_migration::Migrations;
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, marker::PhantomData, path::PathBuf, sync::LazyLock};

pub use settings::*;

use crate::APP_DIR;

static EMBEDDED_GLOBAL_MIGRATIONS: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations/global");
static EMBEDDED_PROJECT_MIGRATIONS: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations/project");

static GLOBAL_MIGRATIONS: LazyLock<Migrations<'static>> =
	LazyLock::new(|| Migrations::from_directory(&EMBEDDED_GLOBAL_MIGRATIONS).unwrap());

static PROJECT_MIGRATIONS: LazyLock<Migrations<'static>> =
	LazyLock::new(|| Migrations::from_directory(&EMBEDDED_PROJECT_MIGRATIONS).unwrap());

pub type GlobalSettingsRes<'w> = ResMut<'w, Settings<Global>>;

#[derive(SystemParam, Deref, DerefMut)]
pub struct GlobalSettings<'w> {
	settings: GlobalSettingsRes<'w>,
}

pub type ProjectSettingsRes<'w> = ResMut<'w, Settings<Project>>;

#[derive(SystemParam, Deref, DerefMut)]
pub struct ProjectSettings<'w> {
	settings: ProjectSettingsRes<'w>,
}

#[derive(Resource)]
pub struct Settings<L>
where
	L: LocalStorage,
{
	db: Mutex<Connection>,
	_pd: PhantomData<L>,
}

impl<L> Settings<L>
where
	L: LocalStorage,
{
	pub fn new() -> Result<Self> {
		let conn = L::db()?;

		Ok(Self {
			db: Mutex::new(conn),
			_pd: default(),
		})
	}

	pub fn set<S>(&mut self, value: impl Borrow<S::Type>) -> Result
	where
		S: Setting,
	{
		let serialized = ron::to_string(value.borrow())?;

		let key = S::field();

		self.db.lock().execute(
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
			self.db.lock().query_one(
				"SELECT [value] FROM [settings] WHERE [key] == ?1",
				(&key,),
				|row| row.get(0).inspect_err(|err| error!("{err}")),
			)?
		};

		let value = ron::de::from_str(&result)?;

		debug!("Queried setting {key} = {result}");

		Ok(value)
	}

	pub fn get_or<S>(&mut self, alt: S::Type) -> S::Type
	where
		S: Setting,
		S::Type: Default,
	{
		self.get::<S>().unwrap_or(alt)
	}

	pub fn get_or_default<S>(&mut self) -> S::Type
	where
		S: Setting,
		S::Type: Default,
	{
		self.get::<S>().unwrap_or_default()
	}
}

pub trait LocalStorage {
	fn db() -> Result<Connection>;
}

pub struct Global;

impl LocalStorage for Global {
	fn db() -> Result<Connection> {
		let mut conn = Connection::open(APP_DIR.join("settings.sqlite"))?;

		GLOBAL_MIGRATIONS.to_latest(&mut conn)?;

		Ok(conn)
	}
}

pub struct Project;

impl Project {
	fn path() -> PathBuf {
		let current_exe = std::env::current_exe().expect("failed to get current executable");
		let stem = current_exe
			.file_stem()
			.expect("failed to get current executable file stem");

		let filename = format!("{}.{}.sqlite", stem.display(), env!("CARGO_PKG_NAME"));
		current_exe.parent().unwrap().to_path_buf().join(filename)
	}
}

impl LocalStorage for Project {
	fn db() -> Result<Connection> {
		let mut conn = Connection::open(Self::path())?;

		PROJECT_MIGRATIONS.to_latest(&mut conn)?;

		Ok(conn)
	}
}

#[derive(SystemParam, new)]
pub struct Layouts<'w> {
	storage: ResMut<'w, Settings<Project>>,
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
