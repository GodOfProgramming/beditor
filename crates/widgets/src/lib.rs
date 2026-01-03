use std::{collections::HashSet, hash::Hash};

use egui::scroll_area::ScrollAreaOutput;
use itertools::Itertools;

pub struct Dialog {
	id: egui::Id,
	title: egui::WidgetText,
	pub open: bool,
}

impl Dialog {
	pub fn new(id: egui::Id, title: impl Into<egui::WidgetText>) -> Self {
		Self {
			id,
			title: title.into(),
			open: false,
		}
	}

	pub fn set_title(&mut self, title: impl Into<egui::WidgetText>) {
		self.title = title.into();
	}

	/// See [`egui::Window::show`]
	pub fn show<R>(
		&mut self,
		ctx: &egui::Context,
		contents: impl FnOnce(&mut egui::Ui, &mut bool) -> R,
	) -> Option<egui::ModalResponse<R>> {
		if self.open {
			let response = egui::Modal::new(self.id).show(ctx, |ui| (contents)(ui, &mut self.open));
			Some(response)
		} else {
			None
		}
	}
}

pub struct BorderedBox {
	pos: egui::Pos2,
	size: egui::Vec2,
	thickness: f32,
}

impl BorderedBox {
	pub fn new(pos: impl Into<egui::Pos2>, size: impl Into<egui::Vec2>) -> Self {
		Self {
			pos: pos.into(),
			size: size.into(),
			thickness: 1.0,
		}
	}

	pub fn with_thickness(mut self, thickness: f32) -> Self {
		self.thickness = thickness;
		self
	}

	pub fn show<R>(
		&self,
		ui: &mut egui::Ui,
		contents: impl FnOnce(&mut egui::Ui) -> R,
	) -> egui::InnerResponse<R> {
		Self::ui(ui, self.pos, self.size, self.thickness, contents)
	}

	fn ui<R>(
		ui: &mut egui::Ui,
		pos: egui::Pos2,
		size: egui::Vec2,
		thickness: f32,
		contents: impl FnOnce(&mut egui::Ui) -> R,
	) -> egui::InnerResponse<R> {
		let rect = egui::Rect::from_min_size(pos, size);
		let stroke = egui::Stroke::new(thickness, ui.style().visuals.widgets.active.fg_stroke.color);

		egui::Frame::default().stroke(stroke).show(ui, |ui| {
			ui.set_min_size(rect.size());
			ui.set_max_size(rect.size());
			(contents)(ui)
		})
	}
}

pub struct Card {
	size: egui::Vec2,
	label: Option<egui::WidgetText>,
	border_thickness: Option<f32>,
	content_size: Option<f32>,
}

impl Card {
	pub fn new(size: impl Into<egui::Vec2>) -> Self {
		Self {
			size: size.into(),
			label: None,
			border_thickness: None,
			content_size: None,
		}
	}

	pub fn with_label(mut self, text: impl Into<egui::WidgetText>) -> Self {
		self.label = Some(text.into());
		self
	}

	pub fn show<R>(
		&self,
		ui: &mut egui::Ui,
		add_contents: impl FnOnce(&mut egui::Ui) -> R,
	) -> egui::InnerResponse<R> {
		ui.vertical_centered(|ui| {
			ui.set_width(self.size.x);
			ui.set_height(self.size.y);

			let border_thickness = self.border_thickness.unwrap_or_else(|| self.size.x / 25.0);
			let cell_content_size = self.content_size.unwrap_or(self.size.x - border_thickness);

			let inner = BorderedBox::new((0.0, 0.0), (cell_content_size, cell_content_size))
				.with_thickness(border_thickness)
				.show(ui, |ui| ui.centered_and_justified(add_contents));

			if let Some(text) = &self.label {
				ui.label(text.clone());
			}

			inner.inner.inner
		})
	}
}

pub fn horizontal_list<I, T>(
	ui: &mut egui::Ui,
	columns: impl Into<usize>,
	iterable: I,
	mut add_content: impl FnMut(&mut egui::Ui, usize, T),
) where
	I: IntoIterator<Item = T> + Sized,
{
	let mut index = 0;
	let columns = columns.into();
	let chunks = iterable.into_iter().chunks(columns);
	for chunk in &chunks {
		ui.columns(columns, |uis| {
			for (ui, item) in uis.iter_mut().zip(chunk) {
				add_content(ui, index, item);
				index += 1;
			}
		});
	}
}

