use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro_error::{abort, proc_macro_error};
use proc_macro2::{Literal, Span};
use quote::quote;
use syn::{
	DeriveInput, Ident, Lit, LitInt, parse::Parse, parse_macro_input, punctuated::Punctuated, token,
};

#[proc_macro_error]
#[proc_macro_derive(EditorAsset, attributes(ns))]
pub fn editor_asset(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let name = input.ident;

	let ns_value = input.attrs.iter().find_map(|attr| {
		if attr.path().is_ident("ns") {
			match attr.parse_args::<Lit>() {
				Ok(value) => {
					if let Lit::Str(s) = value {
						Some(s)
					} else {
						None
					}
				}
				Err(err) => {
					abort!(name, format!("{err}"));
				}
			}
		} else {
			None
		}
	});

	let tt_macro = if let Some(ns) = ns_value {
		quote! { #[typetag::serde(name = #ns)] }
	} else {
		quote! { #[typetag::serde] }
	};

	let crate_name =
		crate_name("beditor").expect("beditor should be present for this macro to be used");
	let span = Span::call_site();
	let beditor = match crate_name {
		FoundCrate::Itself => Ident::new("crate", span),
		FoundCrate::Name(name) => Ident::new(&name, span),
	};

	quote! {
		#tt_macro
		impl #beditor::AssetDef for #name { }
	}
	.into()
}

#[proc_macro_error]
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
		abort!(name, "Missing valid #[id(\"...\")] attribute");
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

struct NameOfEnumStruct {
	member: Ident,
	name: Ident,
	variant: Ident,
}

impl Parse for NameOfEnumStruct {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		// name_of_enum_tuple(0 in Foo::Bar)
		let member = input.parse()?;
		input.parse::<token::In>()?;
		let name = input.parse()?;
		input.parse::<token::PathSep>()?;
		let variant = input.parse()?;

		Ok(Self {
			member,
			name,
			variant,
		})
	}
}

#[proc_macro_error]
#[proc_macro]
pub fn name_of_enum_struct(input: TokenStream) -> TokenStream {
	let args = parse_macro_input!(input as NameOfEnumStruct);

	let NameOfEnumStruct {
		member,
		name,
		variant,
	} = args;

	let variant_name = variant.to_string();
	let member_name = member.to_string();

	quote! {{
		let _ = |f: #name| {
			let #name::#variant { #member: _, .. } = f else {
				return;
			};
		};
		(#variant_name, #member_name)
	}}
	.into()
}

struct NameOfEnumTuple {
	index: LitInt,
	name: Ident,
	variant: Ident,
}

impl Parse for NameOfEnumTuple {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		// name_of_enum_tuple(0 in Foo::Bar)
		let index = input.parse()?;
		input.parse::<token::In>()?;
		let name = input.parse()?;
		input.parse::<token::PathSep>()?;
		let variant = input.parse()?;

		Ok(Self {
			index,
			name,
			variant,
		})
	}
}

#[proc_macro_error]
#[proc_macro]
pub fn name_of_enum_tuple(input: TokenStream) -> TokenStream {
	let args = parse_macro_input!(input as NameOfEnumTuple);

	let NameOfEnumTuple {
		index,
		name,
		variant,
	} = args;

	let Ok(index) = index.base10_parse::<usize>() else {
		abort!(index, "Tuples can only be indexed by a valid usize");
	};

	let variant_name = variant.to_string();
	let index_name = index.to_string();

	let underscores = Punctuated::<token::Underscore, token::Comma>::from_iter(vec![
		token::Underscore(
			Span::call_site()
		);
		index + 1
	]);

	quote! {{
		let _ = |f: #name| {
			let #name::#variant(#underscores, ..) = f else {
				return;
			};
		};
		(#variant_name, #index_name)
	}}
	.into()
}
