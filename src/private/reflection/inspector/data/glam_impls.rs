use crate::inspector::{
	InspectorPrimitive, InspectorPrimitiveMultiedit,
	options::{NumberOptions, QuatDisplay, QuatOptions},
	ui::{ImmutableContext, InspectorUi, MutableContext, get_one_if_all_equal},
};
use bevy::{
	math::{DMat2, DMat3, DMat4, DVec2, DVec3, DVec4},
	prelude::*,
};
use derive_more::derive::Deref;
use derive_new::new;
use smallvec::SmallVec;
use std::any::Any;

macro_rules! vec_ui {
    ($ty:ty: $count:literal $($component:ident)*) => {
        fn ui<'c>(
            &self,
            ui: &mut egui::Ui,
            _: &dyn Any,
            _: egui::Id,
             env: &InspectorUi<'_, ImmutableContext<'c>>,
        ) {
            ui.scope(|ui| {
                ui.columns($count, |ui| match ui {
                    [$($component),*] => {
                        $(env.ui_for_reflect(&self.$component, $component);)*
                    }
                    _ => unreachable!(),
                });
            });
        }
    }
}

macro_rules! vec_ui_mut {
    ($ty:ty: $count:literal $($component:ident)*) => {
        fn ui_mut<'c>(
            &mut self,
            ui: &mut egui::Ui,
            options: &dyn Any,
            id: egui::Id,
            env: &mut InspectorUi<'_, MutableContext<'c>>,
        ) -> bool {
            let options = options
                .downcast_ref::<NumberOptions<$ty>>()
                .cloned()
                .unwrap_or_default();

            let mut changed = false;
            ui.scope(|ui| {
                ui.columns($count, |ui| match ui {
                    [$($component),*] => {
                        $(changed |= env.ui_for_reflect_mut_with_options(&mut self.$component, $component, id.with(stringify!($component)), &options.map(|vec| vec.$component));)*
                    }
                    _ => unreachable!(),
                });
            });
            changed
        }
    };
}

macro_rules! vec_ui_many {
  ($ty:ty>$elem_ty:ty: $count:literal $($component:ident)*) => {
    fn ui_mut_multiedit<'s, 'c>(
        ui: &mut egui::Ui,
        _: &dyn Any,
        id: egui::Id,
        _env: &mut InspectorUi<'_, MutableContext<'c>>,
        values: impl Iterator<Item = &'s mut Self>,
    ) -> bool {
      let mut changed = false;
      let mut values = values.collect::<SmallVec<[_; 8]>>();

      ui.scope(|ui| {
        ui.columns($count, |ui| match ui {
          [$($component),*] => {
            $(
              let same = get_one_if_all_equal(values.iter().map(|v| v.$component));

              let id = id.with(stringify!($component));
              changed |= crate::inspector::ui::change_slider($component, id, same, |change, overwrite| {
                for value in values.iter_mut() {
                  if false { value.$component = change };
                  if overwrite {
                    value.$component = change;
                  } else {
                    value.$component += change;
                  }

                }
              });
            )*
          }
          _ => unreachable!(),
        });
      });
      changed
    }
  };
}

macro_rules! mat_ui {
  ($ty:ty: $($component:ident)*) => {
    fn ui<'c>(
      &self,
      ui: &mut egui::Ui,
      _: &dyn Any,
      _: egui::Id,
      env: &InspectorUi<'_, ImmutableContext<'c>>,
    ) {
      ui.vertical(|ui| {
        $(env.ui_for_reflect(&self.$component, ui);)*
      });
    }
  };
}

macro_rules! mat_ui_mut {
  ($ty:ty: $($component:ident)*) => {
    fn ui_mut<'c>(
      &mut self,
      ui: &mut egui::Ui,
      _: &dyn Any,
      _: egui::Id,
      env: &mut InspectorUi<'_, MutableContext<'c>>,
    ) -> bool {
      let mut changed = false;
      ui.vertical(|ui| {
        $(changed |= env.ui_for_reflect_mut(&mut self.$component, ui);)*
      });
      changed
    }
  };
}

macro_rules! impl_vec_primitive {
	($ty:ty: $count:literal $($component:ident)*) => {
    impl InspectorPrimitive for $ty {
      vec_ui!($ty: $count $($component)*);

      vec_ui_mut!($ty: $count $($component)*);
    }
	};
}

macro_rules! impl_vec_primitive_many {
  ($ty:ty>$elem_ty:ty: $count:literal $($component:ident)*) => {
    impl InspectorPrimitiveMultiedit for $ty {
      vec_ui!($ty: $count $($component)*);

      vec_ui_mut!($ty: $count $($component)*);

      vec_ui_many!($ty>$elem_ty: $count $($component)*);
    }
  }
}

