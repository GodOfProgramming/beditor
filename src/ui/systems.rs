use crate::{
	Layouts, Settings,
	misc::{MissingUi, UiState},
	ui::{EditorUiCamera, UiManager, UiPanels},
	util::storage::{CurrentLayoutSetting, SaveLayoutOnExitSetting},
};
use bevy::prelude::*;
use bevy_egui::PrimaryEguiContext;
use itertools::Itertools;
use persistent_id::PersistentId;

pub fn init_resources(world: &mut World) -> Result {
	world.spawn((Name::new("Editor UI Camera"), EditorUiCamera));
	world.spawn((Name::new("Editor Ui Panels"), UiPanels));
	world.resource_scope(|world, mut ui_manager: Mut<UiManager>| ui_manager.restore_or_init(world))
}

pub fn setup_ctx(
	mut q_ctx: Query<
		(
			&mut bevy_egui::EguiContext,
			&mut bevy_egui::EguiContextSettings,
		),
		Added<PrimaryEguiContext>,
	>,
) {
	let mut fonts = egui::FontDefinitions::default();
	egui_phosphor_icons::add_fonts(&mut fonts);

	for (mut ctx, mut settings) in &mut q_ctx {
		let ctx = ctx.get_mut();
		ctx.set_fonts(fonts.clone());
		settings.capture_pointer_input = false;
	}
}

pub fn reset_ui_state(mut q_ui_infos: Query<&mut UiState>) {
	q_ui_infos.par_iter_mut().for_each(|mut ui_info| {
		ui_info.rendered = false;
	});
}

pub fn render(world: &mut World) {
	world.resource_scope(|world, mut ui_manager: Mut<UiManager>| {
		ui_manager.render(world);
	});
}

pub fn dispatch_render_events(world: &mut World) {
	let mut q_entities = world.query::<(Entity, &UiState)>();
	let (rendered, unrendered): (Vec<Entity>, Vec<Entity>) =
		q_entities.iter(world).partition_map(|(entity, ui_info)| {
			if ui_info.rendered {
				itertools::Either::Left(entity)
			} else {
				itertools::Either::Right(entity)
			}
		});

	world.resource_scope(|world, ui_manager: Mut<UiManager>| {
		for entity in rendered {
			let Some(vtable) = ui_manager.vtable_of(entity, world) else {
				continue;
			};
			(vtable.when_rendered)(entity, world);
		}

		for entity in unrendered {
			let Some(vtable) = ui_manager.vtable_of(entity, world) else {
				continue;
			};
			(vtable.when_not_rendered)(entity, world);
		}
	});
}

pub fn on_app_exit(
	ui_manager: Res<UiManager>,
	q_uuids: Query<&PersistentId, Without<MissingUi>>,
	q_missing: Query<&MissingUi>,
	mut params: ParamSet<(Settings, Layouts)>,
) -> Result {
	let current_layout = {
		let mut settings = params.p0();

		let save_on_exit = settings.get_or_default::<SaveLayoutOnExitSetting>();
		if save_on_exit {
			let name = match settings.get::<CurrentLayoutSetting>().ok() {
				Some(opt) => opt,
				None => {
					let default_layout = String::from("default");
					settings.set::<CurrentLayoutSetting>(&default_layout)?;
					default_layout
				}
			};

			Some(name)
		} else {
			None
		}
	};

	if let Some(name) = current_layout {
		let mut layouts = params.p1();
		let new_state = ui_manager.save_current_layout(&q_uuids, &q_missing);
		layouts.save_layout(name, new_state)?;
	}

	Ok(())
}
