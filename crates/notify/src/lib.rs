use bevy::{ecs::query::QueryFilter, prelude::*};
use bevy_egui::{EguiContext, EguiPrimaryContextPass};
use egui_toast::{Toast, ToastKind};
use std::{marker::PhantomData, time::Duration};

#[derive(Event)]
pub struct Notification {
	toast: Toast,
	ctx: Option<Box<dyn ToString + Send + Sync>>,
}

impl Notification {
	pub fn new(toast: Toast) -> Self {
		Self { toast, ctx: None }
	}

	pub fn success(text: impl Into<egui::WidgetText>) -> Self {
		Self {
			toast: toast(ToastKind::Success, text),
			ctx: None,
		}
	}

	pub fn info(text: impl Into<egui::WidgetText>) -> Self {
		Self {
			toast: toast(ToastKind::Info, text),
			ctx: None,
		}
	}

	pub fn warn(text: impl Into<egui::WidgetText>) -> Self {
		Self {
			toast: toast(ToastKind::Warning, text),
			ctx: None,
		}
	}

	pub fn error(text: impl Into<egui::WidgetText>) -> Self {
		Self {
			toast: toast(ToastKind::Error, text),
			ctx: None,
		}
	}

	pub fn with_context(mut self, ctx: impl ToString + Send + Sync + 'static) -> Self {
		self.ctx = Some(Box::new(ctx));
		self
	}
}

impl Command for Notification {
	fn apply(self, world: &mut World) {
		world.trigger(self);
	}
}

fn toast(kind: ToastKind, text: impl Into<egui::WidgetText>) -> Toast {
	Toast::new()
		.text(text)
		.kind(kind)
		.options(egui_toast::ToastOptions::default().duration(Duration::from_secs(5)))
}

pub struct NotificationPlugin<Q: QueryFilter = ()>(PhantomData<Q>);

impl<Q: QueryFilter> Default for NotificationPlugin<Q> {
	fn default() -> Self {
		Self(default())
	}
}

impl<Q: QueryFilter> Plugin for NotificationPlugin<Q>
where
	Q: 'static + Send + Sync,
{
	fn build(&self, app: &mut App) {
		app
			.init_resource::<Toasts>()
			.add_observer(on_notification)
			.add_systems(EguiPrimaryContextPass, show_toasts::<Q>);
	}
}

#[derive(Resource, Deref, DerefMut)]
struct Toasts(egui_toast::Toasts);

impl Default for Toasts {
	fn default() -> Self {
		Self(
			egui_toast::Toasts::new()
				.anchor(egui::Align2::CENTER_BOTTOM, (0.0, -10.0))
				.direction(egui::Direction::BottomUp),
		)
	}
}

fn show_toasts<Q: QueryFilter>(
	mut context: Single<&mut EguiContext, Q>,
	mut toasts: ResMut<Toasts>,
) {
	let ctx = context.get_mut();

	toasts.show(ctx);
}

fn on_notification(event: On<Notification>, toasts: Option<ResMut<Toasts>>) {
	if let Some(mut toasts) = toasts {
		toasts.add(event.toast.clone());
	}

	match event.toast.kind {
		egui_toast::ToastKind::Info => {
			if let Some(ctx) = &event.ctx {
				info!(
					ctx = format!("{}", ctx.to_string()),
					"{}",
					event.toast.text.text()
				)
			} else {
				info!("{}", event.toast.text.text())
			}
		}
		egui_toast::ToastKind::Warning => {
			if let Some(ctx) = &event.ctx {
				warn!(
					ctx = format!("{}", ctx.to_string()),
					"{}",
					event.toast.text.text()
				)
			} else {
				warn!("{}", event.toast.text.text())
			}
		}
		egui_toast::ToastKind::Error => {
			if let Some(ctx) = &event.ctx {
				warn!(
					ctx = format!("{}", ctx.to_string()),
					"{}",
					event.toast.text.text()
				)
			} else {
				warn!("{}", event.toast.text.text())
			}
		}
		_ => (),
	}
}
