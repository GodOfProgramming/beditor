use super::{LayoutInfo, TabState, VTable};
use crate::{
	EditorUi, EditorUiWorld, UiManager,
	inspector::{
		InspectorPrimitive,
		ui::{ImmutableContext, InspectorUi, MutableContext},
	},
	private::{EditorInternal, EditorInternalFilter, EditorInternalQuery},
};
use bevy::{
	ecs::{
		component::Mutable,
		system::{SystemParam, SystemState},
	},
	platform::collections::HashMap,
	prelude::*,
};
use derive_more::derive::Deref;
use derive_new::new;
use egui::text::LayoutJob;
use egui_dock::DockState;
use egui_file_dialog::FileDialog;
use persistent_id::PersistentId;
use std::borrow::Borrow;
use uuid::{Uuid, uuid};

#[derive(Component, Default, Reflect)]
pub struct UiState {
	hovered: bool,
	was_hovered: bool,
}

impl UiState {
	pub fn hovered(&self) -> bool {
		self.hovered
	}

	pub fn mark_hovered(&mut self) {
		self.hovered = true;
	}

	pub fn clear(&mut self) {
		self.was_hovered = self.hovered;
		self.hovered = false;
	}
}

impl InspectorPrimitive for UiState {
	fn ui<'c>(
		&self,
		ui: &mut egui::Ui,
		_: &dyn std::any::Any,
		_: egui::Id,
		_: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		let mut hovered = self.was_hovered;
		ui.add_enabled(false, egui::Checkbox::new(&mut hovered, "hovered"));
	}

	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		options: &dyn std::any::Any,
		id: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		env.as_immutable(|env| {
			Self::ui(self, ui, options, id, &env);
		});
		false
	}
}

pub(crate) trait EditorUiExtensions {
	const VTABLE: VTable;
}

impl<T> EditorUiExtensions for T
where
	T: EditorUiWorld,
{
	const VTABLE: VTable = VTable::new::<Self>();
}

type UiParams<'w, 's, T> = UiComponentState<<T as EditorUi>::Params<'w, 's>>;

/// # Safety
/// Cannot access the world mutably in the system params
/// Though it is on the user to not query for a mutable reference to themselves when they also have a self reference
pub unsafe trait UiExtensions: EditorUi {
	fn with_entity_params<T>(
		entity: Entity,
		world: &mut World,
		f: impl FnOnce(&mut Self, Self::Params<'_, '_>) -> T,
	) -> T
	where
		Self: Component<Mutability = Mutable>,
	{
		let mut q = world.query_filtered::<(&mut Self, &mut UiParams<Self>), EditorInternalFilter>();

		let world_cell = world.as_unsafe_world_cell();
		let Ok((mut this, mut params)) = q.get_mut(unsafe { world_cell.world_mut() }, entity) else {
			// # Safety
			// This is an error path and we'll be crashing after regardless
			let mut q = unsafe {
				world_cell
					.world_mut()
					.query_filtered::<(Has<Self>, Has<UiParams<Self>>), EditorInternalFilter>()
			};
			match q.get(unsafe { world_cell.world() }, entity) {
				Ok((has_self, has_params)) => {
					panic!(
						"Failed to query {}: has self: {has_self}, has params: {has_params}",
						<Self as EditorUi>::NAME,
					);
				}
				Err(err) => {
					panic!("Failed to query {}: {err}", <Self as EditorUi>::NAME);
				}
			}
		};

		let items = params.get_mut(unsafe { world_cell.world_mut() });
		let result = f(this.as_mut(), items);
		unsafe { params.apply(world_cell.world_mut()) };
		result
	}

	fn register_params(entity: Entity, world: &mut World) {
		if !world.entity(entity).contains::<UiParams<Self>>() {
			let state = SystemState::<Self::Params<'_, '_>>::new(world);
			world
				.entity_mut(entity)
				.insert(UiParams::<Self>::new(state));
		}
	}

	fn with_params<T>(
		entity: Entity,
		world: &mut World,
		f: impl FnOnce(Self::Params<'_, '_>) -> T,
	) -> T {
		let world_cell = world.as_unsafe_world_cell();
		let mut entity = unsafe { world_cell.world_mut() }.entity_mut(entity);
		let mut params = entity.get_mut::<UiParams<Self>>().unwrap();
		let params = params.get_mut(unsafe { world_cell.world_mut() });
		f(params)
	}
}

