pub mod components;
pub mod hierarchy;

use crate::{
	inspector::{
		data::{InspectorPrimitive, many_unimplemented},
		errors::{self, reflect::TypeDataError},
		options::{InspectorOptions, ReflectInspectorOptions, Target},
	},
	util::{
		self,
		egui::{
			add_button, down_button, maybe_grid, maybe_grid_label_if, maybe_grid_readonly,
			maybe_grid_readonly_label_if, remove_button, show_docs, up_button,
		},
		entity, or,
		world::RestrictedWorldView,
	},
};
use bevy::{
	ecs::{query::QueryFilter, world::CommandQueue},
	prelude::*,
	reflect::{
		Array, DynamicEnum, DynamicStruct, DynamicTuple, DynamicTyped, DynamicVariant, Enum, EnumInfo,
		FromType, List, ListInfo, Map, ReflectMut, ReflectRef, Set, SetInfo, StructInfo, Tuple,
		TupleInfo, TupleStructInfo, TypeInfo, TypeRegistry, VariantInfo, VariantType,
	},
};
use derive_new::new;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use std::{
	any::{Any, TypeId},
	borrow::{Borrow, Cow},
	marker::PhantomData,
};

pub trait ProjectorReflect: Fn(&mut dyn PartialReflect) -> &mut dyn PartialReflect {}

impl<T> ProjectorReflect for T where T: Fn(&mut dyn PartialReflect) -> &mut dyn PartialReflect {}

#[derive(new)]
pub struct Context<'c> {
	pub world: RestrictedWorldView<'c>,
	pub queue: &'c mut CommandQueue,
}

#[derive(new)]
pub struct InspectorUi<'i, 'c> {
	pub type_registry: &'i TypeRegistry,
	pub context: Option<&'i mut Context<'c>>,
}

impl<'i, 'c> InspectorUi<'i, 'c> {
	pub fn reborrow<'s>(&'s mut self) -> InspectorUi<'s, 'c> {
		InspectorUi {
			type_registry: self.type_registry,
			context: self.context.as_deref_mut(),
		}
	}

	fn get_reflect_default(&self, type_id: TypeId) -> Option<&ReflectDefault> {
		self.type_registry.get_type_data::<ReflectDefault>(type_id)
	}

	fn get_default_value_for(&mut self, type_id: TypeId) -> Option<Box<dyn Reflect>> {
		if let Some(reflect_default) = self.type_registry.get_type_data::<ReflectDefault>(type_id) {
			return Some(reflect_default.default());
		}

		None
	}

	fn construct_default_variant(
		&mut self,
		variant: &VariantInfo,
		ui: &mut egui::Ui,
	) -> Result<DynamicEnum, ()> {
		let dynamic_variant = match variant {
			VariantInfo::Struct(struct_info) => {
				let mut dynamic_struct = DynamicStruct::default();
				for field in struct_info.iter() {
					let field_default_value = match self.get_default_value_for(field.type_id()) {
						Some(value) => value,
						None => {
							errors::reflect::no_default_value(ui, field.type_path());
							return Err(());
						}
					};
					dynamic_struct.insert_boxed(field.name(), field_default_value.to_dynamic());
				}
				DynamicVariant::Struct(dynamic_struct)
			}
			VariantInfo::Tuple(tuple_info) => {
				let mut dynamic_tuple = DynamicTuple::default();
				for field in tuple_info.iter() {
					let field_default_value = match self.get_default_value_for(field.type_id()) {
						Some(value) => value,
						None => {
							errors::reflect::no_default_value(ui, field.type_path());
							return Err(());
						}
					};
					dynamic_tuple.insert_boxed(field_default_value.to_dynamic());
				}
				DynamicVariant::Tuple(dynamic_tuple)
			}
			VariantInfo::Unit(_) => DynamicVariant::Unit,
		};
		let dynamic_enum = DynamicEnum::new(variant.name(), dynamic_variant);
		Ok(dynamic_enum)
	}
}

impl InspectorUi<'_, '_> {
	/// Draws the inspector UI for the given value.
	pub fn ui_for_reflect(&mut self, value: &mut dyn PartialReflect, ui: &mut egui::Ui) -> bool {
		self.ui_for_reflect_with_options(value, ui, egui::Id::NULL, &())
	}

	/// Draws the inspector UI for the given value in a read-only way.
	pub fn ui_for_reflect_readonly(&mut self, value: &dyn PartialReflect, ui: &mut egui::Ui) {
		self.ui_for_reflect_readonly_with_options(value, ui, egui::Id::NULL, &());
	}

	/// Draws the inspector UI for the given value with some options.
	///
	/// The options can be [`struct@InspectorOptions`] for structs or enums with nested options for their fields,
	/// or other structs like [`NumberOptions`](crate::inspector_options::std_options::NumberOptions) which are interpreted
	/// by leaf types like `f32` or `Vec3`,
	pub fn ui_for_reflect_with_options(
		&mut self,
		value: &mut dyn PartialReflect,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) -> bool {
		let mut options = options;
		if options.is::<()>()
			&& let Some(data) = value.try_as_reflect().and_then(|val| {
				self
					.type_registry
					.get_type_data::<ReflectInspectorOptions>(val.type_id())
			}) {
			options = &data.0;
		}
		let reason = match value.try_as_reflect_mut() {
			Some(value) => match get_type_data(self.type_registry, value) {
				Ok(ui_impl) => {
					return ui_impl.execute(value.as_any_mut(), ui, options, id, self.reborrow());
				}
				Err(e) => e,
			},
			None => TypeDataError::NotFullyReflected,
		};

		if let Some(changed) = short_circuit::short_circuit(self, value, ui, id, options) {
			return changed;
		}

		match value.reflect_mut() {
			ReflectMut::Struct(value) => self.ui_for_struct(value, ui, id, options),
			ReflectMut::TupleStruct(value) => self.ui_for_tuple_struct(value, ui, id, options),
			ReflectMut::Tuple(value) => self.ui_for_tuple(value, ui, id, options),
			ReflectMut::List(value) => self.ui_for_list(value, ui, id, options),
			ReflectMut::Array(value) => self.ui_for_array(value, ui, id, options),
			ReflectMut::Map(value) => self.ui_for_reflect_map(value, ui, id, options),
			ReflectMut::Enum(value) => self.ui_for_enum(value, ui, id, options),
			ReflectMut::Opaque(value) => {
				errors::reflect::reflect_value_no_impl(ui, reason, value.reflect_short_type_path());
				false
			}
			ReflectMut::Set(value) => self.ui_for_set(value, ui, id, options),
			#[allow(unreachable_patterns)]
			_ => {
				ui.label("unsupported");
				false
			}
		}
	}

	/// Draws the inspector UI for the given value with some options in a read-only way.
	///
	/// The options can be [`struct@InspectorOptions`] for structs or enums with nested options for their fields,
	/// or other structs like [`NumberOptions`](crate::inspector_options::std_options::NumberOptions) which are interpreted
	/// by leaf types like `f32` or `Vec3`,
	pub fn ui_for_reflect_readonly_with_options(
		&mut self,
		value: &dyn PartialReflect,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) {
		let mut options = options;
		if options.is::<()>()
			&& let Some(value_reflect) = value.try_as_reflect()
			&& let Some(data) = self
				.type_registry
				.get_type_data::<ReflectInspectorOptions>(value_reflect.type_id())
		{
			options = &data.0;
		}

		let reason = match value.try_as_reflect() {
			Some(value) => match get_type_data(self.type_registry, value) {
				Ok(ui_impl) => {
					return ui_impl.execute_readonly(value.as_any(), ui, options, id, self.reborrow());
				}
				Err(e) => e,
			},
			None => TypeDataError::NotFullyReflected,
		};

		if let Some(()) = short_circuit::short_circuit_readonly(self, value, ui, id, options) {
			return;
		}

		match value.reflect_ref() {
			ReflectRef::Struct(value) => self.ui_for_struct_readonly(value, ui, id, options),
			ReflectRef::TupleStruct(value) => self.ui_for_tuple_struct_readonly(value, ui, id, options),
			ReflectRef::Tuple(value) => self.ui_for_tuple_readonly(value, ui, id, options),
			ReflectRef::List(value) => self.ui_for_list_readonly(value, ui, id, options),
			ReflectRef::Array(value) => self.ui_for_array_readonly(value, ui, id, options),
			ReflectRef::Map(value) => self.ui_for_reflect_map_readonly(value, ui, id, options),
			ReflectRef::Enum(value) => self.ui_for_enum_readonly(value, ui, id, options),
			ReflectRef::Opaque(value) => {
				errors::reflect::reflect_value_no_impl(ui, reason, value.reflect_short_type_path())
			}
			ReflectRef::Set(value) => self.ui_for_set_readonly(value, ui, id, options),
			#[allow(unreachable_patterns)]
			_ => {
				ui.label("unsupported");
			}
		}
	}

