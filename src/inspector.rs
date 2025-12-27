//! All credit for this goes to https://github.com/jakobhellermann/bevy-inspector-egui

mod data;
pub mod errors;
pub mod options;
pub mod ui;

use crate::{
	TypeGroups, TypeList,
	inspector::ui::{
		MutableContext,
		hierarchy::{SelectedEntities, SelectedEntitiesChangedEvent},
	},
	util::{self, AppExtensions, WorldExtensions as _, world::RestrictedWorldView},
};
use bevy::{
	asset::{ReflectAsset, ReflectHandle, UntypedAssetId},
	ecs::query::QueryFilter,
	prelude::*,
	reflect::TypeRegistry,
};
use std::{
	any::TypeId,
	borrow::{Borrow, BorrowMut},
};
use ui::{InspectorUi, components, hierarchy};

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
	fn build(&self, app: &mut App) {
		app
			.register_types::<(
				// math
				TypeGroups<(
					(bevy::math::IVec2, bevy::math::IVec3, bevy::math::IVec4),
					(bevy::math::UVec2, bevy::math::UVec3, bevy::math::UVec4),
					(bevy::math::DVec2, bevy::math::DVec3, bevy::math::DVec4),
					(
						bevy::math::BVec2,
						bevy::math::BVec3,
						bevy::math::BVec3A,
						bevy::math::BVec4,
						bevy::math::BVec4A,
					),
					(
						bevy::math::Vec2,
						bevy::math::Vec3,
						bevy::math::Vec3A,
						bevy::math::Vec4,
					),
					(
						bevy::math::DAffine2,
						bevy::math::DAffine3,
						bevy::math::Affine2,
						bevy::math::Affine3A,
					),
					(bevy::math::DMat2, bevy::math::DMat3, bevy::math::DMat4),
					(
						bevy::math::Mat2,
						bevy::math::Mat3,
						bevy::math::Mat3A,
						bevy::math::Mat4,
					),
					(bevy::math::DQuat, bevy::math::Quat, bevy::math::Rect),
				)>,
				// misc
				TypeList<(bevy::color::Color, core::ops::Range<f32>, TypeId)>,
			)>()
			.add_observer(SelectedEntitiesChangedEvent::on_event);

		let type_registry = app.world().resource::<AppTypeRegistry>();
		let mut type_registry = type_registry.write();

		data::register_type_data(&mut type_registry);
	}
}

pub trait TypeRegistryExtensions: Borrow<TypeRegistry> {
	fn ui_for_value(&self, ui: &mut egui::Ui, value: &mut dyn PartialReflect) -> bool {
		let type_registry = self.borrow();
		InspectorUi::new(type_registry, None).ui_for_reflect(value, ui)
	}

	fn ui_for_value_readonly(&self, ui: &mut egui::Ui, value: &dyn PartialReflect) {
		let type_registry = self.borrow();
		InspectorUi::new(type_registry, None).ui_for_reflect_readonly(value, ui);
	}

	fn show_docs(&self, response: egui::Response, type_id: TypeId) {
		let type_registry = self.borrow();
		let type_docs = type_registry
			.get_type_info(type_id)
			.and_then(|info| info.docs());

		util::egui::show_docs(response, type_docs);
	}
}

impl<T> TypeRegistryExtensions for T where T: Borrow<TypeRegistry> {}

pub trait WorldExtensions: BorrowMut<World> {
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

			let entity_name = util::entity::guess_entity_name(world, entity);
			ui.label(entity_name);

			let mut ctx = MutableContext::new(RestrictedWorldView::new(world), queue);

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
		let world = self.borrow_mut();

		world.queue(|world, queue| {
			let type_registry = world.resource::<AppTypeRegistry>().0.clone();
			let type_registry = type_registry.read();

			// create a context with access to the world except for the `R` resource
			let Some((mut resource, world_view)) =
				RestrictedWorldView::new(world).split_off_resource_typed::<R>()
			else {
				errors::resource_does_not_exist(ui, &util::pretty_type_name::<R>());
				return;
			};

			let mut cx = MutableContext::new(world_view, queue);
			let mut env = InspectorUi::new(&type_registry, Some(&mut cx));

			if env.ui_for_reflect(resource.bypass_change_detection(), ui) {
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
		let world = self.borrow_mut();

		world.queue(|world, queue| {
			// create a context with access to the world except for the current resource
			let world_view = RestrictedWorldView::new(world);
			let (mut resource_view, world_view) = world_view.split_off_resource(resource_type_id);

			let mut cx = MutableContext::new(world_view, queue);
			let mut env = InspectorUi::new(type_registry, Some(&mut cx));

			let mut resource =
				match resource_view.get_resource_reflect_mut_by_id(resource_type_id, type_registry) {
					Ok(resource) => resource,
					Err(err) => return errors::show_error(err, ui, name_of_type),
				};

			let changed = env.ui_for_reflect(
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
		handle: UntypedAssetId,
	) -> bool {
		let world = self.borrow_mut();

		world.queue(|world, queue| {
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
			let Some(reflect_handle) =
				type_registry.get_type_data::<ReflectHandle>(reflect_asset.handle_type_id())
			else {
				errors::no_type_data(
					ui,
					&errors::name_of_type(reflect_asset.handle_type_id(), type_registry),
					"ReflectHandle",
				);
				return false;
			};

			let _: Vec<_> = reflect_asset.ids(world).collect();

			// Create a context with access to the entire world. Displaying the `Handle<T>` will short circuit into
			// displaying the T with a world view excluding Assets<T>.
			let world_view = RestrictedWorldView::new(world);
			let mut cx = MutableContext::new(world_view, queue);

			let id = egui::Id::new(handle);

			if let UntypedAssetId::Uuid { uuid, type_id } = handle {
				let mut handle = reflect_handle
					.typed(UntypedHandle::Uuid { uuid, type_id })
					.into_partial_reflect();

				let mut env = InspectorUi::new(type_registry, Some(&mut cx));
				let changed = env.ui_for_reflect_with_options(&mut *handle, ui, id, &());

				changed
			} else {
				false
			}
		})
	}

	fn ui_for_value(&mut self, ui: &mut egui::Ui, value: &mut dyn Reflect) -> bool {
		let world = self.borrow_mut();

		world.queue(|world, queue| {
			let type_registry = world.resource::<AppTypeRegistry>().0.clone();
			let type_registry = type_registry.read();

			let mut cx = MutableContext::new(RestrictedWorldView::new(world), queue);
			let mut env = InspectorUi::new(&type_registry, Some(&mut cx));

			env.ui_for_reflect(value.as_partial_reflect_mut(), ui)
		})
	}
}

impl<T> WorldExtensions for T where T: BorrowMut<World> {}
