use crate::{
	inspector::{
		InspectorPrimitive, InspectorPrimitiveMultiedit,
		options::{InspectorOptionsType, NumberDisplay, NumberOptions, RangeOptions},
		ui::{ImmutableContext, InspectorUi, MutableContext, change_slider, get_one_if_all_equal},
	},
	private::util::egui::layout_job,
};
use bevy::{platform::time::Instant, prelude::*};
use egui::{DragValue, RichText, TextBuffer, emath::Numeric};
use smallvec::SmallVec;
use std::{any::Any, time::Duration};
use std::{
	any::TypeId,
	borrow::Cow,
	ops::{AddAssign, Sub},
	path::PathBuf,
};

macro_rules! impl_many_for_numerics {
  ($($ty:ty),*) => {
    $(
      impl InspectorPrimitiveMultiedit for $ty {
        fn ui(
          &self,
          ui: &mut egui::Ui,
          options: &dyn Any,
          _: egui::Id,
          _: &InspectorUi<ImmutableContext>,
        ) {
          let options = options
            .downcast_ref::<NumberOptions<Self>>()
            .cloned()
            .unwrap_or_default();
          let decimal_range = 0..=1usize;
          ui.add(
            egui::Button::new(
              RichText::new(format!(
                "{}{}{}",
                options.prefix,
                egui::emath::format_with_decimals_in_range(self.to_f64(), decimal_range),
                options.suffix
              ))
              .monospace(),
            )
            .truncate()
            .sense(egui::Sense::hover()),
          );
        }


        fn ui_mut<'c>(
          &mut self,
          ui: &mut egui::Ui,
          options: &dyn Any,
          _: egui::Id,
          _: &mut InspectorUi<MutableContext>,
        ) -> bool {
          let options = options
            .downcast_ref::<NumberOptions<Self>>()
            .cloned()
            .unwrap_or_default();
          display_number(self, &options, ui, 0.1)
        }

        fn ui_mut_multiedit<'s, 'c>(
          ui: &mut egui::Ui,
          options: &dyn Any,
          id: egui::Id,
          env: &mut InspectorUi<'_, MutableContext<'c>>,
          values: impl Iterator<Item = &'s mut Self>,
        ) -> bool
        where
          Self: 's,
        {
          number_ui_many(ui, options, id, env, values)
        }
      }
    )*
  };
}

impl_many_for_numerics!(f32, f64, i8, u8, i16, u16, i32, u32, i64, u64, isize, usize);

pub fn number_ui<T: egui::emath::Numeric>(
	value: &dyn Any,
	ui: &mut egui::Ui,
	options: &dyn Any,
	_: egui::Id,
	_: &InspectorUi<ImmutableContext>,
) {
	let value = value.downcast_ref::<T>().unwrap();
	let options = options
		.downcast_ref::<NumberOptions<T>>()
		.cloned()
		.unwrap_or_default();
	let decimal_range = 0..=1usize;
	ui.add(
		egui::Button::new(
			RichText::new(format!(
				"{}{}{}",
				options.prefix,
				egui::emath::format_with_decimals_in_range(value.to_f64(), decimal_range),
				options.suffix
			))
			.monospace(),
		)
		.truncate()
		.sense(egui::Sense::hover()),
	);
}

pub fn number_ui_mut<T: egui::emath::Numeric>(
	value: &mut dyn Any,
	ui: &mut egui::Ui,
	options: &dyn Any,
	_: egui::Id,
	_: &mut InspectorUi<MutableContext>,
) -> bool {
	let value = value.downcast_mut::<T>().unwrap();
	let options = options
		.downcast_ref::<NumberOptions<T>>()
		.cloned()
		.unwrap_or_default();
	display_number(value, &options, ui, 0.1)
}

fn display_number<T: egui::emath::Numeric>(
	value: &mut T,
	options: &NumberOptions<T>,
	ui: &mut egui::Ui,
	default_speed: f32,
) -> bool {
	let mut changed = match options.display {
		NumberDisplay::Drag => {
			let mut widget = egui::DragValue::new(value);
			if !options.prefix.is_empty() {
				widget = widget.prefix(&options.prefix);
			}
			if !options.suffix.is_empty() {
				widget = widget.suffix(&options.suffix);
			}
			match (options.min, options.max) {
				(Some(min), Some(max)) => widget = widget.range(min.to_f64()..=max.to_f64()),
				(Some(min), None) => widget = widget.range(min.to_f64()..=f64::MAX),
				(None, Some(max)) => widget = widget.range(f64::MIN..=max.to_f64()),
				(None, None) => {}
			}
			if options.speed != 0.0 {
				widget = widget.speed(options.speed);
			} else {
				widget = widget.speed(default_speed);
			}
			ui.add(widget).changed()
		}
		NumberDisplay::Slider => {
			let min = options.min.unwrap_or_else(|| T::from_f64(0.0));
			let max = options.max.unwrap_or_else(|| T::from_f64(1.0));
			let range = min..=max;
			let widget = egui::Slider::new(value, range);
			ui.add(widget).changed()
		}
	};

	if let Some(min) = options.min {
		let as_f64 = value.to_f64();
		let min = min.to_f64();
		if as_f64 < min {
			*value = T::from_f64(min);
			changed = true;
		}
	}
	if let Some(max) = options.max {
		let as_f64 = value.to_f64();
		let max = max.to_f64();
		if as_f64 > max {
			*value = T::from_f64(max);
			changed = true;
		}
	}
	changed
}

