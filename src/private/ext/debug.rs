use crate::{
	EditorExtension, EditorUiWorld,
	private::{EditorInternalSingle, ui::EditorEguiContext, util::extensions::WorldMutExtensions},
};
use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::TypeIdMap};
use bevy_egui::EguiContext;
use common::match_else;
use derive_more::derive::DerefMut;
use derive_new::new;
use notify::Notification;
use std::{any::TypeId, num::NonZeroUsize, sync::Arc};

#[derive(Default)]
pub struct DebugUiExtension;

impl EditorExtension for DebugUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<DebugUi>();
	}

	fn build_app(&self, app: &mut App) {
		app
			.init_resource::<ScheduleNames>()
			.init_resource::<ScheduleViews>()
			.init_resource::<SvgOptions>()
			.add_message::<LoadScheduleTexture>()
			.add_systems(First, LoadScheduleTexture::handle);
	}

	fn finalize(&self, app: &mut App) {
		app
			.world_mut()
			.resources_scope::<(Schedules, ScheduleNames)>(
				|world, (mut schedules, mut schedule_names)| {
					let settings = bevy_mod_debugdump::schedule_graph::settings::Settings::default();

					let ignored_ambiguities = schedules.ignored_scheduling_ambiguities.clone();
					for (label, schedule) in schedules.iter_mut() {
						let label_name = format!("{label:?}");
						schedule.graph_mut().initialize(world);
						let _ = schedule
							.graph_mut()
							.build_schedule(world, &ignored_ambiguities);

						let graph =
							bevy_mod_debugdump::schedule_graph::schedule_graph_dot(schedule, world, &settings);

						schedule_names.insert(label.type_id(), label_name.clone());

						world.write_message(LoadScheduleTexture::new(label.type_id(), label_name, graph));
					}
				},
			);
	}
}

#[derive(Resource, Deref)]
pub struct SvgOptions {
	fontdb: Arc<resvg::usvg::fontdb::Database>,
}

impl Default for SvgOptions {
	fn default() -> Self {
		let mut fontdb = resvg::usvg::fontdb::Database::new();
		fontdb.load_system_fonts();

		if cfg!(not(windows)) {
			fontdb.set_serif_family("DejaVu Serif");
			fontdb.set_sans_serif_family("DejaVu Sans");
			fontdb.set_monospace_family("DejaVu Sans Mono");
		}

		Self {
			fontdb: Arc::new(fontdb),
		}
	}
}

#[derive(Resource, Deref, DerefMut)]
pub struct ScheduleNames(TypeIdMap<String>);

impl Default for ScheduleNames {
	fn default() -> Self {
		Self(default())
			.with::<Startup>("Startup")
			.with::<Update>("Update")
	}
}

impl ScheduleNames {
	fn with<S: ScheduleLabel>(mut self, name: impl Into<String>) -> Self {
		self.register::<S>(name);
		self
	}

	fn register<S: ScheduleLabel>(&mut self, name: impl Into<String>) -> &mut Self {
		self.insert(TypeId::of::<S>(), name.into());
		self
	}
}

#[derive(Resource, Default, Deref, DerefMut)]
struct ScheduleViews(TypeIdMap<Option<ScheduleView>>);

struct ScheduleView {
	cols: NonZeroUsize,
	rows: NonZeroUsize,
	textures: Vec<egui::TextureHandle>,
}

#[derive(Component, Default)]
pub struct DebugUi {
	selected_label: Option<TypeId>,
}

impl EditorUiWorld for DebugUi {
	type MarkerComponent = Self;
	const NAME: &str = "DebugUi";

	const ID: uuid::Uuid = uuid::uuid!("392393ce-1738-400f-988e-f5cec604eae9");

	fn spawn(_entity: Entity, _world: &mut World) -> Result<Self> {
		Ok(default())
	}

	fn ui(entity: Entity, ui: &mut egui::Ui, world: &mut World) -> Result<()> {
		world.resources_scope::<(ScheduleNames, ScheduleViews)>(|world, (names, views)| {
			let mut q_this = world.query::<&mut DebugUi>();
			let Ok(mut this) = q_this.get_mut(world, entity) else {
				ui.label("Debug ui has no component of itself (logic error)");
				return;
			};

			this.selected_label_ui(ui, &names);

			let view = this.selected_label.and_then(|label| views.get(&label));

			let Some(maybe_image) = view else {
				ui.label("No schedule for selected label");
				return;
			};

			let Some(image) = maybe_image else {
				ui.label("Image processing...");
				return;
			};

			egui::ScrollArea::both()
				.scroll_source(egui::scroll_area::ScrollSource::ALL)
				.auto_shrink([false; 2])
				.show(ui, |ui| {
					ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
					egui::Grid::new(ui.id().with("schedule-grid"))
						.spacing([0.0, 0.0])
						.show(ui, |ui| {
							for r in 0..image.rows.get() {
								for c in 0..image.cols.get() {
									ui.image(&image.textures[r * image.cols.get() + c]);
								}
								ui.end_row();
							}
						});
				});
		});

		Ok(())
	}
}

