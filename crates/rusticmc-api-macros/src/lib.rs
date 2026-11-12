//! Procedural macros of the Rustic guest SDK. Use them through `rusticmc-api`, which re-exports
//! them (`#[rustic::plugin]`, `#[rustic::handlers]`, ...); this crate is an implementation detail.
//!
//! * `#[plugin]` on the plugin struct generates the component's export glue (one global instance
//!   built with `Default`, kept in a thread-local cell by the SDK runtime).
//! * `#[handlers]` on an inherent `impl` block collects the methods marked `#[event_handler]`,
//!   `#[command]` and `#[completer]` and registers them right after `Plugin::enable` returns.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    Expr, ExprArray, ExprLit, FnArg, ImplItem, Item, ItemImpl, Lit, LitStr, Type, parse::Parser,
    punctuated::Punctuated, spanned::Spanned,
};

/// Path to the SDK crate as the plugin crate names it (`::rusticmc_api` or a rename such as
/// `::rustic`).
fn sdk_path() -> TokenStream2 {
    use proc_macro_crate::{FoundCrate, crate_name};
    match crate_name("rusticmc-api") {
        Ok(FoundCrate::Name(name)) => {
            let ident = syn::Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
        // Inside the SDK itself (doc tests) or when the lookup fails: the canonical name.
        _ => quote!(::rusticmc_api),
    }
}

/// Marks the plugin's main type and generates the component exports for it.
///
/// The type must implement `Default` and `rusticmc_api::Plugin`. Exactly one per plugin crate.
///
/// ```ignore
/// #[rustic::plugin]
/// #[derive(Default)]
/// struct Hello { joins: u64 }
/// ```
#[proc_macro_attribute]
pub fn plugin(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return syn::Error::new(
            TokenStream2::from(attr).span(),
            "#[plugin] takes no arguments",
        )
        .to_compile_error()
        .into();
    }
    let parsed = match syn::parse::<Item>(item) {
        Ok(item) => item,
        Err(e) => return e.to_compile_error().into(),
    };
    let (ident, generics) = match &parsed {
        Item::Struct(s) => (&s.ident, &s.generics),
        Item::Enum(e) => (&e.ident, &e.generics),
        other => {
            return syn::Error::new(
                other.span(),
                "#[plugin] goes on the plugin's struct or enum",
            )
            .to_compile_error()
            .into();
        }
    };
    if !generics.params.is_empty() {
        return syn::Error::new(generics.span(), "the plugin type cannot be generic")
            .to_compile_error()
            .into();
    }
    let sdk = sdk_path();
    quote! {
        #parsed
        #sdk::export_plugin!(#ident);
    }
    .into()
}

/// Collects `#[event_handler]`, `#[command]` and `#[completer]` methods of an inherent impl
/// block of the plugin type and registers them after `Plugin::enable`.
///
/// ```ignore
/// #[rustic::handlers]
/// impl Hello {
///     #[event_handler(priority = "high", ignore_cancelled = true)]
///     fn on_chat(&mut self, event: &mut PlayerChatEvent) { /* ... */ }
///
///     #[command(name = "hello", aliases = ["hi"], usage = "[target:player]")]
///     fn hello(&mut self, cmd: &mut CommandCtx) -> CommandResult { Ok(()) }
/// }
/// ```
#[proc_macro_attribute]
pub fn handlers(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return syn::Error::new(
            TokenStream2::from(attr).span(),
            "#[handlers] takes no arguments",
        )
        .to_compile_error()
        .into();
    }
    let mut imp = match syn::parse::<ItemImpl>(item) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error().into(),
    };
    match expand_handlers(&mut imp) {
        Ok(ts) => ts.into(),
        Err(e) => {
            let err = e.to_compile_error();
            quote!(#imp #err).into()
        }
    }
}

/// Marks an event handler method; only meaningful inside a `#[handlers]` impl block.
#[proc_macro_attribute]
pub fn event_handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    misplaced("event_handler", item)
}

/// Marks a command executor method; only meaningful inside a `#[handlers]` impl block.
#[proc_macro_attribute]
pub fn command(_attr: TokenStream, item: TokenStream) -> TokenStream {
    misplaced("command", item)
}

/// Marks a tab-completion method for a command; only meaningful inside a `#[handlers]` block.
#[proc_macro_attribute]
pub fn completer(_attr: TokenStream, item: TokenStream) -> TokenStream {
    misplaced("completer", item)
}

fn misplaced(name: &str, item: TokenStream) -> TokenStream {
    let item = TokenStream2::from(item);
    let msg =
        format!("#[{name}] only works on a method inside an impl block marked #[rustic::handlers]");
    let err = syn::Error::new(item.span(), msg).to_compile_error();
    quote!(#err #item).into()
}

/// The marker attribute a method carries, if any (matched on the last path segment so both
/// `#[command]` and `#[rustic::command]` work).
fn marker_kind(attr: &syn::Attribute) -> Option<&'static str> {
    let last = attr.path().segments.last()?.ident.to_string();
    match last.as_str() {
        "event_handler" => Some("event_handler"),
        "command" => Some("command"),
        "completer" => Some("completer"),
        _ => None,
    }
}

