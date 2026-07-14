use crate::private::{
	ui::{NewTabs, TabState, UiManager, misc::UiExtensions},
	util::WorldExtensions,
};
use bevy::{
	ecs::{component::Mutable, system::SystemParam},
	prelude::*,
};
use egui_dock::{NodeIndex, SurfaceIndex, TabIndex};
use notify::Notification;
use uuid::Uuid;

pub trait EditorUiWorld: Bundle + Send + Sync + Sized {
	type MarkerComponent: Component;

	const NAME: &str;
	const ID: Uuid;

	/// Used to prevent this Ui from appearing in the view menu
	const HIDDEN: bool = false;

	const CLOSEABLE: bool = true;

	const CAN_CLEAR: bool = true;

	const SCROLL_BARS: [bool; 2] = [true, true];

	const UNIQUE: bool = false;

	const POPOUT: bool = true;

	const REOPEN_ON_STARTUP: bool = true;

	fn spawn(entity: Entity, world: &mut World) -> Result<Self>;

	fn on_despawn(entity: Entity, world: &mut World) -> Result {
		let _ = entity;
		let _ = world;
		Ok(())
	}

	fn title(entity: Entity, world: &mut World) -> Result<egui::WidgetText> {
		let _ = entity;
		let _ = world;
		Ok(Self::NAME.into())
	}

	fn ui(entity: Entity, ui: &mut egui::Ui, world: &mut World) -> Result<()>;

	fn context_menu(
		entity: Entity,
		ui: &mut egui::Ui,
		world: &mut World,
		surface: SurfaceIndex,
		node: NodeIndex,
	) -> Result<()> {
		let _ = entity;
		let _ = ui;
		let _ = world;
		let _ = surface;
		let _ = node;
		Ok(())
	}

	fn on_panel_changed(entity: Entity, world: &mut World) -> Result<()> {
		let _ = entity;
		let _ = world;
		Ok(())
	}

	fn handle_tab_response(
		entity: Entity,
		world: &mut World,
		response: &egui::Response,
	) -> Result<()> {
		let _ = entity;
		let _ = world;
		let _ = response;
		Ok(())
	}
}

pub trait EditorUi: EditorUiWorld + Component {
	const NAME: &str;
	const ID: Uuid;

	const HIDDEN: bool = false;

	const CLOSEABLE: bool = true;

	const CAN_CLEAR: bool = true;

	const SCROLL_BARS: [bool; 2] = [true, true];

	const UNIQUE: bool = false;

	const POPOUT: bool = true;

	const REOPEN_ON_STARTUP: bool = true;

	type Params<'w, 's>: for<'world, 'system> SystemParam<
		Item<'world, 'system> = Self::Params<'world, 'system>,
	>;

	fn spawn(params: Self::Params<'_, '_>) -> Self;

	fn init(&mut self, this_entity: Entity, params: Self::Params<'_, '_>) {
		let _ = this_entity;
		let _ = params;
	}

	fn title(&mut self, params: Self::Params<'_, '_>) -> egui::WidgetText {
		let _ = params;
		<Self as EditorUi>::NAME.into()
	}

	fn ui(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>);

	fn context_menu(
		&mut self,
		ui: &mut egui::Ui,
		params: Self::Params<'_, '_>,
		surface: SurfaceIndex,
		node: NodeIndex,
	) {
		let _ = ui;
		let _ = params;
		let _ = surface;
		let _ = node;
	}

	fn handle_tab_response(&mut self, params: Self::Params<'_, '_>, response: &egui::Response) {
		let _ = params;
		let _ = response;
	}

	fn on_panel_changed(&mut self, params: Self::Params<'_, '_>) {
		let _ = params;
	}

	fn on_despawn(&mut self, params: Self::Params<'_, '_>) {
		let _ = params;
	}
}

