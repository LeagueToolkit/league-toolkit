use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::HashSet;
use syn::{
    Data, DataEnum, DataStruct, DeriveInput, Fields, GenericParam, Ident, Type, parse_macro_input,
};

#[proc_macro_derive(DebugSpans)]
pub fn derive_debug_spans(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    derive(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn derive(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;

    let mut generics = input.generics.clone();

    let generic_names: HashSet<Ident> = generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(param) => Some(param.ident.clone()),
            _ => None,
        })
        .collect();

    let mut used_generics = HashSet::new();

    collect_used_generics(&input.data, &generic_names, &mut used_generics);

    if !used_generics.is_empty() {
        let where_clause = generics.make_where_clause();

        for ident in used_generics {
            where_clause
                .predicates
                .push(syn::parse_quote!(#ident: ::std::fmt::Debug));
        }
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let body = match &input.data {
        Data::Struct(data) => derive_struct(name, data),
        Data::Enum(data) => derive_enum(data),
        Data::Union(data) => {
            return Err(syn::Error::new_spanned(
                data.union_token,
                "DebugSpans cannot be derived for unions",
            ));
        }
    };

    Ok(quote! {
        impl #impl_generics ::ltk_ritobin::span::debug::DebugSpans
            for #name #ty_generics #where_clause
        {
            fn fmt(
                &self,
                text: &str,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> ::std::fmt::Result {
                #[allow(unused_imports)]
                use ::ltk_ritobin::span::debug::__private::{DebugField as _, SpannedField as _};
                #body
            }
        }
    })
}

fn collect_used_generics(data: &Data, generic_names: &HashSet<Ident>, used: &mut HashSet<Ident>) {
    let fields = match data {
        Data::Struct(data) => &data.fields,
        Data::Enum(data) => {
            for variant in &data.variants {
                collect_fields(&variant.fields, generic_names, used);
            }
            return;
        }
        Data::Union(_) => return,
    };

    collect_fields(fields, generic_names, used);
}

fn collect_fields(fields: &Fields, generic_names: &HashSet<Ident>, used: &mut HashSet<Ident>) {
    for field in fields {
        collect_type_generics(&field.ty, generic_names, used);
    }
}

fn collect_type_generics(ty: &Type, generic_names: &HashSet<Ident>, used: &mut HashSet<Ident>) {
    struct Visitor<'a> {
        generic_names: &'a HashSet<Ident>,
        used: &'a mut HashSet<Ident>,
    }

    impl<'ast> syn::visit::Visit<'ast> for Visitor<'_> {
        fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
            if let Some(segment) = path.path.segments.last()
                && segment.arguments.is_empty()
                && self.generic_names.contains(&segment.ident)
            {
                self.used.insert(segment.ident.clone());
            }

            syn::visit::visit_type_path(self, path);
        }
    }

    syn::visit::Visit::visit_type(
        &mut Visitor {
            generic_names,
            used,
        },
        ty,
    );
}

fn derive_struct(name: &Ident, data: &DataStruct) -> TokenStream2 {
    match &data.fields {
        Fields::Named(fields) => {
            let fields = fields.named.iter().map(|field| {
                let ident = field.ident.as_ref().unwrap();
                let field_name = ident.to_string();
                let value = debug_value(quote!(self.#ident), &field.ty);

                quote! {
                    builder.field(#field_name, #value);
                }
            });

            quote! {
                let mut builder = f.debug_struct(stringify!(#name));
                #(#fields)*
                builder.finish()
            }
        }

        Fields::Unnamed(fields) => {
            let fields = fields.unnamed.iter().enumerate().map(|(i, field)| {
                let index = syn::Index::from(i);
                let value = debug_value(quote!(self.#index), &field.ty);

                quote! {
                    builder.field(#value);
                }
            });

            quote! {
                let mut builder = f.debug_tuple(stringify!(#name));
                #(#fields)*
                builder.finish()
            }
        }

        Fields::Unit => {
            quote! {
                f.write_str(stringify!(#name))
            }
        }
    }
}

fn derive_enum(data: &DataEnum) -> TokenStream2 {
    let arms = data.variants.iter().map(|variant| {
        let variant_name = &variant.ident;

        match &variant.fields {
            Fields::Unit => quote! {
                Self::#variant_name => {
                    f.write_str(concat!(
                        stringify!(Self),
                        "::",
                        stringify!(#variant_name)
                    ))
                }
            },

            Fields::Unnamed(fields) => {
                let bindings: Vec<_> = (0..fields.unnamed.len())
                    .map(|i| Ident::new(&format!("__field_{i}"), proc_macro2::Span::call_site()))
                    .collect();

                let values = fields
                    .unnamed
                    .iter()
                    .zip(&bindings)
                    .map(|(field, binding)| debug_value(quote!(#binding), &field.ty));

                quote! {
                    Self::#variant_name(#(#bindings),*) => {
                        let mut builder = f.debug_tuple(
                            concat!(
                                stringify!(Self),
                                "::",
                                stringify!(#variant_name)
                            )
                        );

                        #(
                            builder.field(#values);
                        )*

                        builder.finish()
                    }
                }
            }

            Fields::Named(fields) => {
                let bindings: Vec<_> = fields
                    .named
                    .iter()
                    .map(|field| field.ident.clone().unwrap())
                    .collect();

                let values = fields.named.iter().zip(&bindings).map(|(field, binding)| {
                    let field_name = binding.to_string();
                    let value = debug_value(quote!(#binding), &field.ty);

                    quote! {
                        builder.field(#field_name, #value);
                    }
                });

                quote! {
                    Self::#variant_name { #(#bindings),* } => {
                        let mut builder = f.debug_struct(
                            concat!(
                                stringify!(Self),
                                "::",
                                stringify!(#variant_name)
                            )
                        );

                        #(#values)*

                        builder.finish()
                    }
                }
            }
        }
    });

    quote! {
        match self {
            #(#arms),*
        }
    }
}

fn debug_value(value: TokenStream2, ty: &Type) -> TokenStream2 {
    if is_spanned(ty) {
        quote! {
            &::ltk_ritobin::span::debug::__private::spanned(
                (#value).span,
                (&::ltk_ritobin::span::debug::__private::Probe(&(#value).value, text)).dbg_field(),
                text,
            )
        }
    } else {
        quote! {
            &(&::ltk_ritobin::span::debug::__private::Probe(&#value, text)).dbg_field()
        }
    }
}

fn is_spanned(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };

    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Spanned")
}