struct CommandSpec {
    method: syn::Ident,
    name: String,
    aliases: Vec<String>,
    permission: Option<String>,
    usages: Vec<String>,
    description: Option<String>,
    players_only: bool,
    completer: Option<syn::Ident>,
}

fn expand_handlers(imp: &mut ItemImpl) -> syn::Result<TokenStream2> {
    if imp.trait_.is_some() {
        return Err(syn::Error::new(
            imp.span(),
            "#[handlers] goes on an inherent impl block (`impl MyPlugin { ... }`), not a trait impl",
        ));
    }
    if !imp.generics.params.is_empty() {
        return Err(syn::Error::new(
            imp.generics.span(),
            "the plugin type cannot be generic",
        ));
    }
    let sdk = sdk_path();
    let self_ty = imp.self_ty.clone();
    let mut registrations = Vec::new();
    let mut commands: Vec<CommandSpec> = Vec::new();
    let mut completers: Vec<(String, syn::Ident)> = Vec::new();

    for item in &mut imp.items {
        let ImplItem::Fn(func) = item else { continue };
        let mut kept = Vec::new();
        let mut markers = Vec::new();
        for attr in func.attrs.drain(..) {
            match marker_kind(&attr) {
                Some(kind) => markers.push((kind, attr)),
                None => kept.push(attr),
            }
        }
        func.attrs = kept;
        for (kind, attr) in markers {
            match kind {
                "event_handler" => {
                    registrations.push(event_registration(&sdk, func, &attr)?);
                }
                "command" => commands.push(parse_command(func, &attr)?),
                "completer" => {
                    let cmd = parse_completer(&attr)?;
                    completers.push((cmd, func.sig.ident.clone()));
                }
                _ => unreachable!(),
            }
        }
    }

    for (cmd_name, method) in completers {
        match commands.iter_mut().find(|c| c.name == cmd_name) {
            Some(c) if c.completer.is_none() => c.completer = Some(method),
            Some(_) => {
                return Err(syn::Error::new(
                    method.span(),
                    format!("command `{cmd_name}` already has a completer"),
                ));
            }
            None => {
                return Err(syn::Error::new(
                    method.span(),
                    format!(
                        "#[completer] names `{cmd_name}`, but no #[command] in this block has that name"
                    ),
                ));
            }
        }
    }

    for c in &commands {
        let method = &c.method;
        let name = &c.name;
        let mut chain = quote!(#sdk::Command::new(#name));
        if let Some(d) = &c.description {
            chain = quote!(#chain.description(#d));
        }
        for a in &c.aliases {
            chain = quote!(#chain.alias(#a));
        }
        if let Some(p) = &c.permission {
            chain = quote!(#chain.permission(#p));
        }
        if c.players_only {
            chain = quote!(#chain.players_only());
        }
        for u in &c.usages {
            chain = quote!(#chain.usage(#u));
        }
        chain = quote! {
            #chain.executor(|__cmd: &mut #sdk::CommandCtx| {
                #sdk::with_plugin::<Self, _>(|__p| #sdk::__rt::IntoCommandResult::into_command_result(__p.#method(__cmd)))
            })
        };
        if let Some(comp) = &c.completer {
            chain = quote! {
                #chain.completer(|__c: &#sdk::CompleteCtx| {
                    #sdk::with_plugin::<Self, _>(|__p| __p.#comp(__c))
                })
            };
        }
        registrations.push(quote! { ctx.command(#chain)?; });
    }

    let count = registrations.len();
    Ok(quote! {
        #imp

        impl #sdk::__rt::HandlerSet for #self_ty {
            const COUNT: usize = #count;
            fn register(ctx: &mut #sdk::Ctx) -> #sdk::Result<()> {
                #(#registrations)*
                Ok(())
            }
        }
    })
}

/// The type behind `event: &mut E` / `event: &E`, the method's second parameter.
fn event_type(func: &syn::ImplItemFn) -> syn::Result<Type> {
    let mut inputs = func.sig.inputs.iter();
    match inputs.next() {
        Some(FnArg::Receiver(_)) => {}
        _ => {
            return Err(syn::Error::new(
                func.sig.span(),
                "an event handler takes `&mut self` (or `&self`) first",
            ));
        }
    }
    let Some(FnArg::Typed(pt)) = inputs.next() else {
        return Err(syn::Error::new(
            func.sig.span(),
            "an event handler takes the event as its second parameter: `event: &mut PlayerJoinEvent`",
        ));
    };
    if inputs.next().is_some() {
        return Err(syn::Error::new(
            func.sig.span(),
            "an event handler takes exactly one event parameter",
        ));
    }
    match &*pt.ty {
        Type::Reference(r) => Ok((*r.elem).clone()),
        other => Err(syn::Error::new(
            other.span(),
            "take the event by reference: `event: &mut XxxEvent`",
        )),
    }
}

fn event_registration(
    sdk: &TokenStream2,
    func: &syn::ImplItemFn,
    attr: &syn::Attribute,
) -> syn::Result<TokenStream2> {
    let ety = event_type(func)?;
    let mut priority = format_ident!("Normal");
    let mut ignore_cancelled = false;
    let mut custom: Option<String> = None;
    if !matches!(attr.meta, syn::Meta::Path(_)) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("priority") {
                let s: LitStr = meta.value()?.parse()?;
                let p = match s.value().to_ascii_lowercase().as_str() {
                    "lowest" => "Lowest",
                    "low" => "Low",
                    "normal" => "Normal",
                    "high" => "High",
                    "highest" => "Highest",
                    "monitor" => "Monitor",
                    _ => {
                        return Err(meta.error(
                            "priority is one of \"lowest\", \"low\", \"normal\", \"high\", \"highest\", \"monitor\"",
                        ));
                    }
                };
                priority = format_ident!("{}", p);
                Ok(())
            } else if meta.path.is_ident("ignore_cancelled") {
                if meta.input.peek(syn::Token![=]) {
                    let b: syn::LitBool = meta.value()?.parse()?;
                    ignore_cancelled = b.value;
                } else {
                    ignore_cancelled = true;
                }
                Ok(())
            } else if meta.path.is_ident("custom") {
                let s: LitStr = meta.value()?.parse()?;
                custom = Some(s.value());
                Ok(())
            } else {
                Err(meta.error("expected `priority`, `ignore_cancelled` or `custom`"))
            }
        })?;
    }
    let method = &func.sig.ident;
    let label = method.to_string();
    let call = quote! {
        |__e: &mut #ety| {
            #sdk::with_plugin::<Self, _>(|__p| {
                #sdk::__rt::IntoHandlerResult::report(__p.#method(__e), #label)
            })
        }
    };
    Ok(match custom {
        Some(name) => quote! {
            ctx.on_custom_with(#name, #sdk::Priority::#priority, #ignore_cancelled, #call)?;
        },
        None => quote! {
            ctx.on_with(#sdk::Priority::#priority, #ignore_cancelled, #call)?;
        },
    })
}

fn lit_strings(expr: &Expr) -> syn::Result<Vec<String>> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) => Ok(vec![s.value()]),
        Expr::Array(ExprArray { elems, .. }) => elems
            .iter()
            .map(|e| match e {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) => Ok(s.value()),
                other => Err(syn::Error::new(other.span(), "expected a string literal")),
            })
            .collect(),
        other => Err(syn::Error::new(
            other.span(),
            "expected a string literal or an array of string literals",
        )),
    }
}

