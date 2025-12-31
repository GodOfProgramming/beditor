//! For queries that may actually be more readable without abstraction
#![allow(clippy::type_complexity)]
//! For systems that may actually be more readable without abstraction
#![allow(clippy::too_many_arguments)]

pub mod inspector;
mod panels;
mod private;
pub mod ui;
mod util;

use crate::{
	panels::managed_view::EditorManagedViewUiExtension,
	settings::{WindowMaximizedSetting, WindowSizeSetting},
	util::{
		WorldExtensions as _,
		components::{ComponentRegistry, RegisterableComponent, RegisterableComponents},
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
	ecs::{
		entity_disabling::Disabled,
		system::{NonSendMarker, SystemParam},
	},
	prelude::*,
	remote::{RemotePlugin, http::RemoteHttpPlugin},
	window::{CursorOptions, PrimaryWindow, WindowCloseRequested, WindowMode},
	winit::WINIT_WINDOWS,
};
use bevy_axes_gizmo::AxesGizmoPlugin;
use bevy_infinite_grid::InfiniteGridPlugin;
use bevy_mod_outline::OutlinePlugin;
use brefabs::{PrefabPlugin, Prefabs};
use derive_new::new;
use platform_dirs::AppDirs;
use private::ui::UiManager;
use std::{path::PathBuf, sync::LazyLock};
use transform_gizmo_bevy::TransformGizmoPlugin;

pub use prelude::*;

pub mod prelude {
	pub use crate::{
		AppSystems, EditorExtension, EditorExtensionContext, EditorExtensionPlugin, EditorPlugin,
		ui::{EditorUi, EditorUiBundle},
		util::{
			AppExtensions, RegisterableType, TypeGroups, TypeList,
			storage::{
				DataTable, PersistentData, Project, ProjectSettings, SettingChanged, Settings, settings,
			},
		},
	};
	pub use bevy_egui;
	pub use brefabs;
	pub use egui;
	pub use macros::{self, Identifiable};
	pub use persistent_id::{self, Identifiable};
	pub use serde;
	pub use uuid;
	pub use widgets;
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
	camera_registrations: Vec<fn(&mut App)>,
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

	pub fn register_camera<C>(mut self) -> Self
	where
		C: Component + Identifiable,
	{
		self.camera_registrations.push(|app| {
			app.add_plugins(EditorExtensionPlugin::<EditorManagedViewUiExtension<C>>::default());
		});
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
			.add_plugins(private::util::log::LogPlugin);

		if let Some(config_fn) = &self.default_plugins {
			(config_fn)(app);
		} else {
			app.add_plugins(Self::override_defaults(DefaultPlugins.build()));
		}

		for f in self.camera_registrations.iter() {
			(f)(app);
		}

		app
			.init_resource::<ComponentRegistry>()
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
			.try_add_plugin(OutlinePlugin)
			.try_add_plugin(TransformGizmoPlugin)
			// internal
			.add_plugins(private::PrivatePlugins)
			// extensions
			.add_plugins(EditorExtensionPlugin::<panels::EditorPanelsExtension>::default())
			.add_observer(EnableGameUiEvent::handle)
			.add_observer(DisableGameUiEvent::handle)
			.add_systems(Startup, configure_windows)
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
				finish_exit.in_set(EditorGlobalSystems),
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
	fn build_editor(&self, ctx: &mut EditorExtensionContext);

	fn build_app(&self, app: &mut App) {
		let _ = app;
	}
}

#[derive(new, Deref)]
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
	fn build(&self, app: &mut App) {
		self.build_app(app);
	}

	fn finish(&self, app: &mut App) {
		info!("Added editor extension {}", std::any::type_name::<T>());

		app
			.world_mut()
			.resources_scope::<(UiManager, ComponentRegistry, Prefabs)>(|world, resources| {
				let (ui_manager, components, prefabs) = resources;
				let mut ctx = EditorExtensionContext::new(world, components, prefabs, ui_manager);
				self.0.build_editor(&mut ctx);
			});
	}
}

impl<T> From<T> for EditorExtensionPlugin<T>
where
	T: 'static + Send + Sync + EditorExtension,
{
	fn from(value: T) -> Self {
		Self(value)
	}
}

#[derive(new)]
pub struct EditorExtensionContext<'w> {
	world: &'w mut World,
	components: &'w mut ComponentRegistry,
	prefabs: &'w mut Prefabs,
	ui_manager: &'w mut UiManager,
}

impl<'w> EditorExtensionContext<'w> {
	pub fn world(&mut self) -> &mut World {
		self.world
	}

	pub fn prefabs(&mut self) -> &mut Prefabs {
		self.prefabs
	}

	pub fn register_component<T: RegisterableComponent>(&mut self) -> &mut Self {
		self.components.register::<T>(self.world);
		self
	}

	pub fn register_components<T: RegisterableComponents>(&mut self) -> &mut Self {
		self.components.register_multiple::<T>(self.world);
		self
	}

	pub fn register_ui<U: EditorUiBundle>(&mut self) -> &mut Self {
		self.ui_manager.register::<U>();
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
		private::util::window::show_cursor(&mut cursor);
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

#[derive(SystemParam)]
pub struct NoParams;

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
