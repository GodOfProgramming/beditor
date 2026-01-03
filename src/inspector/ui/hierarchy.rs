use super::{EntityFilter, Filter};
use crate::util::egui::{CollapsingResponseExtensions, ResponseConditions};
use crate::util::entity;
use bevy::ecs::entity::EntityHashSet;
use bevy::{ecs::query::QueryFilter, prelude::*};
use derive_new::new;
use egui::{CollapsingHeader, CollapsingResponse, RichText};
use smallvec::SmallVec;
use std::collections::HashSet;
use std::sync::Arc;

pub struct UnusedPayload;

pub type DndHandlerFn<P> =
	fn(ui: &mut egui::Ui, entity: Entity, world: &mut World, payload: Arc<P>);

pub struct Hierarchy<'a, P = UnusedPayload>
where
	P: 'static + Send + Sync,
{
	pub world: &'a mut World,
	pub selected: &'a mut SelectedEntities,
	pub dnd: DndHandlerFn<P>,
}

impl<P> Hierarchy<'_, P>
where
	P: 'static + Send + Sync,
{
	pub fn show<QF>(&mut self, ui: &mut egui::Ui) -> Option<CollapsingResponse<Entity>>
	where
		QF: QueryFilter,
	{
		let filter: Filter = Filter::from_ui(ui, egui::Id::new("default_hierarchy_filter"));

		let mut root_query = self
			.world
			.query_filtered::<Entity, (Without<ChildOf>, QF)>();

		let always_open: HashSet<Entity> = self
			.selected
			.iter()
			.flat_map(|selected| {
				std::iter::successors(Some(selected), |&entity| {
					self.world.get::<ChildOf>(entity).map(|c| c.0)
				})
				.skip(1)
			})
			.collect();

		let mut entities: Vec<_> = root_query.iter(self.world).collect();
		filter.filter_entities(self.world, &mut entities);
		entities.sort();

		let mut selected = None;

		for &entity in &entities {
			selected.maybe_take(self.entity_ui(ui, entity, &always_open, &entities, &filter));
		}

		selected
	}

	fn entity_ui<F>(
		&mut self,
		ui: &mut egui::Ui,
		entity: Entity,
		always_open: &HashSet<Entity>,
		at_same_level: &[Entity],
		filter: &F,
	) -> Option<CollapsingResponse<Entity>>
	where
		F: EntityFilter,
	{
		let mut new_selection = None;
		let selected = self.selected.contains(entity);

		let entity_name = entity::guess_entity_name(self.world, entity);
		let mut name = RichText::new(entity_name);
		if selected {
			name = name.strong();
		}

		let has_children = self
			.world
			.get::<Children>(entity)
			.is_some_and(|children| !children.is_empty());

		let open = if !has_children {
			Some(false)
		} else if always_open.contains(&entity) {
			Some(true)
		} else {
			None
		};

		let frame = egui::Frame::default();
		let mut frame = frame.begin(ui);

		let response = CollapsingHeader::new(name)
			.id_salt(entity)
			.icon(move |ui, openness, response| {
				if !has_children {
					return;
				}
				paint_default_icon(ui, openness, response);
			})
			.open(open)
			.show(&mut frame.content_ui, |ui| {
				let children = self.world.get::<Children>(entity);

				if let Some(children) = children {
					let mut children = children.to_vec();
					filter.filter_entities(self.world, &mut children);
					for &child in &children {
						new_selection.maybe_take(self.entity_ui(ui, child, always_open, &children, filter));
					}
				} else {
					ui.label("No children");
				}
			});

		let dnd_response = frame.allocate_space(ui);

		if response.header_response.clicked() {
			let selection_mode = ui
				.input(|input| SelectionMode::from_ctrl_shift(input.modifiers.ctrl, input.modifiers.shift));

			let extend_with = |from, to| {
				let mut from_position = None;
				let mut to_position = None;

				for (i, &entity) in at_same_level.iter().enumerate() {
					if entity == from {
						from_position = Some(i);
					}

					if entity == to {
						to_position = Some(i)
					}
				}

				from_position
					.zip(to_position)
					.map(|(from, to)| {
						let (min, max) = if from < to { (from, to) } else { (to, from) };
						at_same_level[min..=max].iter().copied()
					})
					.into_iter()
					.flatten()
			};

			let event = self.selected.select(selection_mode, entity, extend_with);
			self.world.trigger(event);

			new_selection.maybe_take(Some(egui::CollapsingResponse {
				header_response: response.header_response,
				body_response: response.body_response,
				body_returned: Some(entity),
				openness: response.openness,
			}));
		} else if ResponseConditions::from(&response.header_response).any()
			|| response
				.body_response
				.as_ref()
				.map(|r| ResponseConditions::from(r).any())
				.unwrap_or(false)
		{
			new_selection.maybe_take(Some(egui::CollapsingResponse {
				header_response: response.header_response,
				body_response: response.body_response,
				body_returned: Some(entity),
				openness: response.openness,
			}));
		}

		if let Some(payload) = dnd_response.dnd_release_payload::<P>() {
			(self.dnd)(ui, entity, self.world, payload)
		}

		new_selection
	}
}

