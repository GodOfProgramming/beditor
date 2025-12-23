use crate::{TypeGroups, TypeList, util::AppExtensions};
use bevy::prelude::*;
use std::any::TypeId;

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
	fn build(&self, app: &mut App) {
		app.register_types::<(
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
		)>();
	}
}
