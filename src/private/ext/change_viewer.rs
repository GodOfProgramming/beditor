use crate::{
	EditorExtension, EditorUiWorld,
	private::{EditorInternal, util::extensions::WorldMutExtensions as _},
	ui::OpenUi,
};
use bevy::{
	ecs::{component::ComponentId, entity::EntityHashMap, world::unsafe_world_cell::UnsafeWorldCell},
	platform::collections::HashMap,
	prelude::*,
};
use derive_new::new;
use nameof::name_of_type;
use std::{
	any::{Any, TypeId},
	borrow::Borrow,
	collections::BTreeMap,
};
use uuid::{Uuid, uuid};

#[derive(Default)]
pub struct ChangeViewerUiExtension;

impl EditorExtension for ChangeViewerUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<ChangeViewerUi>();
	}
}

#[derive(Component, Reflect, Default)]
pub struct ChangeViewerUi {
	kind: Option<ChangeViewerKind>,
	entity_hits: BTreeMap<Entity, HashMap<(String, u32), usize>>,
	entity_names: EntityHashMap<String>,
	searchable_name: String,
	search_error: Option<String>,
	cached_type_id: Option<TypeId>,
	failed_to_find_component_name: bool,
	cached_name: Option<String>,
}

impl ChangeViewerUi {
	fn new_with_id(kind: ChangeViewerKind, type_id: Option<TypeId>) -> Self {
		Self {
			kind: Some(kind),
			cached_type_id: type_id,
			entity_hits: default(),
			entity_names: default(),
			searchable_name: default(),
			search_error: default(),
			cached_name: default(),
			failed_to_find_component_name: default(),
		}
	}
}

#[derive(Component, Reflect, Default)]
#[require(EditorInternal)]
pub struct ChangeViewerMarker;

impl EditorUiWorld for ChangeViewerUi {
	type MarkerComponent = ChangeViewerUi;

	const NAME: &str = name_of_type!(ChangeViewerUi);

	const ID: Uuid = uuid!("51c26e01-69cb-4a74-826d-995b72a2a281");

	const REOPEN_ON_STARTUP: bool = false;

	fn spawn(_entity: Entity, _world: &mut World) -> Result<Self> {
		Ok(default())
	}

	fn title(entity: Entity, world: &mut World) -> Result<egui::WidgetText> {
		let cell = world.as_unsafe_world_cell();

		// SAFETY: goal is to find the component name once
		unsafe {
			let Ok(entity_mut) = cell.get_entity(entity) else {
				return Ok(Self::NAME.into());
			};

			let Some(mut state) = entity_mut.get_mut::<ChangeViewerUi>() else {
				return Ok(Self::NAME.into());
			};

			if let Some(cached_name) = &state.cached_name {
				return Ok(cached_name.clone().into());
			}

			let Some(type_id) = state.cached_type_id else {
				if state.failed_to_find_component_name {
					return Ok(Self::NAME.into());
				}

				let Some(kind) = &state.kind else {
					return Ok(Self::NAME.into());
				};

				let component_id = match kind {
					ChangeViewerKind::Resource(component_id) => component_id,
					ChangeViewerKind::Component { component_id, .. } => component_id,
				};

				let Some(component) = cell
					.components()
					.iter_registered()
					.find(|c| c.id() == *component_id)
				else {
					state.failed_to_find_component_name = true;
					return Ok(Self::NAME.into());
				};

				let Some(tid) = component.type_id() else {
					state.failed_to_find_component_name = true;
					return Ok(Self::NAME.into());
				};

				state.cached_type_id = Some(tid);

				return Ok(Self::NAME.into());
			};

			let Some(app_type_registry) = cell.get_resource::<AppTypeRegistry>() else {
				return Ok(Self::NAME.into());
			};
			let type_registry = app_type_registry.read();

			let Some(tr) = type_registry.get(type_id) else {
				return Ok(Self::NAME.into());
			};

			let name = tr.type_info().type_path_table().short_path();

			let name = format!("{} - {}", Self::NAME, name);

			state.cached_name = Some(name.clone());

			Ok(name.into())
		}
	}

