//! For queries that may actually be more readable without abstraction
#![allow(clippy::type_complexity)]
//! For systems that may actually be more readable without abstraction
#![allow(clippy::too_many_arguments)]

mod assets;
mod input;
pub mod inspector;
mod panels;
mod scene;
mod ui;
mod util;
mod view;

use crate::{
	inspector::EditorInspectorPlugin,
	panels::prelude::*,
	scene::EditorScenePlugin,
	settings::{EditorSettingsSetting, WindowMaximizedSetting, WindowSizeSetting},
	util::{
		components::{ComponentRegistry, RegisterableComponent, RegisterableComponents},
		log::LogPlugin,
		reflection::ReflectionExtensionsPlugin,
		storage::{Global, GlobalEditorSettings},
	},
};
use bevy::{
	app::PluginGroupBuilder,
	dev_tools::{
		frame_time_graph::FrameTimeGraphPlugin, picking_debug::DebugPickingPlugin,
		states::log_transitions,
	},
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
use bevy_infinite_grid::InfiniteGridPlugin;
use bevy_mesh_outline::MeshOutlinePlugin;
use brefabs::PrefabPlugin;
use derive_new::new;
use input::EditorInputPlugin;
use platform_dirs::AppDirs;
pub use prelude::*;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::LazyLock};
use transform_gizmo_bevy::TransformGizmoPlugin;
use ui::EditorUiPlugin;
use view::EditorViewPlugin;

