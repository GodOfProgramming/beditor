use std::{
	borrow::{self, BorrowMut},
	ops::{Div, Mul},
};

pub trait ContextExtensions: borrow::Borrow<egui::Context> {
	fn to_points<T>(&self, pixels: T) -> T
	where
		T: Div<f32, Output = T>,
	{
		let ppp = self.borrow().pixels_per_point();
		pixels / ppp
	}

	fn to_points_many<T, const N: usize>(&self, pixels: [T; N]) -> [T; N]
	where
		T: Div<f32, Output = T>,
	{
		let ppp = self.borrow().pixels_per_point();
		pixels.map(|p| p / ppp)
	}

	fn to_pixels<T>(&self, points: T) -> T
	where
		T: Mul<f32, Output = T>,
	{
		let ppp = self.borrow().pixels_per_point();
		points * ppp
	}

	fn to_pixels_many<T, const N: usize>(&self, points: [T; N]) -> [T; N]
	where
		T: Mul<f32, Output = T>,
	{
		let ppp = self.borrow().pixels_per_point();
		points.map(|p| p * ppp)
	}
}

impl<T> ContextExtensions for T where T: borrow::Borrow<egui::Context> {}

pub trait CollapsingResponseExtensions<T>: BorrowMut<Option<egui::CollapsingResponse<T>>> {
	fn maybe_take(&mut self, other: Option<egui::CollapsingResponse<T>>) {
		match (self.borrow(), other) {
			(None, that @ Some(_)) => *self.borrow_mut() = that,
			(Some(this), that @ Some(_)) => {
				let header_context_menu_opened =
					ResponseConditions::from(&this.header_response).context_menu_opened;

				let body_context_menu_opened = this
					.body_response
					.as_ref()
					.map(|r| ResponseConditions::from(r).context_menu_opened)
					.unwrap_or(false);

				if !header_context_menu_opened && !body_context_menu_opened {
					*self.borrow_mut() = that;
				}
			}
			_ => (),
		}
	}
}

impl<T, U> CollapsingResponseExtensions<U> for T where
	T: BorrowMut<Option<egui::CollapsingResponse<U>>>
{
}

pub struct ResponseConditions {
	context_menu_opened: bool,
	secondary_clicked: bool,
	hovered: bool,
	should_show_tooltip: bool,
}

impl ResponseConditions {
	pub fn any(&self) -> bool {
		self.context_menu_opened | self.secondary_clicked | self.hovered | self.should_show_tooltip
	}
}

impl From<&egui::Response> for ResponseConditions {
	fn from(response: &egui::Response) -> Self {
		Self {
			context_menu_opened: response.context_menu_opened(),
			secondary_clicked: response.secondary_clicked(),
			hovered: response.hovered(),
			should_show_tooltip: egui::Tooltip::should_show_tooltip(response, true),
		}
	}
}
