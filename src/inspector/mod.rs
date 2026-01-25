//! All credit for this goes to https://github.com/jakobhellermann/bevy-inspector-egui

pub mod errors;
pub mod options;
pub mod ui;
pub mod world_view;

use bevy::{
	asset::{ReflectAsset, UntypedAssetId},
	ecs::query::QueryFilter,
	prelude::*,
	reflect::TypeRegistry,
};
use common::extensions::bevy::WorldMutExtensions as _;
use std::{
	any::{Any, TypeId},
	borrow::BorrowMut,
};
use ui::{
	ImmutableContext, InspectorUi, InspectorUiVTable, MutableContext, components, hierarchy,
	hierarchy::SelectedEntities,
};
use world_view::{MutableWorldView, RestrictedWorldView};

pub fn add<T: InspectorPrimitive + TypePath + PartialEq + Clone + Default>(
	type_registry: &mut TypeRegistry,
) {
	type_registry
		.get_mut(TypeId::of::<T>())
		.unwrap_or_else(|| panic!("{} not registered", std::any::type_name::<T>()))
		.insert(InspectorUiVTable::new::<T>());
}

pub fn add_single<T: InspectorPrimitive>(type_registry: &mut TypeRegistry) {
	type_registry
		.get_mut(TypeId::of::<T>())
		.unwrap_or_else(|| panic!("{} not registered", std::any::type_name::<T>()))
		.insert(InspectorUiVTable::new_single::<T>());
}

pub fn add_multiedit<T: InspectorPrimitiveMultiedit + TypePath>(type_registry: &mut TypeRegistry) {
	type_registry
		.get_mut(TypeId::of::<T>())
		.unwrap_or_else(|| panic!("{} not registered", std::any::type_name::<T>()))
		.insert(InspectorUiVTable::new_many::<T>());
}

pub trait InspectorPrimitive: Reflect {
	fn ui<'c>(
		&self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: &InspectorUi<'_, ImmutableContext<'c>>,
	);

	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool;
}

pub trait InspectorPrimitiveMultiedit: Reflect {
	fn ui<'c>(
		&self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: &InspectorUi<'_, ImmutableContext<'c>>,
	);

	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool;

	fn ui_mut_multiedit<'s, 'c>(
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
		values: impl Iterator<Item = &'s mut Self>,
	) -> bool
	where
		Self: 's;
}

impl<T> InspectorPrimitive for T
where
	Self: InspectorPrimitiveMultiedit,
{
	fn ui<'c>(
		&self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		<Self as InspectorPrimitiveMultiedit>::ui(self, ui, options, id, env);
	}

	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		<Self as InspectorPrimitiveMultiedit>::ui_mut(self, ui, options, id, env)
	}
}

pub trait WorldExtensions: BorrowMut<World> {
	fn ui_for_value(&mut self, ui: &mut egui::Ui, value: &dyn PartialReflect) {
		let world = self.borrow_mut();
		world.queue(|world, queue| {
			let type_registry = world.resource::<AppTypeRegistry>().0.clone();
			let type_registry = type_registry.read();

			let ctx = ImmutableContext::new(world, queue);
			let env = InspectorUi::new(&type_registry, &ctx);
			env.ui_for_reflect(value, ui);
		});
	}

	fn ui_for_value_mut(&mut self, ui: &mut egui::Ui, value: &mut dyn PartialReflect) -> bool {
		self.borrow_mut().queue(|world, queue| {
			let type_registry = world.resource::<AppTypeRegistry>().0.clone();
			let type_registry = type_registry.read();

			let mut ctx = MutableContext::new(world, queue);
			let mut env = InspectorUi::new(&type_registry, &mut ctx);
			env.ui_for_reflect_mut(value, ui)
		})
	}

	fn hierarchy_ui<QF: QueryFilter, P>(
		&mut self,
		ui: &mut egui::Ui,
		selected: &mut SelectedEntities,
		dnd: hierarchy::DndHandlerFn<P>,
	) -> Option<egui::CollapsingResponse<Entity>>
	where
		P: 'static + Send + Sync,
	{
		let world = self.borrow_mut();
		hierarchy::Hierarchy {
			world,
			selected,
			dnd,
		}
		.show::<QF>(ui)
	}

