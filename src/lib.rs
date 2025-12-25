//! For queries that may actually be more readable without abstraction
#![allow(clippy::type_complexity)]
//! For systems that may actually be more readable without abstraction
#![allow(clippy::too_many_arguments)]

mod input;
pub mod inspector;
mod ui;
mod util;
mod view;

use std::{path::PathBuf, sync::LazyLock};

use crate::{
	inspector::InspectorPlugin,
	settings::{
		EditorSettingsSetting, StartEditorInTestingSetting, WindowMaximizedSetting, WindowSizeSetting,
	},
	ui::builtin::managed_view::EditorManagedView,
	util::{
		components::{ComponentRegistry, RegisterableComponent, RegisterableComponents},
		log::LogPlugin,
		reflection::ReflectionExtensionsPlugin,
		storage::{Global, GlobalEditorSettings},
	},
};
use bevy::{
	app::PluginGroupBuilder,
	diagnostic::{
		EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
	},
	ecs::{entity_disabling::Disabled, system::NonSendMarker},
	prelude::*,
	reflect::Reflectable,
	remote::{RemotePlugin, http::RemoteHttpPlugin},
	window::{CursorOptions, PrimaryWindow, WindowCloseRequested, WindowMode},
	winit::WINIT_WINDOWS,
};
use bevy_axes_gizmo::AxesGizmoPlugin;
use bevy_mesh_outline::MeshOutlinePlugin;
use brefabs::PrefabPlugin;
use input::InputPlugin;
use platform_dirs::AppDirs;
pub use prelude::*;
use serde::{Deserialize, Serialize};
use transform_gizmo_bevy::TransformGizmoPlugin;
use ui::{UiManager, UiPlugin};
use view::EditorViewPlugin;

pub mod prelude {
	pub use crate::{
		EditorPlugin,
		ui::{EditorUi, EditorUiBundle, NoParams, notifications::Notification, widgets},
		util::{
			AppExtensions, EntityManager, GameEntity, GameRenderLayer, RegisterableType, TypeGroups,
			TypeList,
			reflection::{ReflectDefaultCache, serde::SerdeRegistry},
			storage::{
				DataTable, PersistentData, Project, ProjectSettings, SettingChanged, Settings, settings,
			},
		},
		view::cam::EditorCamera,
	};
	pub use bevy_egui;
	pub use brefabs;
	pub use macros::{self, Identifiable};
	pub use persistent_id::{self, Identifiable};
	pub use serde;
	pub use uuid;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
pub enum EditorState {
	Editing,
	Testing,
	Exiting,
}

type AppRegistrationFn = Box<dyn Fn(&mut App) + Send + Sync>;
type UiRegistrationFn = Box<dyn Fn(&mut App, &mut UiManager) + Send + Sync>;
type ComponentRegistrationFn = Box<dyn Fn(&mut App, &mut ComponentRegistry) + Send + Sync>;

static APP_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
	let dirs =
		AppDirs::new(Some("beditor"), false).expect("Could not acquire application directories");
	std::fs::create_dir_all(&dirs.data_dir).expect("Must be able to create app data dir");
	dirs.data_dir
});

#[derive(Default)]
pub struct EditorPlugin {
	default_plugins: Option<AppRegistrationFn>,
	generic_registrations: Vec<AppRegistrationFn>,
	ui_registrations: Vec<UiRegistrationFn>,
	component_registrations: Vec<ComponentRegistrationFn>,
}

impl EditorPlugin {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn configure_defaults<P, F>(mut self, f: F) -> Self
	where
		F: Fn(&mut App, DefaultPlugins) -> P + Send + Sync + 'static,
		P: Into<PluginGroupBuilder> + 'static,
	{
		self.default_plugins = Some(Box::new(move |app| {
			let plugins = (f)(app, DefaultPlugins).into();
			app.add_plugins(Self::override_defaults(plugins));
		}));
		self
	}

	pub fn register_component<T: RegisterableComponent>(mut self) -> Self {
		self
			.component_registrations
			.push(Box::new(|app, component_registry| {
				T::register(app.world_mut(), component_registry);
			}));
		self
	}

	pub fn register_components<T: RegisterableComponents>(mut self) -> Self {
		self
			.component_registrations
			.push(Box::new(|app, component_registry| {
				T::register_components(app.world_mut(), component_registry);
			}));
		self
	}