impl<T> EditorUiWorld for T
where
	Self: Component<Mutability = Mutable> + EditorUi + 'static,
{
	type MarkerComponent = Self;

	const NAME: &str = <Self as EditorUi>::NAME;
	const ID: Uuid = <T as EditorUi>::ID;

	const HIDDEN: bool = <Self as EditorUi>::HIDDEN;

	const CLOSEABLE: bool = <Self as EditorUi>::CLOSEABLE;

	const CAN_CLEAR: bool = <Self as EditorUi>::CAN_CLEAR;

	const SCROLL_BARS: [bool; 2] = <Self as EditorUi>::SCROLL_BARS;

	const UNIQUE: bool = <Self as EditorUi>::UNIQUE;

	const POPOUT: bool = <Self as EditorUi>::POPOUT;

	const REOPEN_ON_STARTUP: bool = <Self as EditorUi>::REOPEN_ON_STARTUP;

	fn spawn(entity: Entity, world: &mut World) -> Result<Self> {
		Self::register_params(entity, world);
		let mut ui = Self::with_params(entity, world, EditorUi::spawn)?;
		Self::with_params(entity, world, |params| {
			EditorUi::init(&mut ui, entity, params)
		})?;
		Ok(ui)
	}

	fn title(entity: Entity, world: &mut World) -> Result<egui::WidgetText> {
		Self::with_entity_params(entity, world, EditorUi::title)
	}

	fn ui(entity: Entity, ui: &mut egui::Ui, world: &mut World) -> Result<()> {
		Self::with_entity_params(entity, world, |this, params| {
			this.ui(ui, params);
		})
	}

	fn context_menu(
		entity: Entity,
		ui: &mut egui::Ui,
		world: &mut World,
		surface: SurfaceIndex,
		node: NodeIndex,
	) -> Result<()> {
		Self::with_entity_params(entity, world, |this, params| {
			this.context_menu(ui, params, surface, node);
		})
	}

	fn handle_tab_response(
		entity: Entity,
		world: &mut World,
		response: &egui::Response,
	) -> Result<()> {
		Self::with_entity_params(entity, world, |this, params| {
			this.handle_tab_response(params, response);
		})
	}

	fn on_panel_changed(entity: Entity, world: &mut World) -> Result<()> {
		Self::with_entity_params(entity, world, <Self as EditorUi>::on_panel_changed)
	}

	fn on_despawn(entity: Entity, world: &mut World) -> Result<()> {
		Self::with_entity_params(entity, world, <Self as EditorUi>::on_despawn)
	}
}

#[derive(Deref, DerefMut)]
pub struct OpenUi(pub(crate) Box<dyn 'static + Send + Sync + FnOnce(&mut World)>);

impl OpenUi {
	pub fn open<T: EditorUiWorld>(mode: OpenMode) -> Self {
		Self::new(move |world| {
			let Ok(tab) = world.notify_on_error(
				|world| TabState::new::<T>(world),
				|_, err| ("Failed to open ui", Some(err)),
			) else {
				return;
			};
			mode.open(world, tab);
		})
	}

	pub fn open_with_value<T: EditorUiWorld>(mode: OpenMode, value: T) -> Self {
		Self::new(move |world| {
			let Ok(tab) = world.notify_on_error(
				|world| TabState::new::<T>(world),
				|_, err| ("Failed to open ui", Some(err)),
			) else {
				return;
			};
			world.entity_mut(tab.entity()).insert(value);
			mode.open(world, tab);
		})
	}

	fn new<F>(f: F) -> Self
	where
		F: 'static + Send + Sync + FnOnce(&mut World),
	{
		Self(Box::new(f))
	}
}

impl Command for OpenUi {
	type Out = ();
	fn apply(self, world: &mut World) {
		world.resource_mut::<NewTabs>().push(self);
	}
}

#[derive(Clone, Copy)]
pub enum OpenMode {
	AppendToFocused,
	Window,
	FocusAt(SurfaceIndex, NodeIndex, TabIndex),
}

impl OpenMode {
	fn open(self, world: &mut World, tab: TabState) {
		let mut ui_manager = world.resource_mut::<UiManager>();

		let success = match self {
			Self::AppendToFocused => ui_manager.add_tab_to_focused(tab),
			Self::Window => {
				ui_manager.add_detached(vec![tab]);
				return;
			}
			Self::FocusAt(surface, node, neighbor) => {
				ui_manager.insert_and_focus(surface, node, neighbor, tab)
			}
		};

		if !success {
			let name = tab.vtable.name;
			world.trigger(Notification::error("Failed to create Ui").with_context(name))
		}
	}
}