pub mod prelude {
	pub use crate::{
		AppSystems, EditorExtension, EditorExtensionContext, EditorExtensionPlugin, EditorPlugin,
		ui::{EditorUi, EditorUiBundle, NoParams, UiManager, notifications::Notification, widgets},
		util::{
			AppExtensions, RegisterableType, TypeGroups, TypeList,
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

/// All application systems that need to be editor controlled should be a part of this set
#[derive(SystemSet, Hash, Debug, PartialEq, Eq, Clone)]
pub struct AppSystems;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
pub enum EditorState {
	Editing,
	SimulationPrep,
	Simulating(SimulationState),
	Exiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimulationState {
	Live,
	Idle,
}

static APP_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
	let dirs =
		AppDirs::new(Some("beditor"), false).expect("Could not acquire application directories");
	std::fs::create_dir_all(&dirs.data_dir).expect("Must be able to create app data dir");
	dirs.data_dir
});

type AppRegistrationFn = Box<dyn Fn(&mut App) + Send + Sync>;

#[derive(Default)]
pub struct EditorPlugin {
	skip_registering_reflected_components: bool,
	default_plugins: Option<AppRegistrationFn>,
}

impl EditorPlugin {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn skip_registering_reflected_components(mut self) -> Self {
		self.skip_registering_reflected_components = true;
		self
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
}

impl Plugin for EditorPlugin {
	fn build(&self, app: &mut App) {
		dotenvy::dotenv().ok();

		app
			.insert_resource(Settings::<Global>::new().unwrap())
			.insert_resource(Settings::<Project>::new().unwrap())
			.add_plugins(LogPlugin);

		if let Some(config_fn) = &self.default_plugins {
			(config_fn)(app);
		} else {
			app.add_plugins(Self::override_defaults(DefaultPlugins.build()));
		}

		let ui_manager = UiManager::new(app);
		app
			.insert_resource(ui_manager)
			.init_resource::<ComponentRegistry>()
			.init_resource::<RuntimeSettings>()
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
			// bevy
			.try_add_plugin(MeshPickingPlugin)
			.try_add_plugin(DebugPickingPlugin)
			.try_add_plugin(SystemInformationDiagnosticsPlugin)
			.try_add_plugin(EntityCountDiagnosticsPlugin::default())
			.try_add_plugin(FrameTimeDiagnosticsPlugin::default())
			.try_add_plugin(FrameTimeGraphPlugin)
			.try_add_plugin(RemotePlugin::default())
			.try_add_plugin(RemoteHttpPlugin::default())
			// crates
			.try_add_plugin(AxesGizmoPlugin::default())
			.try_add_plugin(InfiniteGridPlugin)
			.try_add_plugin(MeshOutlinePlugin)
			.try_add_plugin(TransformGizmoPlugin)
			// internal
			.add_plugins((
				EditorScenePlugin,
				EditorViewPlugin,
				EditorInputPlugin,
				EditorUiPlugin,
				EditorInspectorPlugin,
				ReflectionExtensionsPlugin,
			))
			.add_observer(EnableGameUiEvent::handle)
			.add_observer(DisableGameUiEvent::handle)
			.add_systems(
				Startup,
				(configure_windows, load_settings, assets::add_primitives),
			)
			.add_systems(PostStartup, show_window)
			.add_systems(OnEnter(EditorState::Editing), show_window_cursor)
			.add_systems(
				FixedUpdate,
				(
					on_close_requested,
					handle_window_events,
					log_transitions::<EditorState>,
				),
			)
			.add_systems(
				OnEnter(EditorState::Exiting),
				(save_editor_settings, finish_exit).in_set(EditorGlobalSystems),
			);

		if !self.skip_registering_reflected_components {
			app.add_systems(Startup, auto_register_components);
		}
	}

	fn finish(&self, app: &mut App) {
		app.try_add_plugin(PrefabPlugin::default());

		let mut schedules = app.world_mut().resource_mut::<Schedules>();

		for (_, schedule) in schedules.iter_mut() {
			schedule.configure_sets(
				AppSystems.run_if(in_state(EditorState::Simulating(SimulationState::Live))),
			);
		}
	}
}

pub trait EditorExtension {
	fn build(&self, ctx: EditorExtensionContext);
}

#[derive(new)]
pub struct EditorExtensionPlugin<T>(T)
where
	T: EditorExtension;

impl<T> Default for EditorExtensionPlugin<T>
where
	T: Default + EditorExtension,
{
	fn default() -> Self {
		Self(default())
	}
}

impl<T> Plugin for EditorExtensionPlugin<T>
where
	T: 'static + Send + Sync + EditorExtension,
{
	fn build(&self, _: &mut App) {}

	fn finish(&self, app: &mut App) {
		app.try_add_plugin(EditorPlugin::new());

		let mut ui_registrations = Vec::new();

		app
			.world_mut()
			.resource_scope(|world, mut components: Mut<ComponentRegistry>| {
				let ctx = EditorExtensionContext::new(world, &mut components, &mut ui_registrations);
				self.0.build(ctx);
			});

		let mut ui_manager = app
			.world_mut()
			.remove_resource::<UiManager>()
			.expect("EditorPlugin must be added before this");

		for f in ui_registrations {
			(f)(app, &mut ui_manager);
		}

		app.world_mut().insert_resource(ui_manager);
	}
}

type UiRegistrationFn = fn(&mut App, &mut UiManager);

#[derive(new)]
pub struct EditorExtensionContext<'w> {
	world: &'w mut World,
	components: &'w mut ComponentRegistry,

	app_ui_registrations: &'w mut Vec<UiRegistrationFn>,
}

impl<'w> EditorExtensionContext<'w> {
	pub fn register_component<T: RegisterableComponent>(self) -> Self {
		T::register(self.world, self.components);
		self
	}

	pub fn register_components<T: RegisterableComponents>(self) -> Self {
		T::register_components(self.world, self.components);
		self
	}

	pub fn register_game_camera<C>(self) -> Self
	where
		C: Component + Reflectable + Identifiable,
	{
		self.app_ui_registrations.push(|app, ui_manager| {
			view::add_game_camera::<C>(app);
			ui_manager.register::<EditorManagedViewUi<C>>(app);
		});
		self
	}

	pub fn register_ui<U: EditorUiBundle>(self) -> Self {
		self.app_ui_registrations.push(|app, ui_manager| {
			ui_manager.register::<U>(app);
		});
		self
	}
}

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct EditorGlobalSystems;

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
struct EditingSystems;

#[derive(Component, Default)]
struct EditorOwned;

#[derive(Component)]
struct Simulated;

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

fn load_settings(mut settings: ProjectSettings, mut editor_settings: ResMut<RuntimeSettings>) {
	*editor_settings = settings.get(EditorSettingsSetting).unwrap_or_default();
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