macro_rules! impl_mat_primitive {
  ($ty:ty: $($component:ident)*) => {
    impl InspectorPrimitive for $ty {
      mat_ui!($ty: $($component)*);

      mat_ui_mut!($ty: $($component)*);
    }
  };
}

impl_vec_primitive!(BVec2: 2 x y);
impl_vec_primitive!(BVec3: 3 x y z);
impl_vec_primitive!(BVec4: 4 x y z w);
impl_vec_primitive_many!(Vec2>f32: 2 x y);
impl_vec_primitive_many!(Vec3>f32: 3 x y z);
impl_vec_primitive_many!(Vec3A>f32: 3 x y z);
impl_vec_primitive_many!(Vec4>f32: 4 x y z w);
impl_vec_primitive_many!(UVec2>u32: 2 x y);
impl_vec_primitive_many!(UVec3>u32: 3 x y z);
impl_vec_primitive_many!(UVec4>u32: 4 x y z w);
impl_vec_primitive_many!(IVec2>i32: 2 x y);
impl_vec_primitive_many!(IVec3>i32: 3 x y z);
impl_vec_primitive_many!(IVec4>i32: 4 x y z w);
impl_vec_primitive_many!(DVec2>f64: 2 x y);
impl_vec_primitive_many!(DVec3>f64: 3 x y z);
impl_vec_primitive_many!(DVec4>f64: 4 x y z w);

impl_mat_primitive!(Mat2: x_axis y_axis);
impl_mat_primitive!(Mat3: x_axis y_axis z_axis);
impl_mat_primitive!(Mat3A: x_axis y_axis z_axis);
impl_mat_primitive!(Mat4: x_axis y_axis z_axis w_axis);
impl_mat_primitive!(DMat2: x_axis y_axis);
impl_mat_primitive!(DMat3: x_axis y_axis z_axis);
impl_mat_primitive!(DMat4: x_axis y_axis z_axis w_axis);

#[derive(Clone, Copy, Deref, DerefMut)]
struct Euler(Vec3);
#[derive(new, Clone, Copy)]
struct YawPitchRoll {
	yaw: f32,
	pitch: f32,
	roll: f32,
}
#[derive(new, Clone, Copy)]
struct AxisAngle {
	axis: Vec3,
	angle: f32,
}

trait RotationEdit: From<Quat> + Into<Quat> {
	fn ui<'c>(&self, ui: &mut egui::Ui, env: &InspectorUi<'_, ImmutableContext<'c>>);

	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool;
}

impl From<Quat> for Euler {
	fn from(value: Quat) -> Self {
		Self(value.to_euler(EulerRot::XYZ).into())
	}
}

impl From<Euler> for Quat {
	fn from(value: Euler) -> Self {
		Self::from_euler(EulerRot::XYZ, value.0.x, value.0.y, value.0.z)
	}
}

impl RotationEdit for Euler {
	fn ui<'c>(&self, ui: &mut egui::Ui, env: &InspectorUi<'_, ImmutableContext<'c>>) {
		env.ui_for_reflect(&self.0, ui);
	}

	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		env.ui_for_reflect_mut(&mut self.0, ui)
	}
}

impl From<Quat> for YawPitchRoll {
	fn from(value: Quat) -> Self {
		let (x, y, z) = value.to_euler(EulerRot::YXZ);
		Self::new(x, y, z)
	}
}

impl From<YawPitchRoll> for Quat {
	fn from(value: YawPitchRoll) -> Self {
		let YawPitchRoll { yaw, pitch, roll } = value;
		Self::from_euler(EulerRot::YXZ, yaw, pitch, roll)
	}
}

impl RotationEdit for YawPitchRoll {
	fn ui<'c>(&self, ui: &mut egui::Ui, _env: &InspectorUi<'_, ImmutableContext<'c>>) {
		let Self { yaw, pitch, roll } = self;

		ui.vertical(|ui| {
			egui::Grid::new("ypr grid").show(ui, |ui| {
				ui.label(format!("Yaw: {yaw}"));
				ui.end_row();

				ui.label(format!("Pitch: {pitch}"));
				ui.end_row();

				ui.label(format!("Roll: {roll}"));
				ui.end_row();
			});
		});
	}

	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		_env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		let Self { yaw, pitch, roll } = self;

		let mut changed = false;
		ui.vertical(|ui| {
			egui::Grid::new("ypr grid").show(ui, |ui| {
				ui.label("Yaw");
				changed |= ui.drag_angle(yaw).changed();
				ui.end_row();

				ui.label("Pitch");
				changed |= ui.drag_angle(pitch).changed();
				ui.end_row();

				ui.label("Roll");
				changed |= ui.drag_angle(roll).changed();
				ui.end_row();
			});
		});
		changed
	}
}

