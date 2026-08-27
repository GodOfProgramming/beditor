use crate::{
	EditorExtension, EditorUiWorld,
	private::{EditorInternalSingle, ui::EditorEguiContext, util::extensions::WorldMutExtensions},
};
use bevy::{
	ecs::schedule::ScheduleLabel, platform::collections::HashMap, prelude::*,
	time::common_conditions::on_timer, utils::TypeIdMap,
};
use bevy_egui::EguiContext;
use common::match_else;
use derive_more::derive::DerefMut;
use derive_new::new;
use notify::Notification;
use std::{
	any::{Any, TypeId},
	sync::Arc,
	time::Duration,
};

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
			.add_message::<LoadScheduleTexture>()
			.add_systems(First, LoadScheduleTexture::handle);
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
pub struct ScheduleViews(TypeIdMap<Option<egui::TextureHandle>>);

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
		world.resources_scope::<(Schedules, ScheduleNames, ScheduleViews)>(
			|world, (schedules, names, mut views)| {
				let mut q_this = world.query::<&mut DebugUi>();
				let Ok(mut this) = q_this.get_mut(world, entity) else {
					ui.label("Debug ui has no component of itself (logic error)");
					return;
				};

				this.selected_label_ui(ui, &names);

				let view = this.selected_label.and_then(|label| views.get(&label));

				let Some(maybe_image) = view else {
					if let Some(tid) = this.selected_label
						&& let Some(schedule) = schedules.iter().find(|s| tid == s.0.type_id())
						&& let Some(name) = names.get(&tid)
					{
						let dot = bevy_mod_debugdump::schedule_graph::schedule_graph_dot(
							schedule.1,
							world,
							&bevy_mod_debugdump::schedule_graph::Settings::default(),
						);
						if world
							.write_message(LoadScheduleTexture::new(tid, name.to_string(), dot))
							.is_some()
						{
							views.insert(tid, None);
						} else {
							world.trigger(Notification::error(
								"failed to start schedule graph render job",
							));
						}
					}

					return;
				};

				let Some(image) = maybe_image else {
					ui.label("Image processing...");
					return;
				};

				egui::ScrollArea::both()
					.auto_shrink([false; 2])
					.show(ui, |ui| {
						ui.image(image);
					});
			},
		);

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
	) {
		let ctx = context.get_mut();
		for msg in messages.read() {
			commands.trigger(Notification::info("Generating schedule graph..."));
			let graph = match_else!(graphviz_rust::parse(&msg.dot_graph); else err => {
				commands.trigger(Notification::error(format!("failed to parse dot graph for {:?}", msg.type_id)).with_context(serde_json::json!({
					"err": err,
				})));
				return;
			});

			let svg = match_else!(graphviz_rust::exec(
				graph,
				&mut graphviz_rust::printer::PrinterContext::default(),
				vec![graphviz_rust::cmd::CommandArg::Format(
					graphviz_rust::cmd::Format::Svg,
				)],
			); else err => {
				commands.trigger(Notification::error(format!("failed to render graph for {:?}", msg.type_id)).with_context(serde_json::json!({
					"err": err.to_string(),
				})));
				return;
			});

			let opts = resvg::usvg::Options::default();
			let Ok(tree) = resvg::usvg::Tree::from_data(&svg, &opts) else {
				return;
			};

			let size = tree.size();
			let width = size.width();
			let height = size.height();

			let Some(mut pixmap) = resvg::tiny_skia::Pixmap::new(width as u32, height as u32) else {
				commands.trigger(Notification::error(format!(
					"failed to create pixmap for {:?}",
					msg.type_id
				)));
				return;
			};

			resvg::render(
				&tree,
				resvg::tiny_skia::Transform::default(),
				&mut pixmap.as_mut(),
			);

			let color_image =
				egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], pixmap.data());

			let texture = ctx.load_texture(msg.name.clone(), color_image, egui::TextureOptions::LINEAR);

			views.insert(msg.type_id, Some(texture));
		}
	}
}