pub fn number_ui_many<'s, T>(
	ui: &mut egui::Ui,
	_: &dyn Any,
	id: egui::Id,
	_env: &mut InspectorUi<MutableContext>,
	values: impl Iterator<Item = &'s mut T>,
) -> bool
where
	T: Reflect + egui::emath::Numeric + AddAssign<T> + Sub<Output = T> + Default,
{
	let values = values.collect::<SmallVec<[_; 8]>>();
	let same = get_one_if_all_equal(values.iter()).map(|v| **v);

	change_slider(ui, id, same, |change, overwrite| {
		for value in values.into_iter() {
			if overwrite {
				*value = change;
			} else {
				*value += change;
			}
		}
	})
}

impl InspectorPrimitive for bool {
	fn ui_mut(
		&mut self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		_: &mut InspectorUi<MutableContext>,
	) -> bool {
		ui.checkbox(self, "").changed()
	}

	fn ui(&self, ui: &mut egui::Ui, _: &dyn Any, _: egui::Id, _: &InspectorUi<ImmutableContext>) {
		let mut copy = *self;
		ui.add_enabled_ui(false, |ui| {
			ui.checkbox(&mut copy, "");
		});
	}
}

impl InspectorPrimitive for String {
	fn ui_mut(
		&mut self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		_: &mut InspectorUi<MutableContext>,
	) -> bool {
		if self.contains('\n') {
			ui.text_edit_multiline(self).changed()
		} else {
			ui.text_edit_singleline(self).changed()
		}
	}

	fn ui(&self, ui: &mut egui::Ui, _: &dyn Any, _: egui::Id, _: &InspectorUi<ImmutableContext>) {
		if self.contains('\n') {
			ui.text_edit_multiline(&mut self.as_str());
		} else {
			ui.text_edit_singleline(&mut self.as_str());
		}
	}
}

impl InspectorPrimitive for Cow<'static, str> {
	fn ui_mut(
		&mut self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		_: &mut InspectorUi<MutableContext>,
	) -> bool {
		let mut clone = self.to_string();
		let changed = if self.contains('\n') {
			ui.text_edit_multiline(&mut clone).changed()
		} else {
			ui.text_edit_singleline(&mut clone).changed()
		};

		if changed {
			*self = Cow::Owned(clone);
		}

		changed
	}

	fn ui(&self, ui: &mut egui::Ui, _: &dyn Any, _: egui::Id, _: &InspectorUi<ImmutableContext>) {
		if self.contains('\n') {
			ui.text_edit_multiline(&mut self.as_str());
		} else {
			ui.text_edit_singleline(&mut self.as_str());
		}
	}
}

impl InspectorPrimitive for Duration {
	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		id: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		let mut seconds = self.as_secs_f64();
		let options = NumberOptions {
			min: Some(0.0f64),
			suffix: "s".to_string(),
			..Default::default()
		};

		let changed = env.ui_for_reflect_mut_with_options(&mut seconds, ui, id, &options);
		if changed {
			*self = Duration::from_secs_f64(seconds);
		}
		changed
	}

	fn ui<'c>(
		&self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		id: egui::Id,
		env: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		let seconds = self.as_secs_f64();
		let options = NumberOptions {
			min: Some(0.0f64),
			suffix: "s".to_string(),
			..Default::default()
		};
		env.ui_for_reflect_with_options(&seconds, ui, id, &options);
	}
}

impl InspectorPrimitive for Instant {
	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		_: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		let mut secs = self.elapsed().as_secs_f32();
		ui.horizontal(|ui| {
			ui.add_enabled(false, DragValue::new(&mut secs));
			ui.label("seconds ago");
		});
		false
	}

	fn ui<'c>(
		&self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		_: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		let mut secs = self.elapsed().as_secs_f32();
		ui.horizontal(|ui| {
			ui.add_enabled(false, DragValue::new(&mut secs));
			ui.label("seconds ago");
		});
	}
}

