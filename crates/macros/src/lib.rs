use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Literal, Span};
use quote::quote;
use syn::{DeriveInput, Ident, Lit, parse_macro_input};

#[proc_macro_derive(Identifiable, attributes(id))]
pub fn identifiable(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);

	let name = input.ident;
	let name_str = name.to_string();
	let name_lit = Literal::string(&name_str);

	let Some(id_attr) = input.attrs.iter().find_map(|attr| {
		if attr.path().is_ident("id") {
			attr
				.parse_args::<Lit>()
				.ok()
				.and_then(|l| if let Lit::Str(s) = l { Some(s) } else { None })
		} else {
			None
		}
	}) else {
		panic!("Missing valid #[id(\"...\")] attribute");
	};

	let crate_name =
		crate_name("beditor").expect("beditor should be present for this macro to be used");

	let span = Span::call_site();
	let beditor = match crate_name {
		FoundCrate::Itself => Ident::new("crate", span),
		FoundCrate::Name(name) => Ident::new(&name, span),
	};

	let expanded = quote! {
		impl #beditor::Identifiable for #name {
			const ID: #beditor::uuid::Uuid = #beditor::uuid::uuid!(#id_attr);
			const TYPE_NAME: &'static str = #name_lit;
		}
	};

	TokenStream::from(expanded)
}