	pub fn ui_for_reflect_many(
		&mut self,
		type_id: TypeId,
		name: &str,
		ui: &mut egui::Ui,
		id: egui::Id,
		values: &mut [&mut dyn PartialReflect],
		projector: &dyn ProjectorReflect,
	) -> bool {
		self.ui_for_reflect_many_with_options(type_id, name, ui, id, &(), values, projector)
	}

	pub fn ui_for_reflect_many_with_options(
		&mut self,
		type_id: TypeId,
		name: &str,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
		values: &mut [&mut dyn PartialReflect],
		projector: &dyn ProjectorReflect,
	) -> bool {
		let Some(registration) = self.type_registry.get(type_id) else {
			errors::reflect::not_in_type_registry(ui, name);
			return false;
		};
		let info = registration.type_info();

		let mut options = options;
		if options.is::<()>()
			&& let Some(data) = self
				.type_registry
				.get_type_data::<ReflectInspectorOptions>(type_id)
		{
			options = &data.0;
		}

		let reason = match registration.data::<InspectorEguiImpl>() {
			Some(ui_impl) => {
				return ui_impl.execute_many(ui, options, id, self.reborrow(), values, projector);
			}
			None => TypeDataError::NoTypeData,
		};

		if let Some(s) = self
			.type_registry
			.get_type_data::<InspectorEguiImpl>(type_id)
		{
			return s.execute_many(ui, options, id, self.reborrow(), values, projector);
		}

		if let Some(changed) =
			short_circuit::short_circuit_many(self, type_id, ui, id, options, values, projector)
		{
			return changed;
		}

		match info {
			TypeInfo::Struct(info) => self.ui_for_struct_many(info, ui, id, options, values, projector),
			TypeInfo::TupleStruct(info) => {
				self.ui_for_tuple_struct_many(info, ui, id, options, values, projector)
			}
			TypeInfo::Tuple(info) => self.ui_for_tuple_many(info, ui, id, options, values, projector),
			TypeInfo::List(info) => self.ui_for_list_many(info, ui, id, options, values, projector),
			TypeInfo::Array(info) => {
				errors::reflect::no_multiedit(ui, &util::pretty_type_name_str(info.type_path()));
				false
			}
			TypeInfo::Map(info) => {
				errors::reflect::no_multiedit(ui, &util::pretty_type_name_str(info.type_path()));
				false
			}
			TypeInfo::Enum(info) => self.ui_for_enum_many(info, ui, id, options, values, projector),
			TypeInfo::Opaque(info) => {
				errors::reflect::reflect_value_no_impl(ui, reason, info.type_path());
				false
			}
			TypeInfo::Set(info) => self.ui_for_set_many(info, ui, id, options, values, projector),
		}
	}
}

impl InspectorUi<'_, '_> {
	fn ui_for_struct(
		&mut self,
		value: &mut dyn Struct,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) -> bool {
		let Some(TypeInfo::Struct(type_info)) = value.get_represented_type_info() else {
			return false;
		};

		let mut changed = false;
		egui::Grid::new(id).show(ui, |ui| {
			for i in 0..value.field_len() {
				let field_info = type_info.field_at(i).unwrap();

				let response = ui.label(field_info.name());

				show_docs(response, field_info.docs());

				let field = value.field_at_mut(i).unwrap();
				changed |= self.ui_for_reflect_with_options(
					field,
					ui,
					id.with(i),
					inspector_options_struct_field(options, i),
				);
				ui.end_row();
			}
		});
		changed
	}

	fn ui_for_struct_readonly(
		&mut self,
		value: &dyn Struct,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) {
		let Some(TypeInfo::Struct(type_info)) = value.get_represented_type_info() else {
			return;
		};

		egui::Grid::new(id).show(ui, |ui| {
			for i in 0..value.field_len() {
				let field_info = type_info.field_at(i).unwrap();

				let _response = ui.label(field_info.name());

				show_docs(_response, field_info.docs());

				let field = value.field_at(i).unwrap();
				self.ui_for_reflect_readonly_with_options(
					field,
					ui,
					id.with(i),
					inspector_options_struct_field(options, i),
				);
				ui.end_row();
			}
		});
	}

	fn ui_for_struct_many(
		&mut self,
		info: &StructInfo,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
		values: &mut [&mut dyn PartialReflect],
		projector: impl ProjectorReflect,
	) -> bool {
		let mut changed = false;
		egui::Grid::new(id).show(ui, |ui| {
			for (i, field) in info.iter().enumerate() {
				let _response = ui.label(field.name());

				show_docs(_response, field.docs());

				changed |= self.ui_for_reflect_many_with_options(
					field.type_id(),
					field.type_path(),
					ui,
					id.with(i),
					inspector_options_struct_field(options, i),
					values,
					&|a| match projector(a).reflect_mut() {
						ReflectMut::Struct(strukt) => strukt.field_at_mut(i).unwrap(),
						_ => unreachable!(),
					},
				);
				ui.end_row();
			}
		});
		changed
	}

	fn ui_for_tuple_struct(
		&mut self,
		value: &mut dyn TupleStruct,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) -> bool {
		maybe_grid(value.field_len(), ui, id, |ui, label| {
			(0..value.field_len())
				.map(|i| {
					if label {
						ui.label(i.to_string());
					}
					let field = value.field_mut(i).unwrap();
					let changed = self.ui_for_reflect_with_options(
						field,
						ui,
						id.with(i),
						inspector_options_struct_field(options, i),
					);
					ui.end_row();
					changed
				})
				.fold(false, or)
		})
	}

	fn ui_for_tuple_struct_readonly(
		&mut self,
		value: &dyn TupleStruct,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) {
		maybe_grid_readonly(value.field_len(), ui, id, |ui, label| {
			for i in 0..value.field_len() {
				if label {
					ui.label(i.to_string());
				}
				let field = value.field(i).unwrap();
				self.ui_for_reflect_readonly_with_options(
					field,
					ui,
					id.with(i),
					inspector_options_struct_field(options, i),
				);
				ui.end_row();
			}
		})
	}

	fn ui_for_tuple_struct_many(
		&mut self,
		info: &TupleStructInfo,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
		values: &mut [&mut dyn PartialReflect],
		projector: impl ProjectorReflect,
	) -> bool {
		maybe_grid(info.field_len(), ui, id, |ui, label| {
			info
				.iter()
				.enumerate()
				.map(|(i, field)| {
					if label {
						ui.label(i.to_string());
					}
					let changed = self.ui_for_reflect_many_with_options(
						field.type_id(),
						field.type_path(),
						ui,
						id.with(i),
						inspector_options_struct_field(options, i),
						values,
						&|a| match projector(a).reflect_mut() {
							ReflectMut::TupleStruct(strukt) => strukt.field_mut(i).unwrap(),
							_ => unreachable!(),
						},
					);
					ui.end_row();
					changed
				})
				.fold(false, or)
		})
	}

	fn ui_for_tuple(
		&mut self,
		value: &mut dyn Tuple,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) -> bool {
		maybe_grid(value.field_len(), ui, id, |ui, label| {
			(0..value.field_len())
				.map(|i| {
					if label {
						ui.label(i.to_string());
					}
					let field = value.field_mut(i).unwrap();
					let changed = self.ui_for_reflect_with_options(
						field,
						ui,
						id.with(i),
						inspector_options_struct_field(options, i),
					);
					ui.end_row();
					changed
				})
				.fold(false, or)
		})
	}

