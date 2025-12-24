use std::hash::Hash;

use egui::{
	WidgetText,
	text::{CCursor, CCursorRange},
};
use itertools::Itertools;

#[derive(Default)]
pub struct Dialog {
	title: egui::WidgetText,
	pub open: bool,
}

impl Dialog {
	pub fn new(title: impl Into<WidgetText>) -> Self {
		Self {
			title: title.into(),
			open: false,
		}
	}

	pub fn set_title(&mut self, title: impl Into<WidgetText>) {
		self.title = title.into();
	}

	/// See [`egui::Window::show`]
	pub fn show<R>(
		&mut self,
		ctx: &egui::Context,
		contents: impl FnOnce(&mut egui::Ui, &mut bool) -> R,
	) -> Option<egui::InnerResponse<Option<R>>> {
		let mut open = self.open;
		let out = egui::Window::new(self.title.clone())
			.open(&mut self.open)
			.anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
			.title_bar(true)
			.resizable(false)
			.movable(false)
			.collapsible(false)
			.show(ctx, |ui| (contents)(ui, &mut open));
		self.open &= open;
		out
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

pub struct SelectableList<T>
where
	T: Eq + Clone + AsRef<str>,
{
	selected: Option<T>,
}

impl<T> Default for SelectableList<T>
where
	T: Eq + Clone + AsRef<str>,
{
	fn default() -> Self {
		Self { selected: None }
	}
}

impl<T> SelectableList<T>
where
	T: Eq + Clone + AsRef<str>,
{
	pub fn selected(&self) -> Option<&T> {
		self.selected.as_ref()
	}

	pub fn ui(&mut self, ui: &mut egui::Ui, items: &[T]) -> Option<egui::InnerResponse<usize>> {
		let text_style = egui::TextStyle::Body;
		let row_height = ui.text_style_height(&text_style);

		egui::ScrollArea::both()
			.auto_shrink([false, false])
			.show_rows(ui, row_height, items.len(), |ui, range| {
				let mut value = None;

				let start = range.start;
				for (i, item) in items[range].iter().enumerate() {
					let response = ui.add(
						egui::Button::selectable(self.selected.as_ref() == Some(item), item.as_ref())
							.truncate(),
					);
					if response.clicked() {
						self.selected = Some(item.clone());
						value = Some(egui::InnerResponse::new(start + i, response));
					}
				}

				value
			})
			.inner
	}
}

pub struct Dir;

impl Dir {
	pub fn ui(ui: &mut egui::Ui, id: egui::Id) -> egui::Response {
		ui.label(egui_phosphor_icons::icons::FOLDER.regular());
		ui.interact(ui.min_rect(), id, egui::Sense::click())
	}
}

pub struct DropDownBox<
	'a,
	F: FnMut(&mut egui::Ui, &str) -> egui::Response,
	V: AsRef<str>,
	I: Iterator<Item = V>,
> {
	buf: &'a mut String,
	popup_id: egui::Id,
	display: F,
	it: I,
	hint_text: WidgetText,
	filter_by_input: bool,
	select_on_focus: bool,
	desired_width: Option<f32>,
}

impl<'a, F: FnMut(&mut egui::Ui, &str) -> egui::Response, V: AsRef<str>, I: Iterator<Item = V>>
	DropDownBox<'a, F, V, I>
{
	/// Creates new dropdown box.
	pub fn from_iter(
		it: impl IntoIterator<IntoIter = I>,
		id_source: impl Hash,
		buf: &'a mut String,
		display: F,
	) -> Self {
		Self {
			popup_id: egui::Id::new(id_source),
			it: it.into_iter(),
			display,
			buf,
			hint_text: WidgetText::default(),
			filter_by_input: true,
			select_on_focus: false,
			desired_width: None,
		}
	}

	/// Add a hint text to the Text Edit
	pub fn hint_text(mut self, hint_text: impl Into<WidgetText>) -> Self {
		self.hint_text = hint_text.into();
		self
	}

	/// Determine whether to filter box items based on what is in the Text Edit already
	pub fn filter_by_input(mut self, filter_by_input: bool) -> Self {
		self.filter_by_input = filter_by_input;
		self
	}

	/// Determine whether to select the text when the Text Edit gains focus
	pub fn select_on_focus(mut self, select_on_focus: bool) -> Self {
		self.select_on_focus = select_on_focus;
		self
	}

	/// Passes through the desired width value to the underlying Text Edit
	pub fn desired_width(mut self, desired_width: f32) -> Self {
		self.desired_width = desired_width.into();
		self
	}
}

impl<'a, F: FnMut(&mut egui::Ui, &str) -> egui::Response, V: AsRef<str>, I: Iterator<Item = V>>
	egui::Widget for DropDownBox<'a, F, V, I>
{
	fn ui(self, ui: &mut egui::Ui) -> egui::Response {
		let Self {
			popup_id,
			buf,
			it,
			mut display,
			hint_text,
			filter_by_input,
			select_on_focus,
			desired_width,
		} = self;

		let mut edit = egui::TextEdit::singleline(buf).hint_text(hint_text);
		if let Some(dw) = desired_width {
			edit = edit.desired_width(dw);
		}
		let mut edit_output = edit.show(ui);
		let mut r = edit_output.response;
		if r.gained_focus() {
			if select_on_focus {
				edit_output
					.state
					.cursor
					.set_char_range(Some(CCursorRange::two(
						CCursor::new(0),
						CCursor::new(buf.len()),
					)));
				edit_output.state.store(ui.ctx(), r.id);
			}
			egui::Popup::open_id(ui.ctx(), popup_id);
		}

		let mut changed = false;
		egui::Popup::menu(&r)
			.id(popup_id)
			.close_behavior(egui::PopupCloseBehavior::CloseOnClick)
			.show(|ui| {
				egui::ScrollArea::vertical().show(ui, |ui| {
					let mut any_visible = false;
					let mut counter = 0;
					for var in it {
						counter += 1;
						let text = var.as_ref();
						if filter_by_input
							&& !buf.is_empty()
							&& !text.to_lowercase().contains(&buf.to_lowercase())
						{
							continue;
						}
						any_visible = true;

						if display(ui, text).clicked() {
							*buf = text.to_owned();
							changed = true;
						}
					}
					if !any_visible {
						if buf.is_empty() {
							ui.label("No items found");
						} else {
							ui.label(format!("No items out of {} match the filter", counter));
						}
					}
				});
			});

		if changed {
			egui::Popup::close_id(ui.ctx(), popup_id);
			r.mark_changed();
		}

		r
	}
}