	pub fn register_game_camera<C>(mut self) -> Self
	where
		C: Component + Reflectable + Identifiable,
	{
		self.ui_registrations.push(Box::new(|app, ui_manager| {
			view::add_game_camera::<C>(app);
			ui_manager.register::<EditorManagedView<C>>(app);
		}));
		self
	}

	pub fn register_ui<U: EditorUiBundle>(mut self) -> Self {
		self.ui_registrations.push(Box::new(|app, ui_manager| {
			ui_manager.register::<U>(app);
		}));
		self
	}

	fn override_defaults(builder: PluginGroupBuilder) -> PluginGroupBuilder {
		builder
			.set(WindowPlugin {
				primary_window: Some(Window {
					title: String::from("Beditor"),
					mode: WindowMode::Windowed,
					position: WindowPosition::Automatic,
					visible: false,
					..default()
				}),
				close_when_requested: false,
				..default()
			})
			.disable::<bevy::log::LogPlugin>()
	}

	fn configure<'a>(&self, app: &'a mut App) -> &'a mut App {
		app
			.insert_resource(Settings::<Global>::new().unwrap())
			.insert_resource(Settings::<Project>::new().unwrap());

		if let Some(config_fn) = &self.default_plugins {
			(config_fn)(app);
		} else {
			app.add_plugins(Self::override_defaults(DefaultPlugins.build()));
		}

		for f in &self.generic_registrations {
			(f)(app);
		}

		let mut ui_manager = UiManager::new(app);
		for f in &self.ui_registrations {
			(f)(app, &mut ui_manager);
		}

		let mut component_registry = ComponentRegistry::default();
		for f in &self.component_registrations {
			(f)(app, &mut component_registry);
		}

		app
			.insert_resource(component_registry)
			.insert_resource(ui_manager)
	}
}

impl Plugin for EditorPlugin {
	fn build(&self, app: &mut App) {
		dotenvy::dotenv().ok();

		self
			.configure(app)
			.init_resource::<RuntimeSettings>()
			.init_resource::<GameRenderLayer>()
			.insert_state(EditorState::Editing)
			.configure_sets(
				Update,
				(
					EditorGlobalSystems,
					EditingSystems
						.in_set(EditorGlobalSystems)
						.run_if(in_state(EditorState::Editing)),
				),
			)
			.configure_sets(
				FixedUpdate,
				(
					EditorGlobalSystems,
					EditingSystems
						.in_set(EditorGlobalSystems)
						.run_if(in_state(EditorState::Editing)),
				),
			)
			.add_plugin_if_not_present(MeshPickingPlugin)
			.add_plugin_if_not_present(FrameTimeDiagnosticsPlugin::default())
			.add_plugin_if_not_present(EntityCountDiagnosticsPlugin::default())
			.add_plugin_if_not_present(SystemInformationDiagnosticsPlugin)
			.add_plugin_if_not_present(RemotePlugin::default())
			.add_plugin_if_not_present(RemoteHttpPlugin::default())
			.add_plugin_if_not_present(MeshOutlinePlugin)
			.add_plugin_if_not_present(TransformGizmoPlugin)
			.add_plugin_if_not_present(AxesGizmoPlugin::default())
			.add_plugins((
				EditorViewPlugin,
				InputPlugin,
				UiPlugin,
				LogPlugin,
				ReflectionExtensionsPlugin,
				InspectorPlugin,
			))
			.add_observer(EnableGameUiEvent::handle)
			.add_observer(DisableGameUiEvent::handle)
			.add_systems(
				Startup,
				(configure_windows, auto_register_components, load_settings),
			)
			.add_systems(PostStartup, show_window)
			.add_systems(OnEnter(EditorState::Editing), show_window_cursor)
			.add_systems(FixedUpdate, (on_close_requested, handle_window_events))
			.add_systems(
				OnEnter(EditorState::Exiting),
				(save_editor_settings, finish_exit).in_set(EditorGlobalSystems),
			);
	}

	fn finish(&self, app: &mut App) {
		app.add_plugin_if_not_present(PrefabPlugin::default());
	}
}

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct EditorGlobalSystems;

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct EditingSystems;

#[derive(Resource, Reflect, Serialize, Deserialize, Clone)]
#[reflect(Resource, Default)]
pub struct RuntimeSettings {
	render_ui: bool,
}

