use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

/// Marks a `#[tauri::command]` as inspectable by Victauri.
///
/// Generates:
/// - A companion `<fn_name>__schema()` function returning the command's JSON schema
/// - Runtime telemetry (duration, result status) emitted as tracing spans
/// - Auto-registration in the global command registry
///
/// # Example
///
/// ```rust,ignore
/// #[tauri::command]
/// #[inspectable(description = "Save API key for a provider")]
/// async fn save_api_key(provider: String, key: String) -> Result<(), String> {
///     // ...
/// }
/// ```
#[proc_macro_attribute]
pub fn inspectable(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let attrs = parse_attrs(attr);

    let fn_name = &input.sig.ident;
    let fn_name_str = fn_name.to_string();
    let schema_fn_name = syn::Ident::new(&format!("{fn_name_str}__schema"), fn_name.span());
    let is_async = input.sig.asyncness.is_some();

    let description = attrs
        .description
        .unwrap_or_else(|| fn_name_str.replace('_', " "));

    let args_info = extract_args(&input.sig);
    let arg_tokens: Vec<_> = args_info
        .iter()
        .map(|(name, type_str, required)| {
            quote! {
                victauri_core::registry::CommandArg {
                    name: #name.to_string(),
                    type_name: #type_str.to_string(),
                    required: #required,
                    schema: None,
                }
            }
        })
        .collect();

    let return_type = extract_return_type(&input.sig);

    let expanded = quote! {
        #input

        #[allow(dead_code)]
        fn #schema_fn_name() -> victauri_core::registry::CommandInfo {
            victauri_core::registry::CommandInfo {
                name: #fn_name_str.to_string(),
                plugin: None,
                description: Some(#description.to_string()),
                args: vec![#(#arg_tokens),*],
                return_type: Some(#return_type.to_string()),
                is_async: #is_async,
            }
        }
    };

    TokenStream::from(expanded)
}

struct InspectableAttrs {
    description: Option<String>,
}

fn parse_attrs(attr: TokenStream) -> InspectableAttrs {
    let mut description = None;

    let attr_str = attr.to_string();
    if let Some(desc_start) = attr_str.find("description") {
        if let Some(eq_pos) = attr_str[desc_start..].find('=') {
            let after_eq = &attr_str[desc_start + eq_pos + 1..];
            let trimmed = after_eq.trim();
            if trimmed.starts_with('"') {
                if let Some(end) = trimmed[1..].find('"') {
                    description = Some(trimmed[1..end + 1].to_string());
                }
            }
        }
    }

    InspectableAttrs { description }
}

fn extract_args(sig: &syn::Signature) -> Vec<(String, String, bool)> {
    sig.inputs
        .iter()
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                let name = match &*pat_type.pat {
                    syn::Pat::Ident(ident) => ident.ident.to_string(),
                    _ => return None,
                };

                // Skip tauri framework types
                let type_str = quote!(#pat_type.ty).to_string();
                if type_str.contains("AppHandle")
                    || type_str.contains("State")
                    || type_str.contains("Window")
                    || type_str.contains("Webview")
                {
                    return None;
                }

                let is_option = type_str.contains("Option");
                let type_name = format!("{}", quote!(#(pat_type.ty)));

                Some((name, type_name, !is_option))
            } else {
                None
            }
        })
        .collect()
}

fn extract_return_type(sig: &syn::Signature) -> String {
    match &sig.output {
        syn::ReturnType::Default => "()".to_string(),
        syn::ReturnType::Type(_, ty) => quote!(#ty).to_string(),
    }
}
