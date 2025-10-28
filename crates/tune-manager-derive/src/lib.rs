//! Derive macro for the ProcessingError trait.
//!
//! This crate provides a derive macro for automatically implementing the `ProcessingError`
//! trait on error enums. Use the `#[CausesSkip]` attribute on enum variants to mark them
//! as causing stage skips.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

/// Derive macro for implementing the ProcessingError trait.
///
/// # Example
///
/// ```ignore
/// #[derive(ProcessingError, Error, Debug)]
/// pub enum MyError {
///     #[CausesSkip]
///     #[error("Not configured")]
///     NotConfigured,
///
///     #[error("Something went wrong")]
///     SomethingWrong,
/// }
/// ```
///
/// This will generate an implementation where `NotConfigured` causes the stage to be skipped.
#[proc_macro_derive(ProcessingError, attributes(CausesSkip))]
pub fn derive_processing_error(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => panic!("ProcessingError can only be derived for enums"),
    };

    // Build match arms for causes_skip()
    let causes_skip_arms = variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let has_causes_skip = variant
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("CausesSkip"));

        // Handle different field types (unit, tuple, struct)
        let pattern = match &variant.fields {
            Fields::Unit => quote! { #name::#variant_name },
            Fields::Unnamed(_) => quote! { #name::#variant_name(..) },
            Fields::Named(_) => quote! { #name::#variant_name { .. } },
        };

        if has_causes_skip {
            quote! {
                #pattern => true,
            }
        } else {
            quote! {
                #pattern => false,
            }
        }
    });

    let expanded = quote! {
        impl crate::processing::error::ProcessingError for #name {
            fn causes_skip(&self) -> bool {
                match self {
                    #(#causes_skip_arms)*
                }
            }
        }
    };

    TokenStream::from(expanded)
}
