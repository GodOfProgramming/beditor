pub mod assets;
pub mod cam;
pub mod ext;
pub mod input;
pub mod reflection;
pub mod scene;
pub mod ui;
pub mod util;

use crate::{
	AppSystems, EditorExtensionPlugin, EditorState, SimulationState,
	reg::{components::ComponentRegistry, serde::SerdeRegistry},
	storage::{
		GlobalEditorSettings,
		settings::{WindowMaximizedSetting, WindowSizeSetting},
	},
};
use bevy::{
	dev_tools::states::log_transitions,
	ecs::{entity_disabling::DefaultQueryFilters, system::NonSendMarker},
	prelude::*,
	window::{CursorOptions, PrimaryWindow, WindowCloseRequested},
	winit::WINIT_WINDOWS,
};
use bevy_infinite_grid::InfiniteGridBundle;

pub struct InternalPlugin;

impl Plugin for InternalPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<ComponentRegistry>()
			.init_resource::<SerdeRegistry>()
			.add_plugins((
				scene::EditorScenePlugin,
				cam::EditorCamPlugin,
				input::EditorInputPlugin,
				ui::EditorUiPlugin,
				reflection::ReflectionExtensionsPlugin,
				assets::AssetsPlugin,
				EditorExtensionPlugin::<ext::InternalEditorExtensions>::default(),
			))
			.add_systems(
				Startup,
				(spawn_scene, configure_windows, auto_register_components),
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
			.add_systems(OnEnter(EditorState::Exiting), finish_exit);
	}

	fn finish(&self, app: &mut App) {
		let mut schedules = app.world_mut().resource_mut::<Schedules>();

		for (_, schedule) in schedules.iter_mut() {
			schedule.configure_sets(
				AppSystems.run_if(in_state(EditorState::Simulating(SimulationState::Live))),
			);
		}

		if cfg!(feature = "editor-dev") {
			warn!("editor-dev feature flag enabled, this should only be used when developing the editor");
		} else {
			let internal_component = app.world_mut().register_component::<EditorInternal>();
			let mut defaults = app.world_mut().resource_mut::<DefaultQueryFilters>();
			defaults.register_disabling_component(internal_component);
		}
	}
}

/// For entities that do not need to be found by other crates
#[derive(Component, Reflect, Default)]
#[require(EditorOwned)]
pub struct EditorInternal;

/// For entities that might need to be found by other crates
/// but should not be displayed in some contexts
#[derive(Component, Reflect, Default)]
#[require(EditorOwned)]
pub struct UserHidden;

pub type EditorInternalFilter<F = ()> = (Allow<EditorInternal>, F);

pub type EditorInternalQuery<'w, 's, Q, F = ()> = Query<'w, 's, Q, EditorInternalFilter<F>>;

pub type EditorInternalSingle<'w, 's, Q, F = ()> = Single<'w, 's, Q, EditorInternalFilter<F>>;

#[derive(Component)]
#[require(
  UserHidden,
  SceneRoot,
  Name = Name::new("Editor Scene")
)]
pub struct EditorScene;

/// Entities that are owned by the editor
#[derive(Component, Reflect, Default)]
pub struct EditorOwned;

/// Entities that are spawned during simulation
#[derive(Component, Reflect)]
pub struct SimulationOwned;

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

fn spawn_scene(mut commands: Commands) {
	commands.spawn((
		EditorScene,
		Children::spawn(Spawn((
			UserHidden,
			Name::new("Infinite Grid"),
			InfiniteGridBundle::default(),
		))),
	));
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
