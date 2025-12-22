use super::{EditorUi, EditorUiBundle, VTable};
use crate::{NoParams, UiManager, ui::TabState, util::storage::LayoutInfo};
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
use persistent_id::PersistentId;
use std::borrow::Borrow;
use uuid::{Uuid, uuid};

#[derive(Component, Default)]
pub struct UiState {
	pub(super) hovered: bool,
}

impl UiState {
	pub fn hovered(&self) -> bool {
		self.hovered
	}
}

pub(crate) trait EditorUiExtensions {
	const VTABLE: VTable;
}

impl<T> EditorUiExtensions for T
where
	T: EditorUiBundle,
{
	const VTABLE: VTable = VTable::new::<Self>();
}

type UiParams<'w, 's, T> = UiComponentState<<T as EditorUi>::Params<'w, 's>>;

/// # Safety
/// Cannot access the world mutably in the system params
/// Though it is on the user to not query for a mutable reference to themselves when they also have a self reference
pub unsafe trait UiExtensions: EditorUi {
	fn get_entity_mut<T>(
		entity: Entity,
		world: &mut World,
		f: impl FnOnce(&mut Self, Self::Params<'_, '_>) -> T,
	) -> T
	where
		Self: Component<Mutability = Mutable>,
	{
		let mut q = world.query::<(&mut Self, &mut UiParams<Self>)>();
		let world_cell = world.as_unsafe_world_cell();
		let Ok((mut this, mut params)) = q.get_mut(unsafe { world_cell.world_mut() }, entity) else {
			panic!("Failed to query {}", <Self as EditorUi>::NAME);
		};

		let items = params.get_mut(unsafe { world_cell.world_mut() });
		let result = f(this.as_mut(), items);
		unsafe { params.apply(world_cell.world_mut()) };
		result
	}

	fn register_params(entity: Entity, world: &mut World) {
		if !world.entity(entity).contains::<UiParams<Self>>() {
			let state = SystemState::<<Self as EditorUi>::Params<'_, '_>>::new(world);
			world.entity_mut(entity).insert(UiComponentState(state));
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

#[derive(Component, Deref, DerefMut)]
struct UiComponentState<P>(SystemState<P>)
where
	P: SystemParam + 'static;

#[derive(new, Resource, Deref, DerefMut)]
pub struct UiResourceState<P>(SystemState<P>)
where
	P: SystemParam + 'static;

#[derive(Component, Reflect, Default)]
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

	type Params<'w, 's> = NoParams;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		default()
	}

	const HIDDEN: bool = true;

	const UNIQUE: bool = true;

	fn render(&mut self, ui: &mut egui::Ui, _params: Self::Params<'_, '_>) {
		let mut job = LayoutJob::single_section(self.message.to_owned(), egui::TextFormat::default());
		job.wrap = egui::text::TextWrapping::default();
		ui.label(job);
	}
}

pub(super) trait DockExtensions:
	Borrow<DockState<TabState>> + From<DockState<TabState>>
{
	fn decouple(
		&self,
		ui_manager: &UiManager,
		q_persistent_ids: &Query<&PersistentId, Without<MissingUi>>,
		q_missing: &Query<&MissingUi>,
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
					.get_vtable_by_id(&id)
					.map(|vt| vt.name.to_string())
					.unwrap_or_default();
			}

			LayoutInfo::new(id, name)
		})
	}

	fn restore(
		dock: &DockState<LayoutInfo>,
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
							Name::new(<MissingUi as EditorUiBundle>::NAME),
							MissingUi::new(name, layout_info.id()),
							PersistentId(<MissingUi as EditorUiBundle>::ID),
							UiState::default(),
							UiComponentState(state),
						))
						.id();

					return Some(TabState::new(entity, &MissingUi::VTABLE));
				};

				if vtable.reopen_on_startup {
					let entity = (vtable.spawn)(world);
					Some(TabState::new(entity, vtable))
				} else {
					None
				}
			})
			.into()
	}
}

impl DockExtensions for DockState<TabState> {}