impl DebugUi {
	fn selected_label_ui(&mut self, ui: &mut egui::Ui, names: &ScheduleNames) {
		let display = self
			.selected_label
			.as_ref()
			.and_then(|l| names.get(l))
			.map(|n| n.as_str())
			.unwrap_or_default();

		egui::ComboBox::from_label("Schedule Label")
			.selected_text(display)
			.show_ui(ui, |ui| {
				for (&tid, name) in names.iter() {
					ui.selectable_value(&mut self.selected_label, Some(tid), name);
				}
			});
	}
}

#[derive(new, Message)]
struct LoadScheduleTexture {
	type_id: TypeId,
	name: String,
	dot_graph: String,
}

impl LoadScheduleTexture {
	fn handle(
		mut commands: Commands,
		mut messages: MessageReader<Self>,
		mut context: EditorInternalSingle<&mut EguiContext, With<EditorEguiContext>>,
		mut views: ResMut<ScheduleViews>,
		svg_opts: Res<SvgOptions>,
	) {
		let ctx = context.get_mut();
		let max_tex_side = ctx.input(|i| i.max_texture_side);
		for msg in messages.read() {
			info!("Generating schedule graph {}", msg.name);
			let mut parser = layout::gv::DotParser::new(&msg.dot_graph);

			let graph = match_else!(parser.process(); else err => {
				commands.trigger(Notification::error(format!("failed to parse dot graph for {}", msg.name)).with_context(serde_json::json!({
					"err": err,
				})));
				continue;
			});

			let mut gb = layout::gv::GraphBuilder::new();

			gb.visit_graph(&graph);

			let mut writer = layout::backends::svg::SVGWriter::new();

			let mut graph = gb.get();

			graph.do_it(false, false, false, &mut writer);

			let svg = Vec::from_iter(writer.finalize().bytes());

			let options = resvg::usvg::Options {
				fontdb: Arc::clone(&svg_opts.fontdb),
				..default()
			};
			let tree = match_else!(resvg::usvg::Tree::from_data(&svg, &options); else err => {
				commands.trigger(Notification::error(format!("failed to create svg tree for {}", msg.name)).with_context(serde_json::json!({
					"err": err.to_string(),
				})));
				continue;
			});

			let size = tree.size();
			let width = size.width();
			let height = size.height();

			let Some(mut pixmap) = resvg::tiny_skia::Pixmap::new(width as u32, height as u32) else {
				commands.trigger(Notification::error(format!(
					"failed to create pixmap for {}",
					msg.name
				)));
				continue;
			};

			resvg::render(
				&tree,
				resvg::tiny_skia::Transform::default(),
				&mut pixmap.as_mut(),
			);

			let (cols, rows, chunks) = match_else!(to_chunks(pixmap, max_tex_side); else err => {
				commands.trigger(Notification::error(format!("failed to create textures for schedule {}", msg.name)).with_context(serde_json::json!({
					"err": err.to_string(),
				})));
				continue;
			});

			views.insert(
				msg.type_id,
				Some(ScheduleView {
					cols: match_else!(NonZeroUsize::new(cols).ok_or("cols was not > 0"); else err => {
            commands.trigger(Notification::error(format!("failed to create textures for schedule {}", msg.name)).with_context(serde_json::json!({
              "err": err.to_string(),
            })));
            continue;
          }),
					rows: match_else!(NonZeroUsize::new(rows).ok_or("rows was not > 0"); else err => {
            commands.trigger(Notification::error(format!("failed to create textures for schedule {}", msg.name)).with_context(serde_json::json!({
              "err": err.to_string(),
            })));
            continue;
          }),
					textures: chunks
						.into_iter()
						.enumerate()
						.map(|(i, chunk)| {
							let color_image = egui::ColorImage::from_rgba_premultiplied(
								[chunk.width() as usize, chunk.height() as usize],
								chunk.data(),
							);

							ctx.load_texture(
								format!("beditor-bevy-schedule-{}-{i}", msg.name),
								color_image,
								egui::TextureOptions::LINEAR,
							)
						})
						.collect(),
				}),
			);
		}
	}
}

fn to_chunks(
	pixmap: resvg::tiny_skia::Pixmap,
	max_len: usize,
) -> Result<(usize, usize, Vec<resvg::tiny_skia::Pixmap>)> {
	let width = pixmap.width() as usize;
	let height = pixmap.height() as usize;
	let cols = width / max_len;
	let rows = height / max_len;

	let mut chunks = Vec::with_capacity(rows * cols);

	for r in 0..=rows {
		for c in 0..=cols {
			let xoffset = max_len * c;
			let yoffset = max_len * r;

			let xlen = max_len.min(width - xoffset);
			let ylen = max_len.min(height - yoffset);

			let rect = resvg::tiny_skia::IntRect::from_xywh(
				xoffset.try_into()?,
				yoffset.try_into()?,
				xlen.try_into()?,
				ylen.try_into()?,
			)
			.ok_or("Failed to create chunk rect")?;

			let chunk = pixmap
				.clone_rect(rect)
				.ok_or("Failed to get subrect from pixmap")?;

			chunks.push(chunk);
		}
	}

	if chunks.is_empty() {
		Err("size of schedule graph was zero")?;
	}

	Ok((cols + 1, rows + 1, chunks))
}