impl<T: Reflect + TypePath + egui::emath::Numeric + InspectorOptionsType> InspectorPrimitive
	for std::ops::Range<T>
{
	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		let std::ops::Range { start, end } = self;
		display_range_mut::<T>(ui, options, id, env, "..", Some(start), Some(end))
	}

	fn ui<'c>(
		&self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		let std::ops::Range { start, end } = self;
		display_range::<T>(ui, options, id, env, "..", Some(start), Some(end));
	}
}

fn display_range<'c, T: egui::emath::Numeric + InspectorOptionsType>(
	ui: &mut egui::Ui,
	options: &dyn Any,
	id: egui::Id,
	env: &InspectorUi<'_, ImmutableContext<'c>>,

	symbol: &'static str,
	start: Option<&T>,
	end: Option<&T>,
) {
	let options = options.downcast_ref::<RangeOptions<T>>();

	let start_options = options.map(|a| &a.start as &dyn Any).unwrap_or(&());
	let end_options = options.as_ref().map(|a| &a.end as &dyn Any).unwrap_or(&());

	ui.horizontal(|ui| {
		if let Some(start) = start {
			number_ui::<T>(start, ui, start_options, id, env);
		}
		ui.label(symbol);
		if let Some(end) = end {
			number_ui::<T>(end, ui, end_options, id, env);
		}
	});
}

fn display_range_mut<'c, T: egui::emath::Numeric + InspectorOptionsType>(
	ui: &mut egui::Ui,
	options: &dyn Any,
	id: egui::Id,
	env: &mut InspectorUi<'_, MutableContext<'c>>,

	// this is made to be generic but I'm currently just using it for a..b, not a..=b, ..a, a.., .., etc., because these types don't hand out mutable references
	symbol: &'static str,
	start: Option<&mut T>,
	end: Option<&mut T>,
) -> bool {
	let options = options.downcast_ref::<RangeOptions<T>>();

	let start_options = options.map(|a| &a.start as &dyn Any).unwrap_or(&());
	let end_options = options.map(|a| &a.end as &dyn Any).unwrap_or(&());

	let mut changed = false;
	ui.horizontal(|ui| {
		if let Some(start) = start {
			changed |= number_ui_mut::<T>(start, ui, start_options, id, env);
		}
		ui.label(symbol);
		if let Some(end) = end {
			changed |= number_ui_mut::<T>(end, ui, end_options, id, env);
		}
	});

	changed
}

impl<T: Reflect + TypePath + egui::emath::Numeric + InspectorOptionsType> InspectorPrimitive
	for std::ops::RangeInclusive<T>
{
	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		let mut start = *self.start();
		let mut end = *self.end();

		let changed = display_range_mut::<T>(
			ui,
			options,
			id,
			env,
			"..=",
			Some(&mut start),
			Some(&mut end),
		);

		if changed {
			*self = start..=end;
		}

		changed
	}

	fn ui<'c>(
		&self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		display_range::<T>(
			ui,
			options,
			id,
			env,
			"..",
			Some(self.start()),
			Some(self.end()),
		);
	}
}

impl InspectorPrimitive for PathBuf {
	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		_: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		let mut str = self.to_string_lossy();
		let changed = ui.text_edit_singleline(&mut str).changed();

		if changed {
			*self = PathBuf::from(str.as_str());
		}

		changed
	}

	fn ui<'c>(
		&self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		_: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		ui.text_edit_singleline(&mut self.to_string_lossy());
	}
}

impl InspectorPrimitive for TypeId {
	fn ui<'c>(
		&self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		id: egui::Id,
		env: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		use std::sync::Arc;

		let data_id = id.with("type-id-str");

		let text = match ui.data_mut(|data| data.remove_temp::<egui::WidgetText>(data_id)) {
			Some(label) => label,
			None => {
				let job = env
					.type_registry
					.get_type_info(*self)
					.map(|ti| {
						let type_str = ti.type_path();
						layout_job(&[
							(egui::FontId::proportional(12.0), "TypeId("),
							(egui::FontId::monospace(13.0), type_str),
							(egui::FontId::proportional(12.0), ")"),
						])
					})
					.unwrap_or_else(|| {
						let type_str = format!("{:?}", self);
						layout_job(&[(egui::FontId::default(), &type_str)])
					});

				egui::WidgetText::LayoutJob(Arc::new(job))
			}
		};

		ui.label(text.clone());

		ui.data_mut(|data| data.insert_temp(data_id, text));
	}

	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		env.as_immutable(|env| {
			Self::ui(self, ui, options, id, &env);
		});
		false
	}
}
