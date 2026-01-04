#![expect(
	clippy::type_complexity,
	reason = "For queries that may actually be more readable without abstraction"
)]
#![expect(
	clippy::too_many_arguments,
	reason = "For systems that may actually be more readable without abstraction"
)]

pub mod inspector;
mod private;
pub mod ui;
mod util;

use crate::{
	private::{EditorInternalFilter, ui::EditorUiEguiContextPass},
	util::{
		WorldExtensions as _,
		components::{ComponentRegistry, RegisterableComponent, RegisterableComponents},
		storage::Global,
	},
};
use bevy::{
	app::PluginGroupBuilder,
	dev_tools::{frame_time_graph::FrameTimeGraphPlugin, picking_debug::DebugPickingPlugin},
	diagnostic::{
		EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
	},
	prelude::*,
	remote::{RemotePlugin, http::RemoteHttpPlugin},
	window::WindowMode,
};
use bevy_infinite_grid::InfiniteGridPlugin;
use bevy_mod_outline::OutlinePlugin;
use brefabs::{PrefabPlugin, Prefabs};
use derive_new::new;
use notify::NotificationPlugin;
use platform_dirs::AppDirs;
use private::ui::UiManager;
use std::{path::PathBuf, sync::LazyLock};
use transform_gizmo_bevy::TransformGizmoPlugin;

pub use prelude::*;

pub mod prelude {
	pub use crate::{
		AppSystems, EditorExtension, EditorExtensionContext, EditorExtensionPlugin, EditorPlugin,
		ui::{EditorUi, EditorUiWorld},
		util::{
			AppExtensions, NoParams, TypeGroups, TypeList,
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
	default_plugins: Option<AppRegistrationFn>,
	camera_registrations: Vec<fn(&mut App)>,
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
			// bevy
			.try_add_plugin(MeshPickingPlugin)
			.try_add_plugin(DebugPickingPlugin)
			.try_add_plugin(SystemInformationDiagnosticsPlugin)
			.try_add_plugin(EntityCountDiagnosticsPlugin::default())
			.try_add_plugin(FrameTimeDiagnosticsPlugin::default())
			.try_add_plugin(FrameTimeGraphPlugin)
			.try_add_plugin(RemotePlugin::default())
			.try_add_plugin(RemoteHttpPlugin::default())
			.try_add_plugin(
				NotificationPlugin::<EditorInternalFilter>::default().in_schedule(EditorUiEguiContextPass),
			)
			// crates
			.try_add_plugin(InfiniteGridPlugin)
			.try_add_plugin(OutlinePlugin)
			.try_add_plugin(TransformGizmoPlugin)
			.try_add_plugin(PrefabPlugin::default())
			// internal
			.add_plugins(private::InternalPlugin);
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

	pub fn register_ui<U: EditorUiWorld>(&mut self) -> &mut Self {
		self.ui_manager.register::<U>();
		self
	}
}