	fn ui_for_tuple_readonly(
		&mut self,
		value: &dyn Tuple,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) {
		maybe_grid_readonly(value.field_len(), ui, id, |ui, label| {
			for i in 0..value.field_len() {
				if label {
					ui.label(i.to_string());
				}
				let field = value.field(i).unwrap();
				self.ui_for_reflect_readonly_with_options(
					field,
					ui,
					id.with(i),
					inspector_options_struct_field(options, i),
				);
				ui.end_row();
			}
		});
	}

	fn ui_for_tuple_many(
		&mut self,
		info: &TupleInfo,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
		values: &mut [&mut dyn PartialReflect],
		projector: impl ProjectorReflect,
	) -> bool {
		maybe_grid(info.field_len(), ui, id, |ui, label| {
			info
				.iter()
				.enumerate()
				.map(|(i, field)| {
					if label {
						ui.label(i.to_string());
					}
					let changed = self.ui_for_reflect_many_with_options(
						field.type_id(),
						field.type_path(),
						ui,
						id.with(i),
						inspector_options_struct_field(options, i),
						values,
						&|a| match projector(a).reflect_mut() {
							ReflectMut::Tuple(strukt) => strukt.field_mut(i).unwrap(),
							_ => unreachable!(),
						},
					);
					ui.end_row();
					changed
				})
				.fold(false, or)
		})
	}

