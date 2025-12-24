use crate::{
	inspector::{
		errors,
		ui::{Context, InspectorUi},
	},
	util::{
		self,
		egui::set_highlight_style,
		world::{ReflectBorrow, RestrictedWorldView},
	},
};
use bevy::{
	ecs::{component::ComponentId, world::CommandQueue},
	prelude::*,
	reflect::TypeRegistry,
};
use std::any::TypeId;

pub type EntityComponentContextMenu =
	fn(&mut egui::Ui, Entity, &mut World, &TypeRegistry, ComponentId, TypeId);

pub type EntitiesComponentContextMenu =
	fn(&mut egui::Ui, &[Entity], &mut World, &TypeRegistry, ComponentId, TypeId);

pub fn ui_for_entity_components(
	ctx: &mut Context<'_>,
	entity: Entity,
	ui: &mut egui::Ui,
	id: egui::Id,
	type_registry: &TypeRegistry,
	context_menu: EntityComponentContextMenu,
	highlight_changes: bool,
) {
	let Ok(components) = components_of_entity(&ctx.world, entity) else {
		errors::entity_does_not_exist(ui, entity);
		return;
	};

	for (name, component_id, component_type_id, size) in components {
		let id = id.with(component_id);

		let header = egui::CollapsingHeader::new(&name).id_salt(id);

		let Some(component_type_id) = component_type_id else {
			header.show(ui, |ui| errors::no_type_id(ui, &name));
			continue;
		};

		let type_docs = type_registry
			.get_type_info(component_type_id)
			.and_then(|info| info.docs());

		if size == 0 {
			ui.indent(id, |ui| {
				let response = ui.label(&name);

				response.context_menu(|ui| {
					(context_menu)(
						ui,
						entity,
						// SAFETY: Components is cloned, nothing depends on the world elsewhere
						unsafe { ctx.world.world().world_mut() },
						type_registry,
						component_id,
						component_type_id,
					);
				});

				util::egui::show_docs(response, type_docs);
			});
			continue;
		}

		// create a context with access to the world except for the currently viewed component
		let (mut component_view, world) = ctx.world.split_off_component((entity, component_type_id));

		let mut cx = Context {
			world,
			queue: ctx.queue,
		};

		let value =
			match component_view.get_entity_component_reflect(entity, component_type_id, type_registry) {
				Ok(value) => value,
				Err(_) => {
					ui.indent(id, |ui| {
						let response = ui
							.label(egui::RichText::new(&name).underline())
							.on_hover_ui(|ui| errors::no_access_component(ui, entity, &name));

						response.context_menu(|ui| {
							(context_menu)(
								ui,
								entity,
								// SAFETY: Will continue after this finishes
								unsafe { component_view.world().world_mut() },
								type_registry,
								component_id,
								component_type_id,
							);
						});
					});
					continue;
				}
			};

		if highlight_changes && value.is_changed() {
			set_highlight_style(ui);
		}

		let response = header.show(ui, |ui| {
			ui.reset_style();

			let mut env = InspectorUi::new(type_registry, Some(&mut cx));
			let id = id.with(component_id);
			let options = &();

			match value {
				ReflectBorrow::Mutable(mut value) => {
					let changed = env.ui_for_reflect_with_options(
						value.bypass_change_detection().as_partial_reflect_mut(),
						ui,
						id,
						options,
					);

					if changed {
						value.set_changed();
					}
				}
				ReflectBorrow::Immutable(value) => {
					env.ui_for_reflect_readonly_with_options(value.as_partial_reflect(), ui, id, options)
				}
			};
		});

		let response = response.header_response;

		response.context_menu(|ui| {
			(context_menu)(
				ui,
				entity,
				// SAFETY: Nothing after this point requires the world
				unsafe { component_view.world().world_mut() },
				type_registry,
				component_id,
				component_type_id,
			);
		});

		util::egui::show_docs(response, type_docs);

		ui.reset_style();
	}
}

pub fn ui_for_entities_shared_components(
	world: &mut World,
	entities: &[Entity],
	ui: &mut egui::Ui,
	context_menu: EntitiesComponentContextMenu,
) {
	let type_registry = world.resource::<AppTypeRegistry>().0.clone();
	let type_registry = type_registry.read();

	let Some(&first) = entities.first() else {
		return;
	};

	let Ok(mut components) = components_of_entity(&world.into(), first) else {
		return errors::entity_does_not_exist(ui, first);
	};

	for &entity in entities.iter().skip(1) {
		components.retain(|(_, id, _, _)| {
			world
				.get_entity(entity)
				.map_or(true, |entity| entity.contains_id(*id))
		})
	}

	let mut queue = CommandQueue::default();

	let id = egui::Id::NULL;
	for (name, component_id, component_type_id, size) in components {
		let id = id.with(component_id);

		let header = egui::CollapsingHeader::new(&name).id_salt(id);

		let Some(component_type_id) = component_type_id else {
			header.show(ui, |ui| errors::no_type_id(ui, &name));
			continue;
		};

		let type_docs = type_registry
			.get_type_info(component_type_id)
			.and_then(|info| info.docs());

		if size == 0 {
			ui.indent(id, |ui| {
				let _response = ui.label(&name);
				util::egui::show_docs(_response, type_docs);
			});
			continue;
		}

		let (resources_view, components_view) = RestrictedWorldView::resources_components(world);
		let mut cx = Context {
			world: resources_view,
			queue: &mut queue,
		};

		let mut values = Vec::with_capacity(entities.len());
		for (i, &entity) in entities.iter().enumerate() {
			// skip duplicate entities
			if entities[0..i].contains(&entity) {
				continue;
			};

			// SAFETY: entities are distinct, env has a context with just resources
			match unsafe {
				components_view.get_entity_component_reflect_unchecked(
					entity,
					component_type_id,
					&type_registry,
				)
			} {
				Ok(value) => {
					values.push(value);
				}
				Err(error) => {
					errors::show_error(error, ui, &name);
					return;
				}
			}
		}

		let response = header.show(ui, |ui| {
			ui.reset_style();

			let mut env = InspectorUi::new(&type_registry, Some(&mut cx));
			let id = id.with(component_id);
			let options = &();

			let mut values_reflect: Vec<_> = values
				.iter_mut()
				.map(|value| value.bypass_change_detection().as_partial_reflect_mut())
				.collect();

			let changed = env.ui_for_reflect_many_with_options(
				component_type_id,
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
		});

		response.header_response.context_menu(|ui| {
			(context_menu)(
				ui,
				entities,
				world,
				&type_registry,
				component_id,
				component_type_id,
			);
		});
	}

	queue.apply(world);
}

fn components_of_entity(
	world: &RestrictedWorldView<'_>,
	entity: Entity,
) -> Result<Vec<(String, ComponentId, Option<TypeId>, usize)>> {
	let entity_ref = world.world().get_entity(entity)?;

	let archetype = entity_ref.archetype();
	let mut components: Vec<_> = archetype
		.components()
		.iter()
		.map(|component_id| {
			let info = world.world().components().get_info(*component_id).unwrap();
			let name = util::pretty_type_name_str(&info.name().to_string());

			(name, *component_id, info.type_id(), info.layout().size())
		})
		.collect();
	components.sort_by(|(name_a, ..), (name_b, ..)| name_a.cmp(name_b));
	Ok(components)
}