unsafe impl<T> UiExtensions for T where Self: EditorUi {}

#[derive(new, Component, Deref, DerefMut)]
struct UiComponentState<P>(SystemState<P>)
where
	P: SystemParam + 'static;

#[derive(new, Resource, Deref, DerefMut)]
pub struct UiResourceState<P>(SystemState<P>)
where
	P: SystemParam + 'static;

#[derive(Component, Reflect, Default)]
#[require(EditorInternal)]
pub struct MissingUi {
	message: String,
	id: PersistentId,
	name: String,
}

impl MissingUi {
	pub fn new(name: impl Into<String>, id: impl Into<PersistentId>) -> Self {
		let id = id.into();
		let name = name.into();
		Self {
			message: format!("Failed to find ui component {name} with uuid: {}", *id),
			id,
			name,
		}
	}
	pub fn id(&self) -> &PersistentId {
		&self.id
	}
}

impl EditorUi for MissingUi {
	const NAME: &str = "Missing Ui";
	const ID: Uuid = uuid!("d0f32ae1-2851-4bcd-a0c9-f83ae030d85f");

	type Params<'w, 's> = common::NoParams;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		default()
	}

	const HIDDEN: bool = true;

	const UNIQUE: bool = true;

	fn ui(&mut self, ui: &mut egui::Ui, _params: Self::Params<'_, '_>) {
		let mut job = LayoutJob::single_section(self.message.to_owned(), egui::TextFormat::default());
		job.wrap = egui::text::TextWrapping::default();
		ui.label(job);
	}
}

pub(crate) trait DockExtensions:
	Borrow<DockState<TabState>> + From<DockState<TabState>>
{
	fn decouple(
		&self,
		ui_manager: &UiManager,
		q_persistent_ids: &EditorInternalQuery<&PersistentId, Without<MissingUi>>,
		q_missing: &EditorInternalQuery<&MissingUi>,
	) -> DockState<LayoutInfo> {
		self.borrow().map_tabs(|tab| {
			let id;
			let name;

			if let Ok(missing_uuid) = q_missing.get(tab.entity) {
				id = *missing_uuid.id();
				name = missing_uuid.name.clone();
			} else {
				id = *q_persistent_ids.get(tab.entity).unwrap();
				name = ui_manager
					.vtables
					.get(&id)
					.cloned()
					.map(|vt| vt.name.to_string())
					.unwrap_or_default();
			}

			LayoutInfo::new(id, name)
		})
	}

	fn restore(
		dock: DockState<LayoutInfo>,
		vtables: &HashMap<PersistentId, &'static VTable>,
		world: &mut World,
	) -> Self {
		dock
			.filter_map_tabs(|layout_info| {
				let Some(vtable) = vtables.get(&layout_info.id()) else {
					let name = layout_info.name();
					let state = SystemState::<<MissingUi as EditorUi>::Params<'_, '_>>::new(world);

					warn!(
						"Failed to find ui component {name} with uuid {}",
						*layout_info.id()
					);

					let entity = world
						.spawn((
							Name::new(<MissingUi as EditorUiWorld>::NAME),
							MissingUi::new(name, layout_info.id()),
							PersistentId(<MissingUi as EditorUiWorld>::ID),
							UiState::default(),
							UiComponentState(state),
						))
						.id();

					return Some(TabState {
						entity,
						vtable: &MissingUi::VTABLE,
					});
				};

				if vtable.reopen_on_startup {
					let entity = (vtable.spawn)(world);
					Some(TabState { entity, vtable })
				} else {
					None
				}
			})
			.into()
	}
}

impl DockExtensions for DockState<TabState> {}

#[derive(Deref, DerefMut)]
pub struct CenteredFileDialog(FileDialog);

impl Default for CenteredFileDialog {
	fn default() -> Self {
		Self(
			FileDialog::default()
				.as_modal(true)
				.anchor(egui::Align2::CENTER_CENTER, egui::Vec2::default()),
		)
	}
}