	fn ui(entity: Entity, ui: &mut egui::Ui, world: &mut World) -> Result {
		let cell = world.as_unsafe_world_cell();

		let Ok(entity_mut) = cell.get_entity(entity) else {
			ui.label(format!("Entity {entity} does not exist"));
			return Ok(());
		};
		let mut state = unsafe {
			let Some(state) = entity_mut.get_mut::<ChangeViewerUi>() else {
				return Err(BevyError::error("Failed to get ChangeViewerState"));
			};
			state
		};

		let ChangeViewerUi {
			kind,
			entity_hits,
			entity_names,
			searchable_name,
			search_error,
			..
		} = &mut *state;

		let Some(kind) = kind.as_ref() else {
			ui.label("Search For a resource by name");
			ui.horizontal(|ui| {
				ui.text_edit_singleline(searchable_name);
				if ui.button("Search").clicked() {
					'search: {
						unsafe {
							let Some(app_type_registry) = cell.get_resource::<AppTypeRegistry>() else {
								break 'search;
							};
							let tr = app_type_registry.read();
							let Some(t) = tr
								.get_with_short_type_path(searchable_name)
								.or_else(|| tr.get_with_type_path(searchable_name))
							else {
								*search_error = Some(format!(
									"Failed to find type registration of {searchable_name}"
								));
								break 'search;
							};

							let Some(component_id) = get_component_id_from_type_id(cell, t.type_id()) else {
								*search_error = Some(format!("Failed to find component id of {searchable_name}"));
								break 'search;
							};

							*kind = Some(ChangeViewerKind::Resource(component_id));
						}
					}
				}
			});

			if let Some(error) = search_error {
				ui.colored_label(egui::Color32::RED, error);
			}
			return Ok(());
		};

		match kind {
			ChangeViewerKind::Resource(component_id) => {
				let entity = unsafe {
					let Some(e) = cell.resource_entities().get(*component_id) else {
						return Ok(());
					};
					e
				};
				let Some(entity_cell) = cell.get_entity(entity).ok() else {
					return Ok(());
				};
				let r = unsafe {
					let Ok(r) = entity_cell.get_mut_by_id(*component_id) else {
						return Ok(());
					};
					r
				};

				let Some(loc) = r.changed_by().into_option() else {
					return Ok(());
				};

				let file = loc.file();
				let line = loc.line();

				let hits = entity_hits.entry(entity).or_default();
				let entry = hits.entry((file.to_string(), line)).or_default();
				*entry += 1;

				for ((file, line), count) in hits.iter() {
					ui.label(format!("{file} ({line}): {count}"));
				}
			}
			ChangeViewerKind::Component {
				entities,
				component_id,
			} => {
				for &entity in entities {
					let Ok(entity_cell) = cell.get_entity(entity) else {
						continue;
					};

					let c = unsafe {
						let Ok(c) = entity_cell.get_mut_by_id(*component_id) else {
							continue;
						};
						c
					};

					let Some(loc) = c.changed_by().into_option() else {
						continue;
					};

					let file = loc.file();
					let line = loc.line();

					let hits = entity_hits.entry(entity).or_default();
					let entry = hits.entry((file.to_string(), line)).or_default();
					*entry += 1;
				}

				for (&entity, hits) in entity_hits.iter() {
					let name = entity_names
						.entry(entity)
						.or_insert_with(|| format!("{entity}"));
					ui.collapsing(name.as_str(), |ui| {
						for ((file, line), count) in hits.iter() {
							ui.label(format!("{file} ({line}): {count}"));
						}
					});
				}
			}
		}

		Ok(())
	}
}

#[derive(Reflect)]
enum ChangeViewerKind {
	Resource(ComponentId),
	Component {
		entities: Vec<Entity>,
		component_id: ComponentId,
	},
}

pub enum OpenChangeViewerKind {
	Resource(OpenResourceKind),
	Component {
		entities: Vec<Entity>,
		component_id: ComponentId,
	},
}

pub enum OpenResourceKind {
	TypeId(TypeId),
	ComponentId(ComponentId),
}

#[derive(new)]
pub struct OpenChangeViewer {
	kind: OpenChangeViewerKind,
}

impl Command for OpenChangeViewer {
	type Out = ();
	fn apply(self, world: &mut World) {
		let (kind, maybe_type_id) = match self.kind {
			OpenChangeViewerKind::Resource(OpenResourceKind::TypeId(type_id)) => {
				let Some(component_id) =
					get_component_id_from_type_id(world.as_unsafe_world_cell_readonly(), type_id)
				else {
					return;
				};
				(ChangeViewerKind::Resource(component_id), Some(type_id))
			}
			OpenChangeViewerKind::Resource(OpenResourceKind::ComponentId(component_id)) => {
				(ChangeViewerKind::Resource(component_id), None)
			}
			OpenChangeViewerKind::Component {
				entities,
				component_id,
			} => (
				ChangeViewerKind::Component {
					entities,
					component_id,
				},
				None,
			),
		};

		world.queue(|_, queue| {
			queue.push(OpenUi::open_with_value(
				crate::ui::OpenMode::Window,
				ChangeViewerUi::new_with_id(kind, maybe_type_id),
			))
		});
	}
}

fn get_component_id_from_type_id<'w>(
	cell: impl Borrow<UnsafeWorldCell<'w>>,
	type_id: TypeId,
) -> Option<ComponentId> {
	let component_id = cell
		.borrow()
		.components()
		.iter_registered()
		.filter_map(|c| c.type_id().map(|tid| (c.id(), tid)))
		.find_map(|(cid, tid)| if tid == type_id { Some(cid) } else { None })?;

	Some(component_id)
}
