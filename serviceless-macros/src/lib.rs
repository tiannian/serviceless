use proc_macro::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote, Error, Ident, ImplItem, ItemImpl, LitStr, Token, Type,
};

struct ServiceArgs {
    runtime: Option<Type>,
    name: Option<LitStr>,
}

impl Parse for ServiceArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut args = Self {
            runtime: None,
            name: None,
        };

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;

            match key.to_string().as_str() {
                "runtime" if args.runtime.is_none() => {
                    args.runtime = Some(syn::parse_str(&value.value()).map_err(|error| {
                        Error::new(value.span(), format!("invalid runtime type: {error}"))
                    })?);
                }
                "name" if args.name.is_none() => args.name = Some(value),
                "runtime" | "name" => return Err(Error::new(key.span(), "duplicate option")),
                _ => return Err(Error::new(key.span(), "expected `runtime` or `name`")),
            }

            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(args)
    }
}

/// Implement a service, supplying its runtime, metadata name, and empty stream by default.
///
/// `runtime` is a quoted Rust type path. `name` is a string literal. The default
/// metadata name is the implementing type's final path segment.
#[proc_macro_attribute]
pub fn service(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as ServiceArgs);
    let mut item = parse_macro_input!(item as ItemImpl);

    match expand(&args, &mut item) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand(args: &ServiceArgs, item: &mut ItemImpl) -> syn::Result<proc_macro2::TokenStream> {
    let Some((_, trait_path, _)) = &item.trait_ else {
        return Err(Error::new_spanned(item, "expected `impl Service for Type`"));
    };
    if trait_path
        .segments
        .last()
        .is_none_or(|part| part.ident != "Service")
    {
        return Err(Error::new_spanned(
            trait_path,
            "expected an implementation of `Service`",
        ));
    }

    let name_ident = match item.self_ty.as_ref() {
        Type::Path(path) => path.path.segments.last().map(|part| &part.ident),
        _ => None,
    }
    .ok_or_else(|| Error::new_spanned(&item.self_ty, "expected a named service type"))?;

    let mut has_stream = false;
    let mut has_metadata = false;
    for member in &item.items {
        match member {
            ImplItem::Type(associated) if associated.ident == "Runtime" => {
                return Err(Error::new_spanned(
                    associated,
                    "`Runtime` is supplied by #[service]; use `runtime = \"...\"` to override it",
                ));
            }
            ImplItem::Type(associated) if associated.ident == "Stream" => has_stream = true,
            ImplItem::Fn(method) if method.sig.ident == "metadata" => has_metadata = true,
            _ => {}
        }
    }

    if has_metadata && args.name.is_some() {
        return Err(Error::new_spanned(
            &item.self_ty,
            "use either `name` or an explicit `metadata` method",
        ));
    }

    let facade = match crate_name("serviceless") {
        Ok(FoundCrate::Itself) => quote!(::serviceless),
        Ok(FoundCrate::Name(name)) => {
            let ident = format_ident!("{}", name);
            quote!(::#ident)
        }
        Err(error) => {
            return Err(Error::new_spanned(
                &item.self_ty,
                format!("cannot find the `serviceless` dependency: {error}"),
            ));
        }
    };

    let runtime = args
        .runtime
        .as_ref()
        .map(|runtime| quote!(#runtime))
        .unwrap_or_else(|| quote!(#facade::TokioRuntime));
    item.items.insert(0, parse_quote!(type Runtime = #runtime;));

    if !has_stream {
        item.items
            .insert(1, parse_quote!(type Stream = #facade::EmptyStream<Self>;));
    }

    if !has_metadata {
        let name = args
            .name
            .as_ref()
            .map(|name| quote!(#name))
            .unwrap_or_else(|| quote!(stringify!(#name_ident)));
        item.items.push(parse_quote! {
            fn metadata(&self) -> #facade::Metadata<'_> {
                #facade::Metadata { name: #name }
            }
        });
    }

    if !item.attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|part| part.ident == "async_trait")
    }) {
        item.attrs
            .push(parse_quote!(#[#facade::__private::async_trait]));
    }

    Ok(quote!(#item))
}