pub struct SelectableList<S> {
	selector: S,
}

impl<S> Default for SelectableList<S>
where
	S: Selector,
{
	fn default() -> Self {
		Self {
			selector: Default::default(),
		}
	}
}

impl<S> SelectableList<S>
where
	S: Selector,
	S::Item: Clone + AsRef<str>,
{
	pub fn selected(&self) -> &S::Selected {
		self.selector.selected()
	}

	pub fn ui(&mut self, ui: &mut egui::Ui, items: &[S::Item]) -> Option<egui::InnerResponse<usize>> {
		let text_style = egui::TextStyle::Body;
		let row_height = ui.text_style_height(&text_style);

		egui::ScrollArea::both()
			.auto_shrink([false, false])
			.show_rows(ui, row_height, items.len(), |ui, range| {
				let mut inner_response = None;

				let start = range.start;
				for (i, item) in items[range].iter().enumerate() {
					let response = ui.add(
						egui::Button::selectable(self.selector.is_selected(item), item.as_ref()).truncate(),
					);

					if response.clicked() {
						self.selector.on_select(item.clone());
						inner_response = Some(egui::InnerResponse::new(start + i, response));
					}
				}

				inner_response
			})
			.inner
	}
}

pub trait Selector: Default {
	type Item;
	type Selected;

	fn selected(&self) -> &Self::Selected;

	fn is_selected(&self, other: &Self::Item) -> bool;

	fn on_select(&mut self, other: Self::Item);
}

pub struct SingleSelect<T> {
	selected: Option<T>,
}

impl<T> Default for SingleSelect<T> {
	fn default() -> Self {
		Self {
			selected: Default::default(),
		}
	}
}

impl<T> Selector for SingleSelect<T>
where
	T: PartialEq,
{
	type Item = T;
	type Selected = Option<T>;

	fn selected(&self) -> &Self::Selected {
		&self.selected
	}

	fn is_selected(&self, other: &Self::Item) -> bool {
		self.selected.as_ref() == Some(other)
	}

	fn on_select(&mut self, other: Self::Item) {
		if self.is_selected(&other) {
			self.selected = None;
		} else {
			self.selected = Some(other);
		}
	}
}

pub struct MultiSelect<T> {
	selected: HashSet<T>,
}

impl<T> Default for MultiSelect<T> {
	fn default() -> Self {
		Self {
			selected: Default::default(),
		}
	}
}

impl<T> Selector for MultiSelect<T>
where
	T: Eq + Hash,
{
	type Item = T;
	type Selected = HashSet<T>;

	fn selected(&self) -> &Self::Selected {
		&self.selected
	}

	fn is_selected(&self, other: &Self::Item) -> bool {
		self.selected.contains(other)
	}

	fn on_select(&mut self, other: Self::Item) {
		if self.is_selected(&other) {
			self.selected.remove(&other);
		} else {
			self.selected.insert(other);
		}
	}
}

pub struct DualVScrollArea {
	id: egui::Id,
	split_at: f32,
}

impl DualVScrollArea {
	pub fn new(id: egui::Id, split_at: f32) -> Self {
		Self { id, split_at }
	}

	pub fn show<L, R>(
		&self,
		ui: &mut egui::Ui,
		left: impl FnOnce(&mut egui::Ui) -> L,
		right: impl FnOnce(&mut egui::Ui) -> R,
	) -> egui::InnerResponse<(ScrollAreaOutput<L>, ScrollAreaOutput<R>)> {
		ui.allocate_ui_with_layout(
			ui.available_size(),
			egui::Layout::left_to_right(egui::Align::Center),
			|ui| {
				let left = egui::ScrollArea::vertical()
					.max_width(self.split_at)
					.auto_shrink([false; 2])
					.id_salt(self.id.with("dual-scroll-left"))
					.show(ui, |ui| ui.vertical(|ui| (left)(ui)).inner);

				ui.separator();

				let right = egui::ScrollArea::vertical()
					.auto_shrink([true, false])
					.id_salt(self.id.with("dual-scroll-right"))
					.show(ui, |ui| ui.vertical(|ui| (right)(ui)).inner);

				(left, right)
			},
		)
	}
}