	/// Mutate one or more lists based on a [`ListOp`], generated by some user interaction.
	fn respond_to_list_op<'a>(
		&mut self,
		ui: &mut egui::Ui,
		id: egui::Id,
		lists: impl Iterator<Item = &'a mut dyn List>,
		op: ListOp,
	) -> bool {
		use ListOp::*;
		let mut changed = false;
		let error_id = id.with("error");

		for list in lists {
			let Some(TypeInfo::List(info)) = list.get_represented_type_info() else {
				continue;
			};
			match op {
				AddElement(i) => {
					let default = self
						.get_default_value_for(info.item_ty().id())
						.map(|def| def.into_partial_reflect())
						.or_else(|| list.get(i).map(|v| v.to_dynamic()));
					if let Some(new_value) = default {
						list.insert(i, new_value);
					} else {
						ui.data_mut(|data| data.insert_temp::<bool>(error_id, true));
					}
					changed = true;
				}
				RemoveElement(i) => {
					list.remove(i);
					changed = true;
				}
				MoveElementUp(i) => {
					if let Some(prev_idx) = i.checked_sub(1) {
						// Clone this element and insert it at its index - 1.
						if let Some(element) = list.get(i) {
							let clone = element.to_dynamic();
							list.insert(prev_idx, clone);
						}
						// Remove the original, now at its index + 1.
						list.remove(i + 1);
						changed = true;
					}
				}
				MoveElementDown(i) => {
					// Clone the next element and insert it at this index.
					if let Some(next_element) = list.get(i + 1) {
						let next_clone = next_element.to_dynamic();
						list.insert(i, next_clone);
					}
					// Remove the original, now at i + 2.
					list.remove(i + 2);
					changed = true;
				}
			}
		}
		changed
	}

	fn ui_for_list(
		&mut self,
		list: &mut dyn List,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) -> bool {
		use ListOp::*;
		let mut changed = false;

		ui.vertical(|ui| {
			let mut op = None;
			let len = list.len();
			if len == 0 && ui_for_empty_list(ui) {
				op = Some(AddElement(0))
			}
			for i in 0..len {
				egui::Grid::new((id, i)).show(ui, |ui| {
					ui.label(i.to_string());
					let val = list.get_mut(i).unwrap();
					ui.horizontal_top(|ui| {
						changed |= self.ui_for_reflect_with_options(val, ui, id.with(i), options);
					});
					ui.end_row();

					let item_op = ui_for_list_controls(ui, i, len);
					if item_op.is_some() {
						op = item_op;
					}
				});

				if i != len - 1 {
					ui.separator();
				}
			}

			let Some(TypeInfo::List(info)) = list.get_represented_type_info() else {
				return;
			};
			let error_id = id.with("error");

			// Respond to control interaction
			if let Some(op) = op {
				let lists = std::iter::once(list);
				changed |= self.respond_to_list_op(ui, id, lists, op);
			}

			let error = ui.data_mut(|data| *data.get_temp_mut_or_default::<bool>(error_id));
			if error {
				errors::reflect::no_default_value(ui, info.type_path());
			}
			if ui.input(|input| input.pointer.any_down()) {
				ui.data_mut(|data| data.insert_temp::<bool>(error_id, false));
			}
		});

		changed
	}

	fn ui_for_list_readonly(
		&mut self,
		list: &dyn List,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) {
		ui.vertical(|ui| {
			let len = list.len();
			for i in 0..len {
				let val = list.get(i).unwrap();
				ui.horizontal_top(|ui| {
					self.ui_for_reflect_readonly_with_options(val, ui, id.with(i), options)
				});

				if i != len - 1 {
					ui.separator();
				}
			}
		});
	}

	fn ui_for_list_many(
		&mut self,
		info: &ListInfo,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
		values: &mut [&mut dyn PartialReflect],
		projector: impl ProjectorReflect,
	) -> bool {
		use ListOp::*;
		let mut changed = false;

		let same_len = iter_all_eq(values.iter_mut().map(
			|value| match projector(*value).reflect_mut() {
				ReflectMut::List(l) => l.len(),
				_ => unreachable!(),
			},
		));

		let Some(len) = same_len else {
			ui.label("lists have different sizes, cannot multiedit");
			return changed;
		};

		ui.vertical(|ui| {
			let mut op = None;

			if len == 0 && ui_for_empty_list(ui) {
				op = Some(AddElement(0));
			}

			for i in 0..len {
				let mut items_at_i: Vec<&mut dyn PartialReflect> = values
					.iter_mut()
					.map(|value| match projector(*value).reflect_mut() {
						ReflectMut::List(list) => list.get_mut(i).unwrap(),
						_ => unreachable!(),
					})
					.collect();

				egui::Grid::new((id, i)).show(ui, |ui| {
					ui.label(i.to_string());
					ui.horizontal_top(|ui| {
						changed |= self.ui_for_reflect_many_with_options(
							info.item_ty().id(),
							info.type_path(),
							ui,
							id.with(i),
							options,
							items_at_i.as_mut_slice(),
							&|a| a,
						);
					});
					ui.end_row();
					let item_op = ui_for_list_controls(ui, i, len);
					if item_op.is_some() {
						op = item_op;
					}
				});

				if i != len - 1 {
					ui.separator();
				}
			}

			let error_id = id.with("error");
			let error = ui.data_mut(|data| *data.get_temp_mut_or_default::<bool>(error_id));
			if error {
				errors::reflect::no_default_value(ui, info.type_path());
			}
			if ui.input(|input| input.pointer.any_down()) {
				ui.data_mut(|data| data.insert_temp::<bool>(error_id, false));
			}
			if let Some(op) = op {
				let lists = values
					.iter_mut()
					.map(|l| match projector(*l).reflect_mut() {
						ReflectMut::List(list) => list,
						_ => unreachable!(),
					});
				changed |= self.respond_to_list_op(ui, id, lists, op);
			}
		});

		changed
	}

	fn ui_for_reflect_map(
		&mut self,
		map: &mut dyn Map,
		ui: &mut egui::Ui,
		id: egui::Id,
		_options: &dyn Any,
	) -> bool {
		let mut changed = false;
		if map.is_empty() {
			ui.label("(Empty Map)");
			ui.end_row();
		}

		egui::Grid::new(id).show(ui, |ui| {
			let mut i = 0;
			map.retain(&mut |key, value| {
				let ui_id = id.with(i);
				i += 1;

				self.ui_for_reflect_readonly_with_options(key, ui, ui_id, &());
				changed |= self.ui_for_reflect_with_options(value, ui, ui_id, &());
				let delete = remove_button(ui).on_hover_text("Remove element").clicked();
				ui.end_row();
				!delete
			});

			self.map_add_element_ui(map, ui, id, &mut changed);
		});

		changed
	}

	fn map_add_element_ui(
		&mut self,
		map: &mut (dyn Map + 'static),
		ui: &mut egui::Ui,
		id: egui::Id,
		changed: &mut bool,
	) -> Option<()> {
		let map_draft_id = id.with("map_draft");
		let draft_clone = ui.data_mut(|data| {
			data
				.get_temp_mut_or_default::<Option<MapDraftElement>>(map_draft_id)
				.to_owned()
		});

		let map_info = map.get_represented_map_info()?;

		let key_default = self.get_reflect_default(map_info.key_ty().id())?;
		let value_default = self.get_reflect_default(map_info.value_ty().id())?;

		ui.separator();
		ui.end_row();
		ui.label("New element");
		match draft_clone {
			None => {
				// If no draft element exists, show a button to create one.
				if add_button(ui).clicked() {
					// Insert a temporary 'draft' key-value pair into UI state.
					let key = key_default.default().into_partial_reflect();
					let value = value_default.default().into_partial_reflect();
					ui.data_mut(|data| data.insert_temp(map_draft_id, MapDraftElement { key, value }));
				}
				ui.end_row();
			}
			Some(MapDraftElement { mut key, mut value }) => {
				ui.end_row();
				// Show controls for editing our draft element.
				let key_changed = self.ui_for_reflect_with_options(key.as_mut(), ui, id, &());
				let value_changed = self.ui_for_reflect_with_options(value.as_mut(), ui, id, &());

				// If the clone changed, update the data in UI state.
				if key_changed || value_changed {
					let next_draft = MapDraftElement { key, value };
					ui.data_mut(|data| data.insert_temp(map_draft_id, Some(next_draft)));
				}

				// Show controls to insert the draft into the map, or remove it.
				if ui.button("Insert").clicked() {
					let draft = ui
						.data_mut(|data| data.get_temp::<Option<MapDraftElement>>(map_draft_id))
						.flatten();
					if let Some(draft) = draft {
						map.insert_boxed(draft.key, draft.value);
						ui.data_mut(|data| data.remove_by_type::<Option<MapDraftElement>>());
					}
					*changed = true;
				}

				if ui.button("Cancel").clicked() {
					ui.data_mut(|data| data.remove_by_type::<Option<MapDraftElement>>());
					*changed = true;
				}
				ui.end_row();
			}
		}

		Some(())
	}

	fn ui_for_reflect_map_readonly(
		&mut self,
		map: &dyn Map,
		ui: &mut egui::Ui,
		id: egui::Id,
		_options: &dyn Any,
	) {
		egui::Grid::new(id).show(ui, |ui| {
			for (i, (key, value)) in map.iter().enumerate() {
				let ui_id = id.with(i);
				self.ui_for_reflect_readonly_with_options(key, ui, ui_id, &());
				self.ui_for_reflect_readonly_with_options(value, ui, ui_id, &());
				ui.end_row();
			}
		});
	}

	/// Mutate one or more lists based on a [`SetOp`], generated by some user interaction.
	fn respond_to_sets_op<'a>(
		&mut self,
		sets: impl Iterator<Item = &'a mut dyn Set>,
		op: SetOp,
	) -> bool {
		let mut changed = false;

		for set in sets {
			changed |= self.respond_to_set_op(set, &op);
		}
		changed
	}
	fn respond_to_set_op(&mut self, set: &mut dyn Set, op: &SetOp) -> bool {
		use SetOp::*;
		match &op {
			AddElement(new_value) => {
				set.insert_boxed(new_value.to_dynamic());
			}
			RemoveElement(val) => {
				set.remove(&**val);
			}
		}
		true
	}

	fn ui_for_set(
		&mut self,
		set: &mut dyn Set,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) -> bool {
		use SetOp::*;
		let mut changed = false;

		ui.vertical(|ui| {
			let mut op = None;

			let len = set.len();
			if len == 0 {
				ui_for_empty_set(ui);
			}

			for (i, val) in set.iter().enumerate() {
				egui::Grid::new((id, i)).show(ui, |ui| {
					ui.horizontal_top(|ui| {
						self.ui_for_reflect_readonly_with_options(val, ui, id.with(i), options);
					});
					ui.horizontal_top(|ui| {
						if remove_button(ui).on_hover_text("Remove element").clicked() {
							let copy = val.to_dynamic();
							op = Some(RemoveElement(copy));
						}
					});
					ui.end_row();
				});

				if i != len - 1 {
					ui.separator();
				}
			}
			let Some(TypeInfo::Set(set_info)) = set.get_represented_type_info() else {
				return;
			};
			let value_type = set_info.value_ty();
			let new_op = self.set_add_element_ui(value_type, ui, id, options, &mut changed);
			if new_op.is_some() {
				op = new_op;
			}

			ui.end_row();

			let error_id = id.with("error");

			// Respond to control interaction
			if let Some(op) = op {
				changed |= self.respond_to_set_op(set, &op);
			}

			let error = ui.data_mut(|data| *data.get_temp_mut_or_default::<bool>(error_id));
			if error {
				errors::reflect::no_default_value(ui, set_info.type_path());
			}
			if ui.input(|input| input.pointer.any_down()) {
				ui.data_mut(|data| data.insert_temp::<bool>(error_id, false));
			}
		});

		changed
	}

	#[must_use]
	fn set_add_element_ui(
		&mut self,
		value_type: bevy::reflect::Type,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
		changed: &mut bool,
	) -> Option<SetOp> {
		let mut op = None;

		let item_default = self.get_reflect_default(value_type.id())?.clone();

		ui.vertical(|ui| {
			ui.label("New element");
			let set_draft_id = id.with("set_draft");
			let draft_clone = ui.data_mut(|data| {
				data
					.get_temp_mut_or_default::<Option<SetDraftElement>>(set_draft_id)
					.to_owned()
			});
			ui.end_row();
			match draft_clone {
				None => {
					// If no draft element exists, show a button to create one.
					if add_button(ui).clicked() {
						// Insert a temporary 'draft' value into UI state, once inserted, we cannot modify it.
						let draft = SetDraftElement(item_default.default().into_partial_reflect());
						ui.data_mut(|data| data.insert_temp(set_draft_id, Some(draft)));
					}

					ui.end_row();
				}
				Some(SetDraftElement(mut v)) => {
					ui.end_row();
					// Show controls for editing our draft element.
					// FIXME: is the id passed here correct?
					let value_changed = self.ui_for_reflect_with_options(v.as_mut(), ui, id, options);

					// If the clone changed, update the data in UI state.
					if value_changed {
						let next_draft = SetDraftElement(v);
						ui.data_mut(|data| data.insert_temp(set_draft_id, Some(next_draft)));
					}

					// Show controls to insert the draft into the set, or remove it.
					if ui.button("Insert").clicked() {
						let draft = ui
							.data_mut(|data| data.get_temp::<Option<SetDraftElement>>(set_draft_id))
							.flatten();
						if let Some(draft) = draft {
							op = Some(SetOp::AddElement(draft.0));
							ui.data_mut(|data| data.remove_by_type::<Option<SetDraftElement>>());
						}
						*changed = true;
					}

					if ui.button("Cancel").clicked() {
						ui.data_mut(|data| data.remove_by_type::<Option<SetDraftElement>>());
						*changed = true;
					}
					ui.end_row();
				}
			}
		});

		op
	}

	fn ui_for_set_readonly(
		&mut self,
		set: &dyn Set,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) {
		let len = set.len();
		ui.vertical(|ui| {
			for (i, val) in set.iter().enumerate() {
				ui.horizontal_top(|ui| {
					self.ui_for_reflect_readonly_with_options(val, ui, id.with(i), options)
				});

				if i != len - 1 {
					ui.separator();
				}
			}
		});
	}

	fn ui_for_set_many(
		&mut self,
		info: &SetInfo,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
		values: &mut [&mut dyn PartialReflect],
		projector: impl ProjectorReflect,
	) -> bool {
		use SetOp::*;
		let mut changed = false;

		let same_len = iter_all_eq(values.iter_mut().map(
			|value| match projector(*value).reflect_mut() {
				ReflectMut::List(l) => l.len(),
				_ => unreachable!(),
			},
		));

		let Some(len) = same_len else {
			ui.label("lists have different sizes, cannot multiedit");
			return changed;
		};

		ui.vertical(|ui| {
			let mut op = None;

			if len == 0 {
				ui_for_empty_set(ui)
			}

			let set0 = match projector(values[0]).reflect_mut() {
				ReflectMut::Set(set) => set,
				_ => unreachable!(),
			};
			let Some(TypeInfo::Set(set_info)) = set0.get_represented_type_info() else {
				return;
			};
			let value_type = set_info.value_ty();
			let reflected_values: Vec<Box<dyn PartialReflect>> =
				set0.iter().map(|v| v.to_dynamic()).collect();

			for (i, value_to_check) in reflected_values.iter().enumerate() {
				let value_type_id = (**value_to_check).type_id();
				egui::Grid::new((value_type_id, i)).show(ui, |ui| {
					// Do all sets contain this value ?
					if len == 1
						|| values[1..].iter_mut().all(|set_to_compare| {
							let set_to_compare = match projector(*set_to_compare).reflect_mut() {
								ReflectMut::Set(set) => set,
								_ => unreachable!(),
							};
							set_to_compare
								.iter()
								.any(|value| value.reflect_partial_eq(value_to_check.borrow()) == Some(true))
						}) {
						// All sets contain this value: Show value
						ui.horizontal_top(|ui| {
							self.ui_for_reflect_readonly_with_options(
								value_to_check.borrow(),
								ui,
								// FIXME: is the id passed here correct?
								id.with(i),
								options,
							);
						});
						ui.horizontal_top(|ui| {
							if remove_button(ui).on_hover_text("Remove element").clicked() {
								let copy = value_to_check.to_dynamic();
								op = Some(RemoveElement(copy));
							}
						});
					} else {
						ui.label("Different values");
					}

					ui.end_row();
				});
				if i != len - 1 {
					ui.separator();
				}
			}
			let op = self.set_add_element_ui(value_type, ui, id, options, &mut changed);

			ui.end_row();

			let error_id = id.with("error");
			let error = ui.data_mut(|data| *data.get_temp_mut_or_default::<bool>(error_id));
			if error {
				errors::reflect::no_default_value(ui, info.type_path());
			}
			if ui.input(|input| input.pointer.any_down()) {
				ui.data_mut(|data| data.insert_temp::<bool>(error_id, false));
			}
			if let Some(op) = op {
				let sets = values
					.iter_mut()
					.map(|l| match projector(*l).reflect_mut() {
						ReflectMut::Set(list) => list,
						_ => unreachable!(),
					});
				changed |= self.respond_to_sets_op(sets, op);
			}
		});

		changed
	}

	fn ui_for_array(
		&mut self,
		array: &mut dyn Array,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) -> bool {
		let mut changed = false;

		ui.vertical(|ui| {
			let len = array.len();
			for i in 0..len {
				let val = array.get_mut(i).unwrap();
				ui.horizontal_top(|ui| {
					changed |= self.ui_for_reflect_with_options(val, ui, id.with(i), options);
				});

				if i != len - 1 {
					ui.separator();
				}
			}
		});

		changed
	}

	fn ui_for_array_readonly(
		&mut self,
		array: &dyn Array,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) {
		ui.vertical(|ui| {
			let len = array.len();
			for i in 0..len {
				let val = array.get(i).unwrap();
				ui.horizontal_top(|ui| {
					self.ui_for_reflect_readonly_with_options(val, ui, id.with(i), options);
				});

				if i != len - 1 {
					ui.separator();
				}
			}
		});
	}

	fn ui_for_enum(
		&mut self,
		value: &mut dyn Enum,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) -> bool {
		let Some(type_info) = value.get_represented_type_info() else {
			ui.label("Unrepresentable");
			return false;
		};
		let type_info = match type_info {
			TypeInfo::Enum(info) => info,
			_ => unreachable!("invalid reflect impl: type info mismatch"),
		};

		let mut changed = false;

		ui.vertical(|ui| {
			let changed_variant =
				self.ui_for_enum_variant_select(id, ui, value.variant_index(), type_info);
			if let Some((_new_variant, dynamic_enum)) = changed_variant {
				changed = true;
				value.apply(&dynamic_enum);
			}
			let variant_index = value.variant_index();

			let always_show_label = matches!(value.variant_type(), VariantType::Struct);
			changed |= maybe_grid_label_if(value.field_len(), ui, id, always_show_label, |ui, label| {
				(0..value.field_len())
					.map(|i| {
						if label {
							let field_docs = type_info
								.variant_at(variant_index)
								.and_then(|info| match info {
									VariantInfo::Struct(info) => info.field_at(i)?.docs(),
									_ => None,
								});

							let _response = if let Some(name) = value.name_at(i) {
								ui.label(name)
							} else {
								ui.label(i.to_string())
							};

							show_docs(_response, field_docs);
						}
						let field_value = value
							.field_at_mut(i)
							.expect("invalid reflect impl: field len");
						let changed = self.ui_for_reflect_with_options(
							field_value,
							ui,
							id.with(i),
							inspector_options_enum_variant_field(options, variant_index, i),
						);
						ui.end_row();
						changed
					})
					.fold(false, or)
			});
		});

		changed
	}

	fn ui_for_enum_many(
		&mut self,
		info: &EnumInfo,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
		values: &mut [&mut dyn PartialReflect],
		projector: &dyn ProjectorReflect,
	) -> bool {
		let mut changed = false;

		let same_variant =
			iter_all_eq(
				values
					.iter_mut()
					.map(|value| match projector(*value).reflect_mut() {
						ReflectMut::Enum(info) => info.variant_index(),
						_ => unreachable!(),
					}),
			);

		if let Some(variant_index) = same_variant {
			let mut variant = info.variant_at(variant_index).unwrap();

			ui.vertical(|ui| {
				let variant_changed = self.ui_for_enum_variant_select(id, ui, variant_index, info);
				if let Some((new_variant_idx, dynamic_enum)) = variant_changed {
					changed = true;
					variant = info.variant_at(new_variant_idx).unwrap();

					for value in values.iter_mut() {
						let value = projector(*value);
						value.apply(&dynamic_enum);
					}
				}

				let field_len = match variant {
					VariantInfo::Struct(info) => info.field_len(),
					VariantInfo::Tuple(info) => info.field_len(),
					VariantInfo::Unit(_) => 0,
				};

				let always_show_label = matches!(variant, VariantInfo::Struct(_));
				changed |= maybe_grid_label_if(field_len, ui, id, always_show_label, |ui, label| {
					let handle = |(field_index, field_name, field_type_id, field_type_name)| {
						if label {
							ui.label(field_name);
						}

						let mut variants_across: Vec<&mut dyn PartialReflect> = values
							.iter_mut()
							.map(|value| match projector(*value).reflect_mut() {
								ReflectMut::Enum(value) => value.field_at_mut(field_index).unwrap(),
								_ => unreachable!(),
							})
							.collect();

						self.ui_for_reflect_many_with_options(
							field_type_id,
							field_type_name,
							ui,
							id.with(field_index),
							inspector_options_enum_variant_field(options, variant_index, field_index),
							variants_across.as_mut_slice(),
							&|a| a,
						);

						ui.end_row();

						false
					};

					match variant {
						VariantInfo::Struct(info) => info
							.iter()
							.enumerate()
							.map(|(i, field)| {
								(
									i,
									Cow::Borrowed(field.name()),
									field.type_id(),
									field.type_path(),
								)
							})
							.map(handle)
							.fold(false, or),
						VariantInfo::Tuple(info) => info
							.iter()
							.enumerate()
							.map(|(i, field)| {
								(
									i,
									Cow::Owned(i.to_string()),
									field.type_id(),
									field.type_path(),
								)
							})
							.map(handle)
							.fold(false, or),
						VariantInfo::Unit(_) => false,
					}
				});
			});
		} else {
			ui.label("enums have different selected variants, cannot multiedit");
		}

		changed
	}

	fn ui_for_enum_variant_select(
		&mut self,
		id: egui::Id,
		ui: &mut egui::Ui,
		active_variant_idx: usize,
		info: &EnumInfo,
	) -> Option<(usize, DynamicEnum)> {
		let mut changed_variant = None;

		ui.horizontal_top(|ui| {
			egui::ComboBox::new(id.with("select"), "")
				.selected_text(info.variant_names()[active_variant_idx])
				.show_ui(ui, |ui| {
					for (i, variant) in info.iter().enumerate() {
						let variant_name = variant.name();
						let is_active_variant = i == active_variant_idx;

						let variant_is_constructable = variant_constructable(self.type_registry, variant);

						ui.add_enabled_ui(variant_is_constructable.is_ok(), |ui| {
							let mut variant_label_response = ui.selectable_label(is_active_variant, variant_name);

							if let Err(fields) = variant_is_constructable {
								variant_label_response = variant_label_response.on_disabled_hover_ui(|ui| {
									errors::reflect::unconstructable_variant(
										ui,
										info.type_path(),
										variant_name,
										&fields,
									);
								});
							}

							/*let res = variant_label_response.on_hover_ui(|ui| {
									if !unconstructable_variants.is_empty() {
											errors::unconstructable_variants(
													ui,
													info.type_name(),
													&unconstructable_variants,
											);
									}
							});*/

							if variant_label_response.clicked()
								&& let Ok(dynamic_enum) = self.construct_default_variant(variant, ui)
							{
								changed_variant = Some((i, dynamic_enum));
							};
						});
					}

					false
				});
		});

		changed_variant
	}

	fn ui_for_enum_readonly(
		&mut self,
		value: &dyn Enum,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) {
		ui.vertical(|ui| {
			let active_variant = value.variant_name();
			ui.add_enabled_ui(false, |ui| {
				egui::ComboBox::new(id, "")
					.selected_text(active_variant)
					.show_ui(ui, |_| {})
			});

			let always_show_label = matches!(value.variant_type(), VariantType::Struct);
			maybe_grid_readonly_label_if(value.field_len(), ui, id, always_show_label, |ui, label| {
				for i in 0..value.field_len() {
					if label {
						if let Some(name) = value.name_at(i) {
							ui.label(name);
						} else {
							ui.label(i.to_string());
						}
					}
					let field_value = value.field_at(i).expect("invalid reflect impl: field len");
					self.ui_for_reflect_readonly_with_options(
						field_value,
						ui,
						id.with(i),
						inspector_options_enum_variant_field(options, value.variant_index(), i),
					);
					ui.end_row();
				}
			});
		});
	}
}