fn paint_default_icon(ui: &mut egui::Ui, openness: f32, response: &egui::Response) {
	let visuals = ui.style().interact(response);
	let stroke = visuals.fg_stroke;

	let rect = response.rect;

	// Draw a pointy triangle arrow:
	let rect = egui::Rect::from_center_size(
		rect.center(),
		egui::vec2(rect.width(), rect.height()) * 0.75,
	);
	let rect = rect.expand(visuals.expansion);
	let mut points = vec![rect.left_top(), rect.right_top(), rect.center_bottom()];
	use std::f32::consts::TAU;
	let rotation = egui::emath::Rot2::from_angle(egui::remap(openness, 0.0..=1.0, -TAU / 4.0..=0.0));
	for p in &mut points {
		*p = rect.center() + rotation * (*p - rect.center());
	}

	ui.painter().add(egui::Shape::closed_line(points, stroke));
}

#[derive(Default, Debug)]
pub struct SelectedEntities {
	entities: Vec<Entity>,
	last_action: Option<(SelectionMode, Entity)>,
}

#[derive(Debug, Clone, Copy)]
pub enum SelectionMode {
	/// No modifiers
	Replace,
	/// `Ctrl`
	Add,
	/// `Shift`
	Extend,
}

impl SelectionMode {
	pub fn from_ctrl_shift(ctrl: bool, shift: bool) -> SelectionMode {
		match (ctrl, shift) {
			(true, _) => SelectionMode::Add,
			(false, true) => SelectionMode::Extend,
			(false, false) => SelectionMode::Replace,
		}
	}
}

impl SelectedEntities {
	pub fn select_replace(&mut self, entity: Entity) -> SelectedEntitiesChangedEvent {
		self.scope(|this| {
			this.insert_replace(entity);
			this.last_action = Some((SelectionMode::Replace, entity));
		})
	}

	pub fn select_maybe_add(&mut self, entity: Entity, add: bool) -> SelectedEntitiesChangedEvent {
		let mode = match add {
			true => SelectionMode::Add,
			false => SelectionMode::Replace,
		};
		SelectedEntities::select(self, mode, entity, |_, _| std::iter::empty())
	}

	pub fn select<I: IntoIterator<Item = Entity>>(
		&mut self,
		mode: SelectionMode,
		entity: Entity,
		extend_with: impl Fn(Entity, Entity) -> I,
	) -> SelectedEntitiesChangedEvent {
		self.scope(|this| {
			match (this.len(), mode) {
				(0, _) => {
					this.insert(entity);
				}
				(_, SelectionMode::Replace) => {
					this.insert_replace(entity);
				}
				(_, SelectionMode::Add) => {
					// toggle
					if let Some(idx) = this.entities.iter().position(|&e| e == entity) {
						this.entities.remove(idx);
					} else {
						this.entities.push(entity);
					}
				}
				(_, SelectionMode::Extend) => {
					match this.last_action {
						None => this.insert(entity),
						Some((last_mode, last_entity)) => {
							if let SelectionMode::Add | SelectionMode::Replace = last_mode {
								this.clear();
							}
							for entity in extend_with(entity, last_entity) {
								this.insert(entity);
							}

							// extending doesn't update last action
							return;
						}
					};
				}
			}
			this.last_action = Some((mode, entity));
		})
	}

	pub fn contains(&self, entity: Entity) -> bool {
		self.entities.contains(&entity)
	}
	fn insert(&mut self, entity: Entity) {
		if !self.contains(entity) {
			self.entities.push(entity);
		}
	}

	fn insert_replace(&mut self, entity: Entity) {
		self.entities.clear();
		self.entities.push(entity);
	}

	pub fn scope(&mut self, f: impl FnOnce(&mut Self)) -> SelectedEntitiesChangedEvent {
		let previous = EntityHashSet::from_iter(self.iter());
		f(self);
		let current = EntityHashSet::from_iter(self.iter());

		SelectedEntitiesChangedEvent::new(
			self.as_slice().into(),
			previous.difference(&current).cloned().collect(),
		)
	}

	pub fn last_action(&self) -> Option<(SelectionMode, Entity)> {
		self.last_action
	}

	pub fn scoped_clear(&mut self) -> SelectedEntitiesChangedEvent {
		self.scope(|this| {
			this.clear();
		})
	}

	fn clear(&mut self) {
		self.entities.clear();
	}

	pub fn len(&self) -> usize {
		self.entities.len()
	}

	pub fn is_empty(&self) -> bool {
		self.entities.len() == 0
	}

	pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
		self.entities.iter().copied()
	}

	pub fn as_slice(&self) -> &[Entity] {
		self.entities.as_slice()
	}
}

#[must_use]
#[derive(new, Event)]
pub struct SelectedEntitiesChangedEvent {
	current: SmallVec<[Entity; 8]>,
	removed: SmallVec<[Entity; 8]>,
}

impl SelectedEntitiesChangedEvent {
	pub fn on_event(event: On<Self>, mut commands: Commands) {
		for &entity in event.current.iter() {
			if let Ok(mut entity) = commands.get_entity(entity) {
				entity.insert(Selected);
			}
		}

		for &entity in event.removed.iter() {
			if let Ok(mut entity) = commands.get_entity(entity) {
				entity.remove::<Selected>();
			}
		}
	}
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Selected;
