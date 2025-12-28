use crate::{
	inspector::{
		errors,
		ui::{ImmutableContext, InspectorUi, MutableContext},
	},
	util::{
		self, WorldExtensions,
		egui::{CollapsingResponseExtensions, ResponseConditions, set_highlight_style},
		world::{ReflectBorrow, RestrictedWorldView, WorldView},
	},
};
use bevy::{ecs::component::ComponentId, prelude::*, reflect::TypeRegistry};
use std::any::TypeId;

pub struct ComponentInfo {
	pub changed: bool,
	pub type_id: TypeId,
	pub component_id: ComponentId,
}

impl ComponentInfo {
	fn from_response(
		response: egui::Response,
		changed: bool,
		type_id: TypeId,
		component_id: ComponentId,
	) -> Option<egui::CollapsingResponse<Self>> {
		if Self::satisfies_response(&response) {
			Some(egui::CollapsingResponse {
				header_response: response,
				body_response: None,
				body_returned: Some(ComponentInfo {
					changed,
					type_id,
					component_id,
				}),
				openness: 0.0,
			})
		} else {
			None
		}
	}

	fn from_collapsing<T>(
		response: egui::CollapsingResponse<T>,
		changed: bool,
		type_id: TypeId,
		component_id: ComponentId,
	) -> Option<egui::CollapsingResponse<Self>> {
		if Self::satisfies_response(&response.header_response)
			|| response
				.body_response
				.as_ref()
				.map(Self::satisfies_response)
				.unwrap_or(false)
		{
			Some(egui::CollapsingResponse {
				header_response: response.header_response,
				body_response: response.body_response,
				body_returned: Some(ComponentInfo {
					changed,
					type_id,
					component_id,
				}),
				openness: response.openness,
			})
		} else {
			None
		}
	}

	fn satisfies_response(response: &egui::Response) -> bool {
		ResponseConditions::from(response).any()
	}
}

pub fn ui_for_entity_components(
	ctx: &mut MutableContext<'_>,
	entity: Entity,
	ui: &mut egui::Ui,
	id: egui::Id,
	type_registry: &TypeRegistry,
	highlight_changes: bool,
) -> Option<egui::CollapsingResponse<ComponentInfo>> {
	let Ok(components) = components_of_entity(&ctx.world_view, entity) else {
		errors::entity_does_not_exist(ui, entity);
		return None;
	};

	let mut clicked_header = None;

	for (name, component_id, type_id, size) in components {
		let id = id.with(component_id);

		let header = egui::CollapsingHeader::new(&name).id_salt(id);

		let Some(type_id) = type_id else {
			header.show(ui, |ui| errors::no_type_id(ui, &name));
			continue;
		};

		if size == 0 {
			let response = ui.indent(id, |ui| ui.label(&name));

			clicked_header.maybe_take(ComponentInfo::from_response(
				response.inner,
				false,
				type_id,
				component_id,
			));

			continue;
		}

		// create a context with access to the world except for the currently viewed component
		let (mut component_view, world_view) = ctx.world_view.split_off_component((entity, type_id));

		let value = match component_view.entity_component_reflect_mut(entity, type_id, type_registry) {
			Ok(value) => value,
			Err(_) => {
				let response = ui.indent(id, |ui| {
					ui.label(egui::RichText::new(&name).underline())
						.on_hover_ui(|ui| errors::no_access_component(ui, entity, &name))
				});

				clicked_header.maybe_take(ComponentInfo::from_response(
					response.inner,
					false,
					type_id,
					component_id,
				));

				continue;
			}
		};

		if highlight_changes && value.is_changed() {
			set_highlight_style(ui);
		}

		let response = header.show(ui, |ui| {
			ui.reset_style();

			match value {
				ReflectBorrow::Mutable(mut value) => {
					let mut ctx = MutableContext {
						world_view,
						queue: ctx.queue,
					};

					let mut env = InspectorUi::new(type_registry, &mut ctx);
					let id = id.with(component_id);
					let options = &();
					let changed = env.ui_for_reflect_with_options(
						value.bypass_change_detection().as_partial_reflect_mut(),
						ui,
						id,
						options,
					);

					if changed {
						value.set_changed();
					}

					changed
				}
				ReflectBorrow::Immutable(value) => {
					let ctx = ImmutableContext::new(unsafe { world_view.world() }, ctx.queue);
					let env = InspectorUi::new(type_registry, &ctx);
					let id = id.with(component_id);
					let options = &();

					env.ui_for_reflect_readonly_with_options(value.as_partial_reflect(), ui, id, options);
					false
				}
			}
		});

		let changed = response.body_returned.unwrap_or_default();
		clicked_header.maybe_take(ComponentInfo::from_collapsing(
			response,
			changed,
			type_id,
			component_id,
		));

		ui.reset_style();
	}

	clicked_header
}