type InspectorEguiImplFn =
	fn(&mut dyn Any, &mut egui::Ui, &dyn Any, egui::Id, InspectorUi<'_, '_>) -> bool;

type InspectorEguiImplFnReadonly =
	fn(&dyn Any, &mut egui::Ui, &dyn Any, egui::Id, InspectorUi<'_, '_>);

type InspectorEguiImplFnMany = for<'a> fn(
	&mut egui::Ui,
	&dyn Any,
	egui::Id,
	InspectorUi<'_, '_>,
	&mut [&mut dyn PartialReflect],
	&dyn ProjectorReflect,
) -> bool;

#[derive(Clone)]
pub struct InspectorEguiImpl {
	fn_mut: InspectorEguiImplFn,
	fn_readonly: InspectorEguiImplFnReadonly,
	fn_many: InspectorEguiImplFnMany,
}

impl InspectorEguiImpl {
	pub fn of<T: InspectorPrimitive + PartialEq + Clone + Default>() -> Self {
		InspectorEguiImpl {
			fn_mut: ui_vtable::<T>,
			fn_readonly: ui_readonly_vtable::<T>,
			fn_many: ui_many_vtable::<T>,
		}
	}
	pub fn of_with_many<T: InspectorPrimitive>(fn_many: InspectorEguiImplFnMany) -> Self {
		InspectorEguiImpl {
			fn_mut: ui_vtable::<T>,
			fn_readonly: ui_readonly_vtable::<T>,
			fn_many,
		}
	}