impl From<Quat> for AxisAngle {
	fn from(value: Quat) -> Self {
		let (axis, angle) = value.to_axis_angle();
		Self { axis, angle }
	}
}

impl From<AxisAngle> for Quat {
	fn from(value: AxisAngle) -> Self {
		let AxisAngle { axis, angle } = value;

		let Some(axis) = axis.try_normalize() else {
			return Quat::IDENTITY;
		};

		Self::from_axis_angle(axis, angle)
	}
}

impl RotationEdit for AxisAngle {
	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		let Self { axis, angle } = self;

		let mut changed = false;
		ui.vertical(|ui| {
			egui::Grid::new("axis-angle quat").show(ui, |ui| {
				ui.label("Axis");
				changed |= env.ui_for_reflect_mut(axis, ui);
				ui.end_row();
				ui.label("Angle");
				changed |= ui.drag_angle(angle).changed();
				ui.end_row();
			});
		});
		changed
	}

	fn ui<'c>(&self, ui: &mut egui::Ui, env: &InspectorUi<'_, ImmutableContext<'c>>) {
		let Self { axis, angle } = self;

		ui.vertical(|ui| {
			egui::Grid::new("axis-angle quat").show(ui, |ui| {
				ui.label("Axis");
				env.ui_for_reflect(axis, ui);
				ui.end_row();

				ui.label(format!("Angle: {angle}"));
				ui.end_row();
			});
		});
	}
}

fn quat_ui_kind_<'c, T: Send + Sync + 'static + Copy + RotationEdit>(
	val: &Quat,
	ui: &mut egui::Ui,
	env: &InspectorUi<'_, ImmutableContext<'c>>,
) {
	let id = ui.id();
	let mut intermediate = ui.memory_mut(|memory| {
		*memory
			.data
			.get_temp_mut_or_insert_with(id, || T::from(*val))
	});

	let externally_changed = !intermediate.into().abs_diff_eq(*val, f32::EPSILON);
	if externally_changed {
		intermediate = T::from(*val);
	}

	intermediate.ui(ui, env);

	if externally_changed {
		ui.memory_mut(|memory| memory.data.insert_temp(id, intermediate));
	}
}

fn quat_ui_kind_mut<'c, T: Send + Sync + 'static + Copy + RotationEdit>(
	val: &mut Quat,
	ui: &mut egui::Ui,
	env: &mut InspectorUi<'_, MutableContext<'c>>,
) -> bool {
	let id = ui.id();
	let mut intermediate = ui.memory_mut(|memory| {
		*memory
			.data
			.get_temp_mut_or_insert_with(id, || T::from(*val))
	});

	let externally_changed = !intermediate.into().abs_diff_eq(*val, f32::EPSILON);
	if externally_changed {
		intermediate = T::from(*val);
	}

	let changed = intermediate.ui_mut(ui, env);

	if changed || externally_changed {
		*val = intermediate.into();
		ui.memory_mut(|memory| memory.data.insert_temp(id, intermediate));
	}

	changed
}

impl InspectorPrimitive for Quat {
	fn ui<'c>(
		&self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		_: egui::Id,
		env: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		let options = options
			.downcast_ref::<QuatOptions>()
			.cloned()
			.unwrap_or_default();

		ui.vertical(|ui| match options.display {
			QuatDisplay::Raw => {
				let vec4 = Vec4::from(*self);
				env.ui_for_reflect(&vec4, ui);
			}
			QuatDisplay::Euler => quat_ui_kind_::<Euler>(self, ui, env),
			QuatDisplay::YawPitchRoll => quat_ui_kind_::<YawPitchRoll>(self, ui, env),
			QuatDisplay::AxisAngle => quat_ui_kind_::<AxisAngle>(self, ui, env),
		});
	}

	fn ui_mut<'c>(
		&mut self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		_: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		let options = options
			.downcast_ref::<QuatOptions>()
			.cloned()
			.unwrap_or_default();

		ui.vertical(|ui| match options.display {
			QuatDisplay::Raw => {
				let mut vec4 = Vec4::from(*self);
				let changed = env.ui_for_reflect_mut(&mut vec4, ui);
				if changed {
					*self = Quat::from_vec4(vec4).normalize();
				}
				changed
			}
			QuatDisplay::Euler => quat_ui_kind_mut::<Euler>(self, ui, env),
			QuatDisplay::YawPitchRoll => quat_ui_kind_mut::<YawPitchRoll>(self, ui, env),
			QuatDisplay::AxisAngle => quat_ui_kind_mut::<AxisAngle>(self, ui, env),
		})
		.inner
	}
}
