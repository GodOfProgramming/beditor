use crate::{
	private::util::log::LogLevel,
	storage::{DataTable, PersistentData},
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;

pub struct SettingsTable;

impl DataTable for SettingsTable {
	type DataType = String;
	const TABLE: &str = "settings";
	const KEY_COLUMN: &str = "key";
	const VALUE_COLUMN: &str = "value";
}

pub trait SettingsGroup {
	type Table: DataTable;
	const NAME: &str;
}

pub trait Setting: 'static + Send + Sync {
	type Type: Serialize + for<'de> Deserialize<'de>;
	type Group: SettingsGroup;
	const NAME: &str;
}

impl<T> PersistentData for T
where
	T: Setting,
	<T::Group as SettingsGroup>::Table: DataTable<DataType = String>,
{
	type Table = <<T as Setting>::Group as SettingsGroup>::Table;
	type Type = T::Type;

	fn to_key(self) -> String {
		format!("{}.{}", T::Group::NAME, T::NAME)
	}

	fn serialize(value: Self::Type) -> Result<String> {
		let value = ron::to_string(value.borrow())?;
		Ok(value)
	}

	fn deserialize(input: String) -> Result<Self::Type> {
		let value = ron::de::from_str(&input)?;
		Ok(value)
	}
}

///////////////////////////////////////////////////////////////////////////////

pub struct EditorSettingsGroup;

impl SettingsGroup for EditorSettingsGroup {
	type Table = SettingsTable;
	const NAME: &str = "editor";
}

pub struct EditorEguiSettings;

impl Setting for EditorEguiSettings {
	type Type = bevy_egui::egui::Options;

	type Group = EditorSettingsGroup;

	const NAME: &str = "egui_settings";
}

pub struct EditorUiScale;

impl Setting for EditorUiScale {
	type Type = f32;
	type Group = EditorSettingsGroup;
	const NAME: &str = "ui_scale";
}

///////////////////////////////////////////////////////////////////////////////

pub struct WindowSettingsGroup;

impl SettingsGroup for WindowSettingsGroup {
	type Table = SettingsTable;
	const NAME: &str = "window";
}

pub struct WindowMaximizedSetting;

impl Setting for WindowMaximizedSetting {
	type Type = bool;
	type Group = WindowSettingsGroup;
	const NAME: &str = "maximized";
}

pub struct WindowSizeSetting;

impl Setting for WindowSizeSetting {
	type Type = Vec2;
	type Group = WindowSettingsGroup;
	const NAME: &str = "size";
}

///////////////////////////////////////////////////////////////////////////////

pub struct UiSettingsGroup;

impl SettingsGroup for UiSettingsGroup {
	type Table = SettingsTable;
	const NAME: &str = "ui";
}

pub struct SaveLayoutOnExitSetting;

impl Setting for SaveLayoutOnExitSetting {
	type Type = bool;
	type Group = UiSettingsGroup;
	const NAME: &str = "save_layout_on_exit";
}

pub struct CurrentLayoutSetting;

impl Setting for CurrentLayoutSetting {
	type Type = String;
	type Group = UiSettingsGroup;
	const NAME: &str = "current_layout";
}

pub struct CurrentThemeSetting;

impl Setting for CurrentThemeSetting {
	type Type = String;
	type Group = UiSettingsGroup;
	const NAME: &str = "current_theme";
}

///////////////////////////////////////////////////////////////////////////////

pub struct LogSettingsGroup;

impl SettingsGroup for LogSettingsGroup {
	type Table = SettingsTable;
	const NAME: &str = "log";
}

pub struct LogLevelSetting;

impl Setting for LogLevelSetting {
	type Type = LogLevel;
	type Group = LogSettingsGroup;
	const NAME: &str = "level";
}

///////////////////////////////////////////////////////////////////////////////
