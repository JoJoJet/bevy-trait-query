use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{parse_quote, ItemTrait, Lifetime};

#[proc_macro_attribute]
pub fn queryable(_attr: TokenStream, item: TokenStream) -> TokenStream {
    impl_trait_query(item.into()).into()
}

fn impl_trait_query(item: TokenStream) -> TokenStream2 {
    let mut trait_definition = syn::parse::<ItemTrait>(item).unwrap();
    let trait_name = trait_definition.ident.clone();

    let (impl_generics, trait_generics, where_clause) = trait_definition.generics.split_for_impl();

    trait_definition
        .supertraits
        .push(syn::TypeParamBound::Lifetime(Lifetime::new(
            "'static",
            Span::call_site(),
        )));

    let trait_query = quote! { bevy_trait_query::TraitQuery };

    let trait_query_marker = quote! { bevy_trait_query::TraitQueryMarker };
    let component = quote! { bevy_trait_query::imports::Component };

    let mut marker_generics = trait_definition.generics.clone();
    marker_generics
        .params
        .push(parse_quote!(__T: #trait_name + #component));
    let (marker_impl_generics, ..) = marker_generics.split_for_impl();

    let trait_query_marker_impl = quote! {
        impl #marker_impl_generics #trait_query_marker::<dyn #trait_name #trait_generics> for (__T,)
        #where_clause
        {
            type Covered = __T;
            fn cast(ptr: *mut u8) -> *mut dyn #trait_name #trait_generics {
                ptr as *mut __T as *mut _
            }
        }
    };

    quote! {
        #trait_definition

        impl #impl_generics #trait_query for dyn #trait_name #trait_generics #where_clause {}

        #trait_query_marker_impl
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