	fn ui_for_entity(
		&mut self,
		entity: Entity,
		ui: &mut egui::Ui,
		highlight_changes: bool,
	) -> Option<egui::CollapsingResponse<components::ComponentInfo>> {
		let world = self.borrow_mut();

		world.queue(|world, queue| {
			let type_registry = world.resource::<AppTypeRegistry>().0.clone();
			let type_registry = type_registry.read();

			let entity_name = common::ecs::guess_entity_name(world, entity);
			ui.label(entity_name);

			let mut ctx = MutableContext::new(world, queue);

			components::ui_for_entity_components(
				&mut ctx,
				entity,
				ui,
				egui::Id::new(entity),
				&type_registry,
				highlight_changes,
			)
		})
	}

	fn ui_for_entities(
		&mut self,
		ui: &mut egui::Ui,
		entities: &[Entity],
	) -> Option<egui::CollapsingResponse<components::ComponentInfo>> {
		let world = self.borrow_mut();
		components::ui_for_entities_with_shared_components(world, entities, ui)
	}

	fn ui_for_resource<R: Resource + Reflect>(&mut self, ui: &mut egui::Ui) {
		self.borrow_mut().queue(|world, queue| {
			let type_registry = world.resource::<AppTypeRegistry>().0.clone();
			let type_registry = type_registry.read();

			// create a context with access to the world except for the `R` resource
			let Some((mut resource, world_view)) =
				RestrictedWorldView::<MutableWorldView>::from(world).split_off_resource_typed::<R>()
			else {
				errors::resource_does_not_exist(ui, common::types::pretty_name::<R>());
				return;
			};

			let mut ctx = MutableContext::from_world_view(world_view, queue);
			let mut env = InspectorUi::new(&type_registry, &mut ctx);

			if env.ui_for_reflect_mut(resource.bypass_change_detection(), ui) {
				resource.set_changed();
			}
		});
	}

	fn ui_for_resource_type(
		&mut self,
		ui: &mut egui::Ui,
		type_registry: &TypeRegistry,
		resource_type_id: TypeId,
		name_of_type: &str,
	) {
		self.borrow_mut().queue(|world, queue| {
			// create a context with access to the world except for the current resource
			let world_view = RestrictedWorldView::<MutableWorldView>::from(world);
			let Some((mut resource_view, world_view)) = world_view.split_off_resource(resource_type_id)
			else {
				errors::no_access_resource(ui, name_of_type);
				return;
			};

			let mut ctx = MutableContext::from_world_view(world_view, queue);
			let mut env = InspectorUi::new(type_registry, &mut ctx);

			let resource_result =
				resource_view.resource_reflect_mut_by_id(resource_type_id, type_registry);
			let mut resource = common::match_else!(resource_result; else err => {
				errors::show_error(err, ui, name_of_type);
				return;
			});

			let changed = env.ui_for_reflect_mut(
				resource.bypass_change_detection().as_partial_reflect_mut(),
				ui,
			);
			if changed {
				resource.set_changed();
			}
		});
	}

	fn ui_for_asset(
		&mut self,
		ui: &mut egui::Ui,
		type_registry: &TypeRegistry,
		asset_type_id: TypeId,
		asset_id: UntypedAssetId,
	) -> bool {
		self.borrow_mut().queue(|world, queue| {
			let Some(registration) = type_registry.get(asset_type_id) else {
				errors::reflect::not_in_type_registry(
					ui,
					&errors::name_of_type(asset_type_id, type_registry),
				);
				return false;
			};

			let Some(reflect_asset) = registration.data::<ReflectAsset>() else {
				errors::no_type_data(
					ui,
					&errors::name_of_type(asset_type_id, type_registry),
					"ReflectAsset",
				);
				return false;
			};

			let world = RestrictedWorldView::<MutableWorldView>::from(world);

			let Some((assets_view, world_view)) =
				world.split_off_resource(reflect_asset.assets_resource_type_id())
			else {
				let type_name = common::types::pretty_name_of_str(registration.type_info().type_path());
				errors::no_access_resource(ui, type_name);
				return false;
			};

			let Some(asset_value) = ({
				assert!(assets_view.allows_access_to_resource(reflect_asset.assets_resource_type_id()));
				// SAFETY: the world allows mutable access to `Assets<T>`
				unsafe { reflect_asset.get_unchecked_mut(world_view.world_cell(), asset_id) }
			}) else {
				errors::dead_asset_handle(ui, asset_id);
				return false;
			};

			let mut ctx = MutableContext::copy_from(&world_view, queue);

			let id = egui::Id::new(asset_id);

			let mut env = InspectorUi::new(type_registry, &mut ctx);
			env.ui_for_reflect_mut_with_options(asset_value.as_partial_reflect_mut(), ui, id, &())
		})
	}
}

impl<T> WorldExtensions for T where T: BorrowMut<World> {}
