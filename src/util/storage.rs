pub mod settings;

use crate::{APP_DIR, Notification};
use bevy::{ecs::system::SystemParam, prelude::*};
use include_dir::{Dir, include_dir};
use parking_lot::Mutex;
use rusqlite::{
	Connection,
	types::{ToSqlOutput, ValueRef},
};
use rusqlite_migration::Migrations;
use serde::{Deserialize, Serialize};
use std::{marker::PhantomData, path::PathBuf, sync::LazyLock};

static EMBEDDED_GLOBAL_EDITOR_MIGRATIONS: Dir =
	include_dir!("$CARGO_MANIFEST_DIR/migrations/global");
static EMBEDDED_PROJECT_MIGRATIONS: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations/project");

static GLOBAL_EDITOR_MIGRATIONS: LazyLock<Migrations<'static>> =
	LazyLock::new(|| Migrations::from_directory(&EMBEDDED_GLOBAL_EDITOR_MIGRATIONS).unwrap());

static PROJECT_MIGRATIONS: LazyLock<Migrations<'static>> =
	LazyLock::new(|| Migrations::from_directory(&EMBEDDED_PROJECT_MIGRATIONS).unwrap());

pub type GlobalEditorSettingsRes<'w> = ResMut<'w, Settings<Global>>;

#[derive(SystemParam, Deref, DerefMut)]
pub struct GlobalEditorSettings<'w, 's> {
	#[deref]
	settings: GlobalEditorSettingsRes<'w>,
	commands: Commands<'w, 's>,
}

impl GlobalEditorSettings<'_, '_> {
	pub fn set<D>(&mut self, data: D, value: D::Type) -> Result
	where
		D: PersistentData<Type: Clone + Send + Sync>,
	{
		self
			.settings
			.set(data, value.clone())
			.inspect(|_| {
				self.commands.trigger(SettingChanged::<D>::new(value));
			})
			.inspect_err(|err| {
				self
					.commands
					.trigger(Notification::error("Failed save setting").with_context(err.to_string()));
			})
	}
}

pub type ProjectSettingsRes<'w> = ResMut<'w, Settings<Project>>;

#[derive(SystemParam, Deref, DerefMut)]
pub struct ProjectSettings<'w, 's> {
	#[deref]
	settings: ProjectSettingsRes<'w>,
	commands: Commands<'w, 's>,
}

impl ProjectSettings<'_, '_> {
	pub fn set<D>(&mut self, data: D, value: D::Type) -> Result
	where
		D: PersistentData<Type: Clone + Send + Sync>,
	{
		self
			.settings
			.set(data, value.clone())
			.inspect(|_| {
				self.commands.trigger(SettingChanged::<D>::new(value));
			})
			.inspect_err(|err| {
				self
					.commands
					.trigger(Notification::error("Failed save setting").with_context(err.to_string()));
			})
	}
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

	pub fn set<D>(&mut self, data: D, value: D::Type) -> Result
	where
		D: PersistentData,
	{
		let serialized = D::serialize(value)?;
		let value = serialized.into();

		let key = data.to_key();

		let sql = format!(
			include_str!("sql/set.sql"),
			table = D::Table::TABLE,
			key = D::Table::KEY_COLUMN,
			value = D::Table::VALUE_COLUMN
		);

		self.db.lock().execute(
			&sql,
			[
				ToSqlOutput::Borrowed(ValueRef::from(key.as_str())),
				ToSqlOutput::Borrowed(ValueRef::from(&value)),
			],
		)?;

		Ok(())
	}

	pub fn get<D>(&mut self, data: D) -> Result<D::Type>
	where
		D: PersistentData,
	{
		let key = data.to_key();

		let sql = format!(
			include_str!("sql/get.sql"),
			table = D::Table::TABLE,
			key = D::Table::KEY_COLUMN,
			value = D::Table::VALUE_COLUMN
		);

		let result: <D::Table as DataTable>::DataType = {
			self.db.lock().query_one(&sql, [&key], |row| {
				row.get(0).inspect_err(|err| error!("{err}"))
			})?
		};

		let value = D::deserialize(result)?;

		Ok(value)
	}

	pub fn list_keys<T>(&mut self) -> Result<Vec<String>>
	where
		T: DataTable,
	{
		let db = self.db.lock();

		let mut stmt = db.prepare(&format!(
			include_str!("sql/list.sql"),
			table = T::TABLE,
			key = T::KEY_COLUMN
		))?;

		let names = stmt
			.query_map([], |row| row.get(0))?
			.filter_map(Result::ok)
			.filter(|name: &String| !name.is_empty())
			.collect::<Vec<String>>();

		Ok(names)
	}
}

pub trait LocalStorage {
	fn db() -> Result<Connection>;
}

pub struct Global;

impl LocalStorage for Global {
	fn db() -> Result<Connection> {
		let mut conn = Connection::open(APP_DIR.join("settings.sqlite"))?;

		GLOBAL_EDITOR_MIGRATIONS.to_latest(&mut conn)?;

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

pub trait DataTable {
	type DataType: Into<rusqlite::types::Value> + rusqlite::types::FromSql;
	const TABLE: &str;
	const KEY_COLUMN: &str;
	const VALUE_COLUMN: &str;
}

pub trait PersistentData: 'static + Send + Sync {
	type Table: DataTable;
	type Type: Serialize + for<'de> Deserialize<'de>;

	fn to_key(self) -> String;

	fn serialize(value: Self::Type) -> Result<<Self::Table as DataTable>::DataType>;

	fn deserialize(input: <Self::Table as DataTable>::DataType) -> Result<Self::Type>;
}

#[derive(Event)]
pub struct SettingChanged<D>
where
	D: PersistentData,
{
	pub value: D::Type,
}

impl<D> SettingChanged<D>
where
	D: PersistentData,
{
	fn new(value: D::Type) -> Self {
		Self { value }
	}
}