	/// Create a new [`InspectorEguiImpl`] from functions displaying a type
	pub fn new(
		fn_mut: InspectorEguiImplFn,
		fn_readonly: InspectorEguiImplFnReadonly,
		fn_many: InspectorEguiImplFnMany,
	) -> Self {
		InspectorEguiImpl {
			fn_mut,
			fn_readonly,
			fn_many,
		}
	}

	pub fn execute<'a, 'c: 'a>(
		&'a self,
		value: &mut dyn Any,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: InspectorUi<'_, '_>,
	) -> bool {
		(self.fn_mut)(value, ui, options, id, env)
	}
	pub fn execute_readonly<'a, 'c: 'a>(
		&'a self,
		value: &dyn Any,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: InspectorUi<'_, '_>,
	) {
		(self.fn_readonly)(value, ui, options, id, env)
	}
	pub fn execute_many<'a, 'c: 'a, 'e>(
		&'a self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: InspectorUi<'_, '_>,
		values: &mut [&mut dyn PartialReflect],
		projector: &dyn ProjectorReflect,
	) -> bool {
		(self.fn_many)(ui, options, id, env, values, projector)
	}
}

impl<T: InspectorPrimitive> FromType<T> for InspectorEguiImpl {
	fn from_type() -> Self {
		InspectorEguiImpl::of_with_many::<T>(many_unimplemented::<T>)
	}
}

#[derive(Debug)]
pub struct Filter<F: QueryFilter = Without<ChildOf>> {
	pub word: String,
	pub is_fuzzy: bool,
	pub marker: PhantomData<F>,
}

impl<F: QueryFilter + Clone> Clone for Filter<F> {
	fn clone(&self) -> Self {
		Self {
			word: self.word.clone(),
			is_fuzzy: self.is_fuzzy,
			marker: PhantomData,
		}
	}
}

impl<F: QueryFilter> Filter<F> {
	pub fn from_ui_fuzzy(ui: &mut egui::Ui, id: egui::Id) -> Self {
		ui.horizontal(|ui| {
			let word = {
				let id = id.with("word");
				// filter, using eguis memory and provided id
				let mut filter_string = ui.memory_mut(|mem| {
					let filter: &mut String = mem.data.get_persisted_mut_or_default(id);
					filter.clone()
				});
				ui.add(egui::TextEdit::singleline(&mut filter_string).desired_width(180.));
				ui.memory_mut(|mem| {
					*mem.data.get_persisted_mut_or_default(id) = filter_string.clone();
				});

				// improves overall matching
				filter_string.to_lowercase()
			};

			Filter {
				word,
				is_fuzzy: true,
				marker: PhantomData,
			}
		})
		.inner
	}

	pub fn from_ui(ui: &mut egui::Ui, id: egui::Id) -> Self {
		ui.horizontal(|ui| {
			// filter kind
			let is_fuzzy = {
				let id = id.with("is_fuzzy");
				let mut is_fuzzy = ui.memory_mut(|mem| {
					let fuzzy: &mut bool = mem.data.get_persisted_mut_or_default(id);
					*fuzzy
				});
				ui.checkbox(&mut is_fuzzy, "Fuzzy");
				ui.memory_mut(|mem| {
					*mem.data.get_persisted_mut_or_default(id) = is_fuzzy;
				});
				is_fuzzy
			};
			let word = {
				let id = id.with("word");
				// filter, using eguis memory and provided id
				let mut filter_string = ui.memory_mut(|mem| {
					let filter: &mut String = mem.data.get_persisted_mut_or_default(id);
					filter.clone()
				});
				ui.text_edit_singleline(&mut filter_string);
				ui.memory_mut(|mem| {
					*mem.data.get_persisted_mut_or_default(id) = filter_string.clone();
				});

				// improves overall matching
				filter_string.to_lowercase()
			};

			Filter {
				word,
				is_fuzzy,
				marker: PhantomData,
			}
		})
		.inner
	}

	/// empty filter which does nothing
	pub fn all() -> Self {
		Self {
			word: String::from(""),
			is_fuzzy: false,
			marker: PhantomData,
		}
	}
}

impl<F: QueryFilter> EntityFilter for Filter<F> {
	type StaticFilter = F;

	fn is_active(&self) -> bool {
		!self.word.is_empty()
	}

