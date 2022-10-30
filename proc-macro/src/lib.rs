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

    let trait_object = quote! { dyn #trait_name #trait_generics };

    let my_crate = quote! { bevy_trait_query };
    let imports = quote! { #my_crate::imports };

    let trait_query = quote! { #my_crate::TraitQuery };

    let mut marker_generics = trait_definition.generics.clone();
    marker_generics
        .params
        .push(parse_quote!(__T: #trait_name + #imports::Component));
    let (marker_impl_generics, ..) = marker_generics.split_for_impl();

    let marker_impl_code = quote! {
        impl #impl_generics #trait_query for #trait_object #where_clause {}

        impl #marker_impl_generics #my_crate::TraitQueryMarker::<#trait_object> for (__T,)
        #where_clause
        {
            type Covered = __T;
            fn cast(ptr: *mut u8) -> *mut #trait_object {
                ptr as *mut __T as *mut _
            }
        }
    };

    let mut generics_with_lifetime = trait_definition.generics.clone();
    generics_with_lifetime.params.insert(0, parse_quote!('__a));
    let (impl_generics_with_lifetime, ..) = generics_with_lifetime.split_for_impl();

    let trait_object_query_code = quote! {
        impl #impl_generics_with_lifetime #imports::WorldQueryGats<'__a> for &#trait_object
        #where_clause
        {
            type Item = #my_crate::ReadTraits<'__a, #trait_object>;
            type Fetch = #my_crate::ReadAllTraitsFetch<'__a, #trait_object>;
        }

        unsafe impl #impl_generics #imports::ReadOnlyWorldQuery for &#trait_object {}

        unsafe impl #impl_generics_with_lifetime #imports::WorldQuery for &'__a #trait_object {
            type ReadOnly = Self;
            type State = #my_crate::TraitQueryState<#trait_object>;

            #[inline]
            unsafe fn init_fetch<'w>(
                world: &'w World,
                state: &Self::State,
                last_change_tick: u32,
                change_tick: u32,
            ) -> <Self as #imports::WorldQueryGats<'w>>::Fetch {
                <#my_crate::All<&#trait_object> as #imports::WorldQuery>::init_fetch(
                    world,
                    state,
                    last_change_tick,
                    change_tick,
                )
            }

            #[inline]
            unsafe fn clone_fetch<'w>(
                fetch: &<Self as #imports::WorldQueryGats<'w>>::Fetch,
            ) -> <Self as #imports::WorldQueryGats<'w>>::Fetch {
                <#my_crate::All<&#trait_object> as #imports::WorldQuery>::clone_fetch(fetch)
            }

            #[inline]
            fn shrink<'wlong: 'wshort, 'wshort>(
                item: #imports::QueryItem<'wlong, Self>,
            ) -> #imports::QueryItem<'wshort, Self> {
                item
            }

            const IS_DENSE: bool = <#my_crate::All<&#trait_object> as #imports::WorldQuery>::IS_DENSE;
            const IS_ARCHETYPAL: bool =
                <#my_crate::All<&#trait_object> as #imports::WorldQuery>::IS_ARCHETYPAL;

            #[inline]
            unsafe fn set_archetype<'w>(
                fetch: &mut <Self as #imports::WorldQueryGats<'w>>::Fetch,
                state: &Self::State,
                archetype: &'w #imports::Archetype,
                tables: &'w #imports::Table,
            ) {
                <#my_crate::All<&#trait_object> as #imports::WorldQuery>::set_archetype(
                    fetch, state, archetype, tables,
                );
            }

            #[inline]
            unsafe fn set_table<'w>(
                fetch: &mut <Self as #imports::WorldQueryGats<'w>>::Fetch,
                state: &Self::State,
                table: &'w #imports::Table,
            ) {
                <#my_crate::All<&#trait_object> as #imports::WorldQuery>::set_table(fetch, state, table);
            }

            #[inline]
            unsafe fn fetch<'w>(
                fetch: &mut <Self as #imports::WorldQueryGats<'w>>::Fetch,
                entity: #imports::Entity,
                table_row: usize,
            ) -> <Self as #imports::WorldQueryGats<'w>>::Item {
                <#my_crate::All<&#trait_object> as #imports::WorldQuery>::fetch(
                    fetch,
                    entity,
                    table_row,
                )
            }

            #[inline]
            fn update_component_access(
                state: &Self::State,
                access: &mut #imports::FilteredAccess<#imports::ComponentId>,
            ) {
                <#my_crate::All<&#trait_object> as #imports::WorldQuery>::update_component_access(
                    state, access,
                );
            }

            #[inline]
            fn update_archetype_component_access(
                state: &Self::State,
                archetype: &#imports::Archetype,
                access: &mut #imports::Access<#imports::ArchetypeComponentId>,
            ) {
                <#my_crate::All<&#trait_object> as #imports::WorldQuery>::update_archetype_component_access(state, archetype, access);
            }

            #[inline]
            fn init_state(world: &mut World) -> Self::State {
                <#my_crate::All<&#trait_object> as #imports::WorldQuery>::init_state(world)
            }

            #[inline]
            fn matches_component_set(
                state: &Self::State,
                set_contains_id: &impl Fn(#imports::ComponentId) -> bool,
            ) -> bool {
                <#my_crate::All<&#trait_object> as #imports::WorldQuery>::matches_component_set(state, set_contains_id)
            }
        }
    };

    quote! {
        #trait_definition

        #marker_impl_code

        #trait_object_query_code
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
