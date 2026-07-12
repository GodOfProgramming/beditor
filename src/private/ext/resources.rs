use crate::{
	EditorExtension, inspector::ui::InspectorSelection, private::EditorInternal, ui::EditorUi,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use std::marker::PhantomData;
use uuid::uuid;

#[derive(Default)]
pub struct ResourcesUiExtension;

impl EditorExtension for ResourcesUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<ResourcesUi>();
	}
}

#[derive(Default, Component, Reflect)]
#[require(EditorInternal)]
pub struct ResourcesUi;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	type_registry: Res<'w, AppTypeRegistry>,
	selection: ResMut<'w, InspectorSelection>,

	filter: Local<'s, String>,

	_pd: PhantomData<&'s ()>,
}

impl EditorUi for ResourcesUi {
	const NAME: &str = stringify!(Resources);
	const ID: uuid::Uuid = uuid!("54248a54-9544-4e93-9382-3677b8722952");

	const UNIQUE: bool = true;

	type Params<'w, 's> = Params<'w, 's>;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		default()
	}

	fn ui(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Params {
			type_registry,
			mut selection,
			mut filter,
			..
		} = params;

		let type_registry = type_registry.read();

		let mut resources: Vec<_> = type_registry
			.iter()
			.filter(|registration| registration.data::<ReflectResource>().is_some())
			.filter_map(|registration| {
				let name = registration.type_info().type_path_table().short_path();
				(filter.is_empty() || name.to_lowercase().contains(filter.to_lowercase().as_str()))
					.then(|| (name, registration.type_id()))
			})
			.collect();
		resources.sort_by_key(|r| r.0);

		ui.text_edit_singleline(&mut *filter);

		for (resource_name, type_id) in resources {
			let selected = match *selection {
				InspectorSelection::Resource(selected, _) => selected == type_id,
				_ => false,
			};

			if ui.selectable_label(selected, resource_name).clicked() {
				*selection = InspectorSelection::Resource(type_id, resource_name.to_string());
			}
		}
	}
}
