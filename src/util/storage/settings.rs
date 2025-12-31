use crate::{
	DataTable,
	private::{
		cam::{ActiveEditorCamera, cam2d, cam3d},
		util::log::LogLevel,
	},
	util::storage::PersistentData,
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

pub struct ViewSettingsGroup;

impl SettingsGroup for ViewSettingsGroup {
	type Table = SettingsTable;
	const NAME: &str = "view";
}

pub struct RenderCamerasSetting;

impl Setting for RenderCamerasSetting {
	type Type = bool;
	type Group = ViewSettingsGroup;
	const NAME: &str = "render_cameras";
}

pub struct ActiveEditorCameraSetting;

impl Setting for ActiveEditorCameraSetting {
	type Type = ActiveEditorCamera;
	type Group = ViewSettingsGroup;
	const NAME: &str = "active_editor_camera";
}

pub struct CamStateSetting2d;

impl Setting for CamStateSetting2d {
	type Type = cam2d::CameraSaveData;
	type Group = ViewSettingsGroup;
	const NAME: &str = "cam2d_state";
}

pub struct CamStateSetting3d;

impl Setting for CamStateSetting3d {
	type Type = cam3d::CameraSaveData;
	type Group = ViewSettingsGroup;
	const NAME: &str = "cam3d_state";
}
