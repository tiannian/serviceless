use proc_macro::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote, Attribute, Data, DeriveInput, Error, Fields, GenericArgument,
    Ident, ImplItem, ItemImpl, LitStr, PathArguments, Token, Type,
};

fn facade_path() -> syn::Result<proc_macro2::TokenStream> {
    match crate_name("serviceless") {
        Ok(FoundCrate::Itself) => Ok(quote!(::serviceless)),
        Ok(FoundCrate::Name(name)) => {
            let ident = format_ident!("{}", name);
            Ok(quote!(::#ident))
        }
        Err(error) => Err(Error::new(
            proc_macro2::Span::call_site(),
            format!("cannot find the `serviceless` dependency: {error}"),
        )),
    }
}

struct AssociatedTypeArg {
    ty: Type,
}

fn associated_type(attrs: &[Attribute], attr_name: &str, key_name: &str) -> syn::Result<Type> {
    let mut found = None;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident(attr_name)) {
        if found.is_some() {
            return Err(Error::new_spanned(
                attr,
                format!("duplicate `#[{attr_name}]`"),
            ));
        }
        let parsed = attr.parse_args_with(|input: ParseStream<'_>| {
            let key: Ident = input.parse()?;
            if key != key_name {
                return Err(Error::new(key.span(), format!("expected `{key_name}`")));
            }
            input.parse::<Token![=]>()?;
            let ty = input.parse()?;
            if !input.is_empty() {
                return Err(input.error("unexpected tokens"));
            }
            Ok(AssociatedTypeArg { ty })
        })?;
        found = Some(parsed.ty);
    }
    found.ok_or_else(|| {
        Error::new(
            proc_macro2::Span::call_site(),
            format!("missing `#[{attr_name}({key_name} = ...)]`"),
        )
    })
}

/// Implement `Message` using `#[message(result = ResultType)]`.
#[proc_macro_derive(Message, attributes(message))]
pub fn derive_message(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_message(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_message(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let result = associated_type(&input.attrs, "message", "result")?;
    let facade = facade_path()?;
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #facade::Message for #name #ty_generics #where_clause {
            type Result = #result;
        }
    })
}

/// Implement `Topic` using `#[topic(item = ItemType)]`.
#[proc_macro_derive(Topic, attributes(topic))]
pub fn derive_topic(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_topic(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_topic(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let item = associated_type(&input.attrs, "topic", "item")?;
    let facade = facade_path()?;
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #facade::Topic for #name #ty_generics #where_clause {
            type Item = #item;
        }
    })
}

/// Route fields marked `#[topic]` to their `TopicEndpoint<T>` topic.
#[proc_macro_derive(TopicRoutes, attributes(topic))]
pub fn derive_topic_routes(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_topic_routes(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_topic_routes(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(Error::new_spanned(input, "TopicRoutes requires a struct"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(Error::new_spanned(
            &data.fields,
            "TopicRoutes requires named fields",
        ));
    };
    let facade = facade_path()?;
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let mut routes = Vec::new();
    for field in &fields.named {
        let mut topic_attrs = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("topic"));
        if let Some(attr) = topic_attrs.next() {
            if let Some(duplicate) = topic_attrs.next() {
                return Err(Error::new_spanned(duplicate, "duplicate `#[topic]`"));
            }
            if !matches!(attr.meta, syn::Meta::Path(_)) {
                return Err(Error::new_spanned(attr, "expected `#[topic]`"));
            }
            let field_name = field.ident.as_ref().expect("named field");
            let topic = endpoint_topic(&field.ty)?;
            routes.push(quote! {
                impl #impl_generics #facade::RoutedTopic<#name #ty_generics> for #topic #where_clause {
                    fn endpoint(service: &mut #name #ty_generics)
                        -> &mut #facade::RuntimedTopicEndpoint<Self, <#name #ty_generics as #facade::Service>::Runtime>
                    {
                        &mut service.#field_name
                    }
                }
            });
        }
    }
    if routes.is_empty() {
        return Err(Error::new_spanned(
            input,
            "TopicRoutes requires a `#[topic]` field",
        ));
    }
    Ok(quote!(#(#routes)*))
}

fn endpoint_topic(ty: &Type) -> syn::Result<&Type> {
    let Type::Path(path) = ty else {
        return Err(Error::new_spanned(ty, "expected `TopicEndpoint<T>`"));
    };
    let Some(segment) = path.path.segments.last() else {
        return Err(Error::new_spanned(ty, "expected `TopicEndpoint<T>`"));
    };
    let expected_args = match segment.ident.to_string().as_str() {
        "TopicEndpoint" => 1,
        "RuntimedTopicEndpoint" => 2,
        _ => {
            return Err(Error::new_spanned(
                ty,
                "expected `TopicEndpoint<T>` or `RuntimedTopicEndpoint<T, R>`",
            ))
        }
    };
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err(Error::new_spanned(ty, "expected endpoint type arguments"));
    };
    if args.args.len() != expected_args {
        return Err(Error::new_spanned(ty, "unexpected endpoint type arguments"));
    }
    match &args.args[0] {
        GenericArgument::Type(topic) => Ok(topic),
        _ => Err(Error::new_spanned(ty, "expected a topic type")),
    }
}

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

    let facade = facade_path()?;

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
