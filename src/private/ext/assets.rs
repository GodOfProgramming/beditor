use crate::{EditorExtension, private::ui::InspectorSelection, ui::EditorUi};
use bevy::{asset::ReflectAsset, ecs::system::SystemParam, prelude::*};
use uuid::uuid;

#[derive(Default)]
pub struct AssetsUiExtension;

impl EditorExtension for AssetsUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<AssetsUi>();
	}
}

#[derive(Default, Component, Reflect)]
pub struct AssetsUi;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	set: ParamSet<'w, 's, (&'w World, Resources<'w>)>,
	filter: Local<'s, String>,
}

#[derive(SystemParam)]
struct Resources<'w> {
	app_type_registry: Res<'w, AppTypeRegistry>,
	inspector_selection: ResMut<'w, InspectorSelection>,
}

impl EditorUi for AssetsUi {
	const NAME: &str = stringify!(Assets);
	const ID: uuid::Uuid = uuid!("4bfee754-f9bc-4695-b215-2a88d9377dfb");

	const UNIQUE: bool = true;

	type Params<'w, 's> = Params<'w, 's>;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		default()
	}

	fn ui(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Params {
			mut set,
			mut filter,
		} = params;

		let app_type_registry = set.p1().app_type_registry.clone();
		let type_registry = app_type_registry.read();

		let world = set.p0();

		let mut assets = type_registry
			.iter()
			.filter_map(|registration| {
				let reflect_asset = registration.data::<ReflectAsset>()?;
				let name = registration.type_info().type_path_table().short_path();
				(filter.is_empty() || name.to_lowercase().contains(filter.as_str()))
					.then(|| (name, registration.type_id(), reflect_asset))
			})
			.collect::<Vec<_>>();

		assets.sort_by(|(name_a, ..), (name_b, ..)| name_a.cmp(name_b));

		let mut selection = None;
		let current_selection = world.resource::<InspectorSelection>();

		ui.text_edit_singleline(&mut *filter).changed();

		for (asset_name, asset_type_id, reflect_asset) in assets {
			let handles = reflect_asset.ids(world).collect::<Vec<_>>();

			ui.collapsing(format!("{asset_name} ({})", handles.len()), |ui| {
				for handle in handles {
					let selected = match current_selection {
						InspectorSelection::Asset(_, _, selected_id) => *selected_id == handle,
						_ => false,
					};

					if ui
						.selectable_label(selected, format!("{:?}", handle))
						.clicked()
					{
						selection = Some(InspectorSelection::Asset(
							asset_type_id,
							asset_name.to_string(),
							handle,
						));
					}
				}
			});
		}

		if let Some(selection) = selection {
			*set.p1().inspector_selection = selection;
		}
	}
}
