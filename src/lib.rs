#![expect(
	clippy::type_complexity,
	reason = "For queries that may actually be more readable without abstraction"
)]
#![expect(
	clippy::too_many_arguments,
	reason = "For systems that may actually be more readable without abstraction"
)]

pub mod content;
pub mod inspector;
mod private;
pub mod reg;
pub mod storage;
pub mod ui;

use crate::{
	private::{
		EditorInternalFilter, ext::game_camera_view::GameCameraViewExtension,
		ui::EditorUiEguiContextPass,
	},
	reg::components::{ComponentRegistry, RegisterableComponent, RegisterableComponents},
	storage::{Global, Project, Settings},
};
use bevy::{
	app::{PluginGroupBuilder, plugin_group},
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
use brefabs::PrefabPlugin;
use common::extensions::bevy::{AppExtensions as _, WorldMutExtensions as _};
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
		EditorState,
		ui::{EditorUi, EditorUiWorld},
	};
	pub use bevy_egui;
	pub use brefabs;
	pub use common::NoParams;
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

type AppRegistrationFn = Box<dyn Send + Sync + Fn(&mut App)>;
type PrefabModFn = Box<dyn Send + Sync + Fn(&mut PrefabPlugin)>;

#[derive(Default)]
pub struct EditorPlugin {
	default_plugins: Option<AppRegistrationFn>,
	camera_registrations: Vec<fn(&mut App)>,
	prefab_mods: Vec<PrefabModFn>,
}

impl EditorPlugin {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn with_prefabs(mut self, f: impl 'static + Send + Sync + Fn(&mut PrefabPlugin)) -> Self {
		self.prefab_mods.push(Box::new(f));
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

	pub fn register_game_camera<C: Component + Identifiable>(mut self) -> Self {
		self.camera_registrations.push(|app| {
			app.add_plugins(EditorExtensionPlugin::<GameCameraViewExtension<C>>::default());
		});
		self
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

		let mut prefabs = PrefabPlugin::default();
		for f in self.prefab_mods.iter() {
			(f)(&mut prefabs);
		}

		app
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
			// internal
			.add_plugins((private::InternalPlugin, prefabs));
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
			.resources_scope::<(UiManager, ComponentRegistry)>(|world, resources| {
				let (ui_manager, components) = resources;
				let mut ctx = EditorExtensionContext::new(world, components, ui_manager);
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
	ui_manager: &'w mut UiManager,
}

impl<'w> EditorExtensionContext<'w> {
	pub fn world(&mut self) -> &mut World {
		self.world
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

plugin_group! {
		/// Plugins that are needed to leverage editor created assets in a game, without including the editor
		pub struct StandalonePlugins {
			private::assets:::AssetsPlugin,
		}
}
