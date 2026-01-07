use bevy::reflect::TypeRegistry;
use egui::FontId;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use parking_lot::Mutex;
use std::{any::TypeId, sync::LazyLock};

pub fn show_docs(
	type_registry: &TypeRegistry,
	type_id: TypeId,
	response: egui::Response,
) -> Option<egui::Response> {
	type_registry
		.get_type_info(type_id)
		.and_then(|info| info.docs())
		.map(|docs| {
			response.on_hover_ui(|ui| {
				show_markdown(ui, CommonMarkViewer::new(), docs);
			})
		})
}

pub fn show_markdown(
	ui: &mut egui::Ui,
	viewer: CommonMarkViewer,
	text: &str,
) -> egui::InnerResponse<()> {
	static CACHE: LazyLock<Mutex<CommonMarkCache>> = LazyLock::new(Default::default);
	let mut cache = CACHE.lock();
	viewer.show(ui, &mut cache, text)
}

pub fn layout_job(text: &[(FontId, &str)]) -> egui::epaint::text::LayoutJob {
	let mut job = egui::epaint::text::LayoutJob::default();
	for (font_id, text) in text {
		job.append(
			text,
			0.0,
			egui::TextFormat {
				font_id: font_id.clone(),
				..Default::default()
			},
		);
	}
	job
}