fn parse_command(func: &syn::ImplItemFn, attr: &syn::Attribute) -> syn::Result<CommandSpec> {
    let mut spec = CommandSpec {
        method: func.sig.ident.clone(),
        name: func.sig.ident.to_string(),
        aliases: Vec::new(),
        permission: None,
        usages: Vec::new(),
        description: None,
        players_only: false,
        completer: None,
    };
    if !matches!(attr.meta, syn::Meta::Path(_)) {
        attr.parse_nested_meta(|meta| {
            let key = meta
                .path
                .get_ident()
                .map(ToString::to_string)
                .unwrap_or_default();
            match key.as_str() {
                "players_only" => {
                    if meta.input.peek(syn::Token![=]) {
                        let b: syn::LitBool = meta.value()?.parse()?;
                        spec.players_only = b.value;
                    } else {
                        spec.players_only = true;
                    }
                    Ok(())
                }
                "completer" => {
                    let s: LitStr = meta.value()?.parse()?;
                    spec.completer = Some(syn::Ident::new(&s.value(), s.span()));
                    Ok(())
                }
                "name" | "permission" | "description" => {
                    let s: LitStr = meta.value()?.parse()?;
                    match key.as_str() {
                        "name" => spec.name = s.value(),
                        "permission" => spec.permission = Some(s.value()),
                        _ => spec.description = Some(s.value()),
                    }
                    Ok(())
                }
                "aliases" | "usage" | "usages" => {
                    let e: Expr = meta.value()?.parse()?;
                    let v = lit_strings(&e)?;
                    if key == "aliases" {
                        spec.aliases.extend(v);
                    } else {
                        spec.usages.extend(v);
                    }
                    Ok(())
                }
                _ => Err(meta.error(
                    "expected one of `name`, `aliases`, `permission`, `usage`, `description`, `players_only`, `completer`",
                )),
            }
        })?;
    }
    Ok(spec)
}

/// `#[completer("home")]` or `#[completer(command = "home")]`.
fn parse_completer(attr: &syn::Attribute) -> syn::Result<String> {
    let syn::Meta::List(list) = &attr.meta else {
        return Err(syn::Error::new(
            attr.span(),
            "name the command: #[completer(\"home\")]",
        ));
    };
    if let Ok(s) = syn::parse2::<LitStr>(list.tokens.clone()) {
        return Ok(s.value());
    }
    let parser = Punctuated::<syn::MetaNameValue, syn::Token![,]>::parse_terminated;
    let args = parser.parse2(list.tokens.clone())?;
    for nv in args {
        if nv.path.is_ident("command")
            && let Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) = &nv.value
        {
            return Ok(s.value());
        }
    }
    Err(syn::Error::new(
        attr.span(),
        "name the command: #[completer(\"home\")]",
    ))
}
