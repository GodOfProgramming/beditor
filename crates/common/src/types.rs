use bevy::prelude::*;

pub fn pretty_name<T>() -> String {
	disqualified::ShortName::of::<T>().to_string()
}

pub fn pretty_name_of_str(val: &str) -> String {
	disqualified::ShortName(val).to_string()
}