impl Default for RuntimeSettings {
	fn default() -> Self {
		Self { render_ui: true }
	}
}

fn save_editor_settings(
	mut settings: ProjectSettings,
	editor_settings: Res<RuntimeSettings>,
) -> Result {
	settings.set(EditorSettingsSetting, editor_settings.clone())
}

fn load_settings(
	mut settings: ProjectSettings,
	mut editor_settings: ResMut<RuntimeSettings>,
	mut next_state: ResMut<NextState<EditorState>>,
) {
	*editor_settings = settings.get(EditorSettingsSetting).unwrap_or_default();

	let start_in_testing = settings
		.get(StartEditorInTestingSetting)
		.unwrap_or_default();

	if start_in_testing {
		next_state.set(EditorState::Testing);
	}
}

fn auto_register_components(world: &mut World) {
	world.resource_scope(|world, mut component_registry: Mut<ComponentRegistry>| {
		let app_type_registry = world.resource::<AppTypeRegistry>().clone();
		let type_registry = app_type_registry.read();

		for entry in type_registry.iter() {
			component_registry.register_raw(world, entry);
		}
	});
}

fn show_window(mut q_windows: Query<&mut Window>) {
	for mut window in &mut q_windows {
		window.visible = true;
	}
}

fn finish_exit(mut app_exit: MessageWriter<AppExit>) {
	app_exit.write(AppExit::Success);
}

fn show_window_cursor(mut q_cursors: Query<&mut CursorOptions>) {
	for mut cursor in &mut q_cursors {
		util::window::show_cursor(&mut cursor);
	}
}

fn configure_windows(
	mut settings: GlobalEditorSettings,
	mut window: Single<&mut Window, With<PrimaryWindow>>,
) -> Result<()> {
	let maximized = settings.get(WindowMaximizedSetting).unwrap_or_default();
	window.set_maximized(maximized);

	if let Ok(size) = settings.get(WindowSizeSetting) {
		window.resolution.set(size.x, size.y);
	}

	Ok(())
}

#[derive(Event)]
pub struct EnableGameUiEvent;

impl EnableGameUiEvent {
	/// This will likely need further logic to account for game logic that wants to disable UI but needs to not be managed by this
	/// Maybe a marker component that signals that the editor should not disable or enable the UI as a first pass
	fn handle(
		_: On<Self>,
		mut commands: Commands,
		q_ui: Query<Entity, (With<Node>, Allow<Disabled>)>,
	) {
		for entity in q_ui {
			commands
				.entity(entity)
				.remove_recursive::<Children, Disabled>();
		}
	}
}

#[derive(Event)]
pub struct DisableGameUiEvent;

impl DisableGameUiEvent {
	fn handle(_: On<Self>, mut commands: Commands, q_ui: Query<Entity, With<Node>>) {
		for entity in q_ui {
			commands
				.entity(entity)
				.insert_recursive::<Children>(Disabled);
		}
	}
}

fn on_close_requested(
	mut close_requests: MessageReader<WindowCloseRequested>,
	mut next_editor_state: ResMut<NextState<EditorState>>,
) {
	if !close_requests.is_empty() {
		close_requests.clear();
		next_editor_state.set(EditorState::Exiting)
	}
}

fn handle_window_events(
	mut settings: GlobalEditorSettings,
	mut events: MessageReader<bevy::window::WindowResized>,
	window: Single<&mut Window, With<PrimaryWindow>>,
	mut was_maximized: Local<Option<bool>>,
	mut last_size: Local<Option<Vec2>>,
	_non_send_marker: NonSendMarker, // forces main thread
) -> Result {
	WINIT_WINDOWS.with_borrow(|windows| -> Result {
		for event in events.read() {
			let Some(winit_window) = windows.get_window(event.window) else {
				continue;
			};

			{
				let is_maximized = winit_window.is_maximized();
				if *was_maximized != Some(is_maximized) {
					settings.set(WindowMaximizedSetting, is_maximized)?;
					*was_maximized = Some(is_maximized);
				}
			}
		}

		Ok(())
	})?;

	{
		let size = window.resolution.size();
		if *last_size != Some(size) {
			settings.set(WindowSizeSetting, size)?;
			*last_size = Some(size);
		}
	}

	Ok(())
}