pub fn ui_for_entities_with_shared_components(
	world: &mut World,
	entities: &[Entity],
	ui: &mut egui::Ui,
) -> Option<egui::CollapsingResponse<ComponentInfo>> {
	world.queue(|world, queue| {
		let type_registry = world.resource::<AppTypeRegistry>().0.clone();
		let type_registry = type_registry.read();

		let &first = entities.first()?;

		let Ok(mut components) = components_of_entity(&world.into(), first) else {
			errors::entity_does_not_exist(ui, first);
			return None;
		};

		for &entity in entities.iter().skip(1) {
			components.retain(|(_, id, _, _)| {
				world
					.get_entity(entity)
					.map_or(true, |entity| entity.contains_id(*id))
			})
		}

		let mut clicked_header = None;

		let id = egui::Id::NULL;
		for (name, component_id, component_type_id, size) in components {
			let id = id.with(component_id);

			let header = egui::CollapsingHeader::new(&name).id_salt(id);

			let Some(type_id) = component_type_id else {
				header.show(ui, |ui| errors::no_type_id(ui, &name));
				continue;
			};

			if size == 0 {
				ui.indent(id, |ui| {
					let response = ui.label(&name);
					util::egui::show_docs(&type_registry, type_id, response);
				});
				continue;
			}

			let (resources_view, components_view) = RestrictedWorldView::resources_components(world);
			let mut cx = MutableContext::new(resources_view, queue);

			let mut values = Vec::with_capacity(entities.len());
			for (i, &entity) in entities.iter().enumerate() {
				// skip duplicate entities
				if entities[0..i].contains(&entity) {
					continue;
				};

				// SAFETY: entities are distinct, env has a context with just resources
				match unsafe {
					components_view.get_entity_component_reflect_unchecked(entity, type_id, &type_registry)
				} {
					Ok(value) => {
						values.push(value);
					}
					Err(error) => {
						error.ui(ui, &name);
						continue;
					}
				}
			}

			let response = header.show(ui, |ui| {
				ui.reset_style();

				let mut env = InspectorUi::new(&type_registry, &mut cx);
				let id = id.with(component_id);
				let options = &();

				let mut values_reflect: Vec<_> = values
					.iter_mut()
					.map(|value| value.bypass_change_detection().as_partial_reflect_mut())
					.collect();

				let changed = env.ui_for_reflect_many_with_options(
					type_id,
					&name,
					ui,
					id,
					options,
					values_reflect.as_mut_slice(),
					&|a| a,
				);

				if changed {
					for value in values.iter_mut() {
						value.set_changed();
					}
				}

				changed
			});

			let changed = response.body_returned.unwrap_or_default();
			clicked_header.maybe_take(ComponentInfo::from_collapsing(
				response,
				changed,
				type_id,
				component_id,
			));
		}

		clicked_header
	})
}

fn components_of_entity<W: WorldView>(
	world_view: &RestrictedWorldView<W>,
	entity: Entity,
) -> Result<Vec<(String, ComponentId, Option<TypeId>, usize)>> {
	let entity_ref = world_view.world().get_entity(entity)?;

	let archetype = entity_ref.archetype();
	let mut components: Vec<_> = archetype
		.components()
		.iter()
		.map(|component_id| {
			let info = world_view
				.world()
				.components()
				.get_info(*component_id)
				.unwrap();
			let name = util::pretty_type_name_str(&info.name().to_string());

			(name, *component_id, info.type_id(), info.layout().size())
		})
		.collect();
	components.sort_by(|(name_a, ..), (name_b, ..)| name_a.cmp(name_b));
	Ok(components)
}
