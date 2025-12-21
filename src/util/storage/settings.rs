use crate::{
	RuntimeSettings,
	util::log::LogLevel,
	view::{cam::ActiveEditorCamera, view2d, view3d},
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

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

///////////////////////////////////////////////////////////////////////////////

pub struct EditorSettingsGroup;

impl SettingsGroup for EditorSettingsGroup {
	const GROUP: &str = "editor";
}

pub struct EditorSettingsSetting;

impl SettingKey for EditorSettingsSetting {
	type Type = RuntimeSettings;
	type Group = EditorSettingsGroup;
	const KEY: &str = "settings";
}

pub struct StartEditorInTestingSetting;

impl SettingKey for StartEditorInTestingSetting {
	type Type = bool;
	type Group = EditorSettingsGroup;
	const KEY: &str = "start_in_testing";
}

pub struct EditorEguiSettings;

impl SettingKey for EditorEguiSettings {
	type Type = bevy_egui::egui::Options;

	type Group = EditorSettingsGroup;

	const KEY: &str = "egui_settings";
}

///////////////////////////////////////////////////////////////////////////////

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

pub struct WindowSizeSetting;

impl SettingKey for WindowSizeSetting {
	type Type = Vec2;
	type Group = WindowSettingsGroup;
	const KEY: &str = "size";
}

///////////////////////////////////////////////////////////////////////////////

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

pub struct CurrentThemeSetting;

impl SettingKey for CurrentThemeSetting {
	type Type = String;
	type Group = UiSettingsGroup;
	const KEY: &str = "current_theme";
}

///////////////////////////////////////////////////////////////////////////////

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

///////////////////////////////////////////////////////////////////////////////

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

///////////////////////////////////////////////////////////////////////////////
