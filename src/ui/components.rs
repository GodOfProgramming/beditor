use derive_new::new;
use itertools::Itertools;

#[derive(new)]
pub struct Dialog<T>
where
  T: Into<egui::WidgetText>,
{
  title: T,
}

impl<T> Dialog<T>
where
  T: Into<egui::WidgetText>,
{
  pub fn open(
    self,
    ctx: &egui::Context,
    mut opened: bool,
    contents: impl FnOnce(&mut egui::Ui),
  ) -> bool {
    if opened {
      egui::Window::new(self.title)
        .open(&mut opened)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .title_bar(true)
        .resizable(false)
        .movable(false)
        .collapsible(false)
        .show(ctx, contents);
    }
    opened
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

  pub fn with_border_thickness(mut self, thickness: impl Into<f32>) -> Self {
    self.border_thickness = Some(thickness.into());
    self
  }

  pub fn with_content_size(mut self, size: impl Into<f32>) -> Self {
    self.content_size = Some(size.into());
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
  columns: usize,
  iterable: I,
  mut add_content: impl FnMut(&mut egui::Ui, usize, T),
) where
  I: IntoIterator<Item = T> + Sized,
{
  let mut index = 0;
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