	fn filter_entity(&self, world: &mut World, entity: Entity) -> bool {
		self_or_children_satisfy_filter(world, entity, self.word.as_str(), self.is_fuzzy)
	}
}

fn self_or_children_satisfy_filter(
	world: &mut World,
	entity: Entity,
	filter: &str,
	is_fuzzy: bool,
) -> bool {
	let name = entity::guess_entity_name(world, entity);

	let self_matches = if is_fuzzy {
		let matcher = SkimMatcherV2::default();
		matcher.fuzzy_match(name.as_str(), filter).is_some()
	} else {
		name.to_lowercase().contains(filter)
	};
	self_matches || {
		let Ok(children) = world
			.query::<&Children>()
			.get(world, entity)
			.map(|children| children.to_vec())
		else {
			return false;
		};

		children
			.into_iter()
			.any(|child| self_or_children_satisfy_filter(world, child, filter, is_fuzzy))
	}
}

pub trait EntityFilter {
	type StaticFilter: QueryFilter;

	/// Returns true if the filter term is currently active
	///
	/// Used in the default impl of [`EntityFilter::filter_entities`] to skip filtering if false
	///
	/// default impl is true
	fn is_active(&self) -> bool {
		true
	}

	/// Filters entities in place
	///
	/// default impl:
	/// - uses [`EntityFilter::filter_entity`] to mark what entities to retain
	/// - skips filtering if [`EntityFilter::is_active`] returns false
	fn filter_entities(&self, world: &mut World, entities: &mut Vec<Entity>) {
		if !self.is_active() {
			return;
		}
		entities.retain(|&entity| self.filter_entity(world, entity));
	}

	/// Returns true if entity matches the filter term
	fn filter_entity(&self, world: &mut World, entity: Entity) -> bool;
}

pub(crate) fn change_slider<T>(
	ui: &mut egui::Ui,
	id: egui::Id,
	same: Option<T>,
	f: impl FnOnce(T, bool),
) -> bool
where
	T: egui::emath::Numeric + std::ops::Sub<Output = T> + Default + Send + Sync + 'static,
{
	let speed = if T::INTEGRAL { 1.0 } else { 0.1 };

	match same {
		Some(mut same) => {
			let widget = egui::DragValue::new(&mut same).speed(speed);

			let changed = ui.add(widget).changed();
			if changed {
				f(same, true);
			}

			changed
		}
		None => {
			let old_change = ui.memory_mut(|memory| *memory.data.get_temp_mut_or_default::<T>(id));
			let mut change = old_change;

			let widget = egui::DragValue::new(&mut change)
				.speed(speed)
				.custom_formatter(|_, _| "-".to_string());

			let changed = ui.add(widget).changed();
			if changed {
				f(change - old_change, false);
			}

			ui.memory_mut(|memory| *memory.data.get_temp_mut_or_default(id) = change);
			changed
		}
	}
}

pub(crate) fn iter_all_eq<T: PartialEq>(mut iter: impl Iterator<Item = T>) -> Option<T> {
	let first = iter.next()?;
	iter.all(|elem| elem == first).then_some(first)
}

#[macro_export]
#[doc(hidden)]
macro_rules! many_ui {
	($name:ident $inner:ident $ty:ty) => {
		pub fn $name(
			ui: &mut egui::Ui,
			options: &dyn Any,
			id: egui::Id,
			env: InspectorUi<'_, '_>,
			values: &mut [&mut dyn bevy::reflect::PartialReflect],
			projector: &dyn $crate::inspector::ui::ProjectorReflect,
		) -> bool {
			let same = $crate::inspector::ui::iter_all_eq(
				values
					.iter_mut()
					.map(|value| projector(*value).try_downcast_ref::<$ty>().unwrap()),
			);

			let mut temp = same.cloned().unwrap_or_default();
			if $inner(&mut temp, ui, options, id, env) {
				for value in values.iter_mut() {
					let value = projector(*value).try_downcast_mut::<$ty>().unwrap();
					*value = temp.clone();
				}

				return true;
			}
			false
		}
	};
}

fn ui_vtable<T: InspectorPrimitive>(
	val: &mut dyn Any,
	ui: &mut egui::Ui,
	options: &dyn Any,
	id: egui::Id,
	env: InspectorUi<'_, '_>,
) -> bool {
	let val = val.downcast_mut::<T>().unwrap();
	T::ui(val, ui, options, id, env)
}

fn ui_readonly_vtable<T: InspectorPrimitive>(
	val: &dyn Any,
	ui: &mut egui::Ui,
	options: &dyn Any,
	id: egui::Id,
	env: InspectorUi<'_, '_>,
) {
	let val = val.downcast_ref::<T>().unwrap();
	T::ui_readonly(val, ui, options, id, env)
}

fn ui_many_vtable<T: Reflect + PartialEq + Clone + Default + InspectorPrimitive>(
	ui: &mut egui::Ui,
	options: &dyn Any,
	id: egui::Id,
	env: InspectorUi<'_, '_>,
	values: &mut [&mut dyn PartialReflect],
	projector: &dyn ProjectorReflect,
) -> bool {
	let same = iter_all_eq(values.iter_mut().map(|value| {
		projector(*value)
			.try_downcast_mut::<T>()
			.expect("non-fully-reflected value passed to ui_many_vtable")
	}));

	let mut temp = same.cloned().unwrap_or_default();
	if T::ui(&mut temp, ui, options, id, env) {
		for value in values.iter_mut() {
			let value = projector(*value)
				.try_downcast_mut::<T>()
				.expect("non-fully-reflected value passed to ui_many_vtable");
			*value = temp.clone();
		}

		return true;
	}
	false
}

fn ui_for_empty_collection(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>) -> bool {
	let mut add = false;

	ui.vertical_centered(|ui| {
		ui.label(label);
		if add_button(ui).on_hover_text("Add element").clicked() {
			add = true;
		}
	});

	add
}

fn ui_for_empty_list(ui: &mut egui::Ui) -> bool {
	ui_for_empty_collection(ui, "(Empty List)")
}

fn ui_for_empty_set(ui: &mut egui::Ui) {
	ui.vertical_centered(|ui| ui.label("(Empty Set)"));
}

fn variant_constructable<'a>(
	type_registry: &TypeRegistry,
	variant: &'a VariantInfo,
) -> Result<(), Vec<&'a str>> {
	let type_id_is_constructable = |type_id: TypeId| {
		type_registry
			.get_type_data::<ReflectDefault>(type_id)
			.is_some()
	};

	let unconstructable_fields: Vec<&'a str> = match variant {
		VariantInfo::Struct(variant) => variant
			.iter()
			.filter_map(|field| (!type_id_is_constructable(field.type_id())).then_some(field.type_path()))
			.collect(),
		VariantInfo::Tuple(variant) => variant
			.iter()
			.filter_map(|field| (!type_id_is_constructable(field.type_id())).then_some(field.type_path()))
			.collect(),
		VariantInfo::Unit(_) => return Ok(()),
	};

	if unconstructable_fields.is_empty() {
		Ok(())
	} else {
		Err(unconstructable_fields)
	}
}

fn inspector_options_struct_field(options: &dyn Any, field: usize) -> &dyn Any {
	options
		.downcast_ref::<InspectorOptions>()
		.and_then(|options| options.get(Target::Field(field)))
		.unwrap_or(&())
}

fn inspector_options_enum_variant_field(
	options: &dyn Any,
	variant_index: usize,
	field_index: usize,
) -> &dyn Any {
	options
		.downcast_ref::<InspectorOptions>()
		.and_then(|options| {
			options.get(Target::VariantField {
				variant_index,
				field_index,
			})
		})
		.unwrap_or(&())
}

fn get_type_data<'a>(
	type_registry: &'a TypeRegistry,
	type_id: &dyn DynamicTyped,
) -> Result<&'a InspectorEguiImpl, TypeDataError> {
	let registration = type_registry
		.get(type_id.reflect_type_info().type_id())
		.ok_or(TypeDataError::NotRegistered)?;
	let data = registration
		.data::<InspectorEguiImpl>()
		.ok_or(TypeDataError::NoTypeData)?;
	Ok(data)
}

struct MapDraftElement {
	key: Box<dyn PartialReflect>,
	value: Box<dyn PartialReflect>,
}
impl Clone for MapDraftElement {
	fn clone(&self) -> Self {
		Self {
			key: self.key.to_dynamic(),
			value: self.value.to_dynamic(),
		}
	}
}

struct SetDraftElement(Box<dyn PartialReflect>);

impl Clone for SetDraftElement {
	fn clone(&self) -> Self {
		Self(self.0.to_dynamic())
	}
}

enum ListOp {
	AddElement(usize),
	RemoveElement(usize),
	MoveElementUp(usize),
	MoveElementDown(usize),
}

fn ui_for_list_controls(ui: &mut egui::Ui, index: usize, len: usize) -> Option<ListOp> {
	use ListOp::*;

	let mut op = None;

	ui.horizontal_top(|ui| {
		if add_button(ui).on_hover_text("Add element").clicked() {
			op = Some(AddElement(index));
		}

		if remove_button(ui).on_hover_text("Remove element").clicked() {
			op = Some(RemoveElement(index));
		}

		let up_enabled = index > 0;
		ui.add_enabled_ui(up_enabled, |ui| {
			if up_button(ui).on_hover_text("Move element up").clicked() {
				op = Some(MoveElementUp(index));
			}
		});

		let down_enabled = len.checked_sub(1).map(|l| index < l).unwrap_or(false);
		ui.add_enabled_ui(down_enabled, |ui| {
			if down_button(ui).on_hover_text("Move element down").clicked() {
				op = Some(MoveElementDown(index));
			}
		});
	});

	op
}

enum SetOp {
	RemoveElement(Box<dyn PartialReflect>),
	AddElement(Box<dyn PartialReflect>),
}

pub mod short_circuit {
	use super::errors::{self, name_of_type};
	use crate::{
		Notification,
		inspector::ui::{Context, InspectorUi, ProjectorReflect},
	};
	use bevy::{
		asset::{ReflectAsset, ReflectHandle},
		prelude::*,
	};
	use std::any::{Any, TypeId};

	pub fn short_circuit(
		env: &mut InspectorUi,
		value: &mut dyn PartialReflect,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) -> Option<bool> {
		let Some(Context { world, queue }) = &mut env.context else {
			return Some(false);
		};

		let reflected_value = value.try_as_reflect()?;

		let type_id = reflected_value.type_id();

		let reflect_handle = env.type_registry.get_type_data::<ReflectHandle>(type_id)?;

		let Some(handle) = reflect_handle.downcast_handle_untyped(reflected_value.as_any()) else {
			errors::no_asset_handle(ui, &name_of_type(type_id, env.type_registry));
			return Some(false);
		};

		let handle_id = handle.id();

		let Some(reflect_asset) = env
			.type_registry
			.get_type_data::<ReflectAsset>(reflect_handle.asset_type_id())
		else {
			errors::no_type_data(
				ui,
				&name_of_type(reflect_handle.asset_type_id(), env.type_registry),
				"ReflectAsset",
			);
			return Some(false);
		};

		let (assets_view, world) = world.split_off_resource(reflect_asset.assets_resource_type_id());

		assert!(assets_view.allows_access_to_resource(reflect_asset.assets_resource_type_id()));

		let asset_value = {
			// SAFETY: the world allows mutable access to `Assets<T>`
			let asset_value = unsafe { reflect_asset.get_unchecked_mut(world.world(), &handle) };

			match asset_value {
				Some(value) => value,
				None => {
					errors::dead_asset_handle(ui, handle_id);
					return Some(false);
				}
			}
		};

		let mut restricted_env = InspectorUi {
			type_registry: env.type_registry,
			context: Some(&mut Context::new(world, queue)),
		};

		Some(restricted_env.ui_for_reflect_with_options(
			asset_value.as_partial_reflect_mut(),
			ui,
			id.with("asset"),
			options,
		))
	}

	pub fn short_circuit_many(
		env: &mut InspectorUi,
		type_id: TypeId,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
		values: &mut [&mut dyn PartialReflect],
		projector: &dyn ProjectorReflect,
	) -> Option<bool> {
		let Some(Context { world, queue }) = &mut env.context else {
			return Some(false);
		};

		let reflect_handle = env.type_registry.get_type_data::<ReflectHandle>(type_id)?;

		let Some(reflect_asset) = env
			.type_registry
			.get_type_data::<ReflectAsset>(reflect_handle.asset_type_id())
		else {
			errors::no_type_data(
				ui,
				&name_of_type(reflect_handle.asset_type_id(), env.type_registry),
				"ReflectAsset",
			);
			return Some(false);
		};

		let (assets_view, world) = world.split_off_resource(reflect_asset.assets_resource_type_id());

		let mut new_values = Vec::with_capacity(values.len());
		let mut used_handles = Vec::with_capacity(values.len());

		for value in values {
			let handle = projector(*value);
			let Some(handle) = handle.try_as_reflect() else {
				// Edge case, continue as normal:
				// this for loop should only work if we're multi-editing a bunch of Handles
				return None;
			};
			let handle = reflect_handle
				.downcast_handle_untyped(handle.as_any())
				.unwrap();
			let handle_id = handle.id();

			if used_handles.contains(&handle_id) {
				continue;
			};
			used_handles.push(handle_id);

			let asset_value = {
				assert!(assets_view.allows_access_to_resource(reflect_asset.assets_resource_type_id()));

				// SAFETY: the world allows mutable access to `Assets<T>`
				let asset_value = unsafe { reflect_asset.get_unchecked_mut(world.world(), &handle) };

				match asset_value {
					Some(value) => value,
					None => {
						errors::dead_asset_handle(ui, handle_id);
						return Some(false);
					}
				}
			};

			new_values.push(asset_value.as_partial_reflect_mut());
		}

		let mut restricted_env = InspectorUi {
			type_registry: env.type_registry,
			context: Some(&mut Context::new(world, queue)),
		};

		Some(restricted_env.ui_for_reflect_many_with_options(
			reflect_handle.asset_type_id(),
			"",
			ui,
			id.with("asset"),
			options,
			new_values.as_mut_slice(),
			&|a| a,
		))
	}

	pub fn short_circuit_readonly(
		env: &mut InspectorUi,
		value: &dyn PartialReflect,
		ui: &mut egui::Ui,
		id: egui::Id,
		options: &dyn Any,
	) -> Option<()> {
		let Some(Context { world, queue }) = &mut env.context else {
			return Some(());
		};

		let value = value.try_as_reflect()?;

		let reflect_handle = env
			.type_registry
			.get_type_data::<ReflectHandle>(value.type_id())?;

		let handle = reflect_handle
			.downcast_handle_untyped(value.as_any())
			.unwrap();

		let handle_id = handle.id();

		let Some(reflect_asset) = env
			.type_registry
			.get_type_data::<ReflectAsset>(reflect_handle.asset_type_id())
		else {
			errors::no_type_data(
				ui,
				&name_of_type(reflect_handle.asset_type_id(), env.type_registry),
				"ReflectAsset",
			);
			return Some(());
		};

		let (assets_view, world) = world.split_off_resource(reflect_asset.assets_resource_type_id());

		let asset_value = {
			assert!(assets_view.allows_access_to_resource(reflect_asset.assets_resource_type_id()));

			// SAFETY: the following code only accesses a resources it has access to, `Assets<T>`
			let asset_value = unsafe { reflect_asset.get(assets_view.world().world(), &handle) };

			match asset_value {
				Some(value) => value,
				None => {
					errors::dead_asset_handle(ui, handle_id);
					return Some(());
				}
			}
		}
		.as_partial_reflect();

		let mut restricted_env = InspectorUi {
			type_registry: env.type_registry,
			context: Some(&mut Context::new(world, queue)),
		};

		match handle {
			UntypedHandle::Strong(strong_handle) => {
				if ui.button("Make Persistent").clicked() {
					warn!("TODO");
				}
			}
			UntypedHandle::Uuid { type_id, uuid } => {
				if ui.button("Save Asset").clicked() {
					warn!("TODO");
				}
			}
		}

		restricted_env.ui_for_reflect_readonly_with_options(asset_value, ui, id.with("asset"), options);

		Some(())
	}
}
