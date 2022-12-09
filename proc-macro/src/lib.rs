use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{parse_quote, ItemTrait, Result, TraitItem};

/// When added to a trait declaration, generates the impls required to use that trait in queries.
///
/// # Poor use cases
///
/// You should avoid using trait queries for very simple cases that can be solved with more direct solutions.
///
/// One naive use would be querying for a trait that looks something like:
///
/// ```
/// trait Person {
///     fn name(&self) -> &str;
/// }
/// ```
///
/// A far better way of expressing this would be to store the name in a separate component
/// and query for that directly, making `Person` a simple marker component.
///
/// Trait queries are often the most *obvious* solution to a problem, but not always the best one.
/// For examples of strong real-world use-cases, check out the RFC for trait queries in `bevy`:
/// https://github.com/bevyengine/rfcs/pull/39.
///
/// # Note
///
/// This will add the trait bound `'static` to the trait and all of its type parameters.
///
/// You may opt out of this by using the form `#[queryable(no_bounds)]`,
/// but you will have to add the bounds yourself to make it compile.
#[proc_macro_attribute]
pub fn queryable(attr: TokenStream, item: TokenStream) -> TokenStream {
    impl_trait_query(attr, item)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn impl_trait_query(arg: TokenStream, item: TokenStream) -> Result<TokenStream2> {
    syn::custom_keyword!(no_bounds);
    let no_bounds: Option<no_bounds> = syn::parse(arg).map_err(|e| {
        syn::Error::new(
            e.span(),
            "Valid forms are: `#[queryable]` and `#[queryable(no_bounds)]`",
        )
    })?;

    let mut trait_definition = syn::parse::<ItemTrait>(item)?;
    let trait_name = trait_definition.ident.clone();

    // Add `'static` bounds, unless the user asked us not to.
    if !no_bounds.is_some() {
        trait_definition.supertraits.push(parse_quote!('static));

        for param in &mut trait_definition.generics.params {
            // Make sure the parameters to the trait are `'static`.
            if let syn::GenericParam::Type(param) = param {
                param.bounds.push(parse_quote!('static));
            }
        }

        for item in &mut trait_definition.items {
            // Make sure all associated types are `'static`.
            if let TraitItem::Type(assoc) = item {
                assoc.bounds.push(parse_quote!('static));
            }
        }
    }

    let mut impl_generics_list = vec![];
    let mut trait_generics_list = vec![];
    let where_clause = trait_definition.generics.where_clause.clone();

    for param in &trait_definition.generics.params {
        impl_generics_list.push(param.clone());
        match param {
            syn::GenericParam::Type(param) => {
                let ident = &param.ident;
                trait_generics_list.push(quote! { #ident });
            }
            syn::GenericParam::Lifetime(param) => {
                let ident = &param.lifetime;
                trait_generics_list.push(quote! { #ident });
            }
            syn::GenericParam::Const(param) => {
                let ident = &param.ident;
                trait_generics_list.push(quote! { #ident });
            }
        }
    }

    // Add generics for unbounded associated types.
    for item in &trait_definition.items {
        if let TraitItem::Type(assoc) = item {
            if !assoc.generics.params.is_empty() {
                return Err(syn::Error::new(
                    assoc.ident.span(),
                    "Generic associated types are not supported in trait queries",
                ));
            }
            let ident = &assoc.ident;
            let lower_ident = format_ident!("__{ident}");
            let bound = &assoc.bounds;
            impl_generics_list.push(parse_quote! { #lower_ident: #bound });
            trait_generics_list.push(quote! { #ident = #lower_ident });
        }
    }

    let impl_generics = quote! { <#( #impl_generics_list ,)*> };
    let trait_generics = quote! { <#( #trait_generics_list ,)*> };

    let trait_object = quote! { dyn #trait_name #trait_generics };

    let my_crate = proc_macro_crate::crate_name("bevy-trait-query").unwrap();
    let my_crate = match my_crate {
        proc_macro_crate::FoundCrate::Itself => quote! { crate },
        proc_macro_crate::FoundCrate::Name(x) => {
            let ident = quote::format_ident!("{x}");
            quote! { #ident }
        }
    };

    let imports = quote! { #my_crate::imports };

    let trait_query = quote! { #my_crate::TraitQuery };

    let mut marker_impl_generics_list = impl_generics_list.clone();
    marker_impl_generics_list
        .push(parse_quote!(__Component: #trait_name #trait_generics + #imports::Component));
    let marker_impl_generics = quote! { <#( #marker_impl_generics_list ,)*> };

    let marker_impl_code = quote! {
        impl #impl_generics #trait_query for #trait_object #where_clause {}

        impl #marker_impl_generics #my_crate::TraitQueryMarker::<#trait_object> for (__Component,)
        #where_clause
        {
            type Covered = __Component;
            fn cast(ptr: *mut u8) -> *mut #trait_object {
                ptr as *mut __Component as *mut _
            }
        }
    };

    let mut impl_generics_with_lifetime = impl_generics_list.clone();
    impl_generics_with_lifetime.insert(0, parse_quote!('__a));
    let impl_generics_with_lifetime = quote! { <#( #impl_generics_with_lifetime ,)*> };

    let trait_object_query_code = quote! {
        unsafe impl #impl_generics #imports::ReadOnlyWorldQuery for &#trait_object
        #where_clause
        {}

        unsafe impl #impl_generics #imports::ReadOnlyWorldQuery for Added<&#trait_object>
        #where_clause
        {}

        unsafe impl #impl_generics #imports::ReadOnlyWorldQuery for Changed<&#trait_object>
        #where_clause
        {}

        unsafe impl #impl_generics_with_lifetime #imports::WorldQuery for &'__a #trait_object
        #where_clause
        {
            type Item<'__w> = #my_crate::ReadTraits<'__w, #trait_object>;
            type Fetch<'__w> = #my_crate::ReadAllTraitsFetch<'__w, #trait_object>;
            type ReadOnly = Self;
            type State = #my_crate::TraitQueryState<#trait_object>;

            #[inline]
            unsafe fn init_fetch<'w>(
                world: &'w #imports::World,
                state: &Self::State,
                last_change_tick: u32,
                change_tick: u32,
            ) -> Self::Fetch<'w> {
                <#my_crate::All<&#trait_object> as #imports::WorldQuery>::init_fetch(
                    world,
                    state,
                    last_change_tick,
                    change_tick,
                )
            }

            #[inline]
            unsafe fn clone_fetch<'w>(
                fetch: &Self::Fetch<'w>,
            ) -> Self::Fetch<'w> {
                <#my_crate::All<&#trait_object> as #imports::WorldQuery>::clone_fetch(fetch)
            }

            #[inline]
            fn shrink<'wlong: 'wshort, 'wshort>(
                item: Self::Item<'wlong>,
            ) -> Self::Item<'wshort> {
                item
            }

            const IS_DENSE: bool = <#my_crate::All<&#trait_object> as #imports::WorldQuery>::IS_DENSE;
            const IS_ARCHETYPAL: bool =
                <#my_crate::All<&#trait_object> as #imports::WorldQuery>::IS_ARCHETYPAL;

            #[inline]
            unsafe fn set_archetype<'w>(
                fetch: &mut Self::Fetch<'w>,
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
                fetch: &mut Self::Fetch<'w>,
                state: &Self::State,
                table: &'w #imports::Table,
            ) {
                <#my_crate::All<&#trait_object> as #imports::WorldQuery>::set_table(fetch, state, table);
            }

            #[inline]
            unsafe fn fetch<'w>(
                fetch: &mut Self::Fetch<'w>,
                entity: #imports::Entity,
                table_row: usize,
            ) -> Self::Item<'w> {
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
            fn init_state(world: &mut #imports::World) -> Self::State {
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

        unsafe impl #impl_generics_with_lifetime #imports::WorldQuery for &'__a mut #trait_object
        #where_clause
        {
            type Item<'__w> = #my_crate::WriteTraits<'__w, #trait_object>;
            type Fetch<'__w> = #my_crate::WriteAllTraitsFetch<'__w, #trait_object>;
            type ReadOnly = &'__a #trait_object;
            type State = #my_crate::TraitQueryState<#trait_object>;

            #[inline]
            unsafe fn init_fetch<'w>(
                world: &'w #imports::World,
                state: &Self::State,
                last_change_tick: u32,
                change_tick: u32,
            ) -> Self::Fetch<'w> {
                <#my_crate::All<&mut #trait_object> as #imports::WorldQuery>::init_fetch(
                    world,
                    state,
                    last_change_tick,
                    change_tick,
                )
            }

            #[inline]
            unsafe fn clone_fetch<'w>(
                fetch: &Self::Fetch<'w>,
            ) -> Self::Fetch<'w> {
                <#my_crate::All<&mut #trait_object> as #imports::WorldQuery>::clone_fetch(fetch)
            }

            #[inline]
            fn shrink<'wlong: 'wshort, 'wshort>(
                item: Self::Item<'wlong>,
            ) -> Self::Item<'wshort> {
                item
            }

            const IS_DENSE: bool = <#my_crate::All<&mut #trait_object> as #imports::WorldQuery>::IS_DENSE;
            const IS_ARCHETYPAL: bool =
                <#my_crate::All<&mut #trait_object> as #imports::WorldQuery>::IS_ARCHETYPAL;

            #[inline]
            unsafe fn set_archetype<'w>(
                fetch: &mut Self::Fetch<'w>,
                state: &Self::State,
                archetype: &'w #imports::Archetype,
                table: &'w #imports::Table,
            ) {
                <#my_crate::All<&mut #trait_object> as #imports::WorldQuery>::set_archetype(
                    fetch, state, archetype, table,
                );
            }

            #[inline]
            unsafe fn set_table<'w>(
                fetch: &mut Self::Fetch<'w>,
                state: &Self::State,
                table: &'w #imports::Table,
            ) {
                <#my_crate::All<&mut #trait_object> as #imports::WorldQuery>::set_table(fetch, state, table);
            }

            #[inline]
            unsafe fn fetch<'w>(
                fetch: &mut Self::Fetch<'w>,
                entity: #imports::Entity,
                table_row: usize,
            ) -> Self::Item<'w> {
                <#my_crate::All<&mut #trait_object> as #imports::WorldQuery>::fetch(
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
                <#my_crate::All<&mut #trait_object> as #imports::WorldQuery>::update_component_access(
                    state, access,
                );
            }

            #[inline]
            fn update_archetype_component_access(
                state: &Self::State,
                archetype: &#imports::Archetype,
                access: &mut #imports::Access<#imports::ArchetypeComponentId>,
            ) {
                <#my_crate::All<&mut #trait_object> as #imports::WorldQuery>::update_archetype_component_access(state, archetype, access);
            }


            #[inline]
            fn init_state(world: &mut #imports::World) -> Self::State {
                <#my_crate::All<&mut #trait_object> as #imports::WorldQuery>::init_state(world)
            }

            #[inline]
            fn matches_component_set(
                state: &Self::State,
                set_contains_id: &impl Fn(#imports::ComponentId) -> bool,
            ) -> bool {
                <#my_crate::All<&mut #trait_object> as #imports::WorldQuery>::matches_component_set(state, set_contains_id)
            }
        }

        unsafe impl #impl_generics_with_lifetime #imports::WorldQuery for #imports::Added<&'__a #trait_object>
        #where_clause
        {
            type Item<'__w> = #my_crate::ReadTraits<'__w, #trait_object>;
            type Fetch<'__w> = #my_crate::change_detection::ChangeDetectionFetch<'__w, #trait_object>;
            type ReadOnly = Self;
            type State = #my_crate::TraitQueryState<#trait_object>;

            #[inline]
            unsafe fn init_fetch<'w>(
                world: &'w #imports::World,
                state: &Self::State,
                last_change_tick: u32,
                change_tick: u32,
            ) -> Self::Fetch<'w> {
                <#my_crate::change_detection::TraitAdded<&#trait_object> as #imports::WorldQuery>::init_fetch(
                    world,
                    state,
                    last_change_tick,
                    change_tick,
                )
            }

            #[inline]
            unsafe fn clone_fetch<'w>(
                fetch: &Self::Fetch<'w>,
            ) -> Self::Fetch<'w> {
                <#my_crate::change_detection::TraitAdded<&#trait_object> as #imports::WorldQuery>::clone_fetch(fetch)
            }

            #[inline]
            fn shrink<'wlong: 'wshort, 'wshort>(
                item: Self::Item<'wlong>,
            ) -> Self::Item<'wshort> {
                item
            }

            const IS_DENSE: bool = <#my_crate::change_detection::TraitAdded<&#trait_object> as #imports::WorldQuery>::IS_DENSE;
            const IS_ARCHETYPAL: bool =
                <#my_crate::change_detection::TraitAdded<&#trait_object> as #imports::WorldQuery>::IS_ARCHETYPAL;

            #[inline]
            unsafe fn set_archetype<'w>(
                fetch: &mut Self::Fetch<'w>,
                state: &Self::State,
                archetype: &'w #imports::Archetype,
                tables: &'w #imports::Table,
            ) {
                <#my_crate::change_detection::TraitAdded<&#trait_object> as #imports::WorldQuery>::set_archetype(
                    fetch, state, archetype, tables,
                );
            }

            #[inline]
            unsafe fn set_table<'w>(
                fetch: &mut Self::Fetch<'w>,
                state: &Self::State,
                table: &'w #imports::Table,
            ) {
                <#my_crate::change_detection::TraitAdded<&#trait_object> as #imports::WorldQuery>::set_table(fetch, state, table);
            }

            #[inline]
            unsafe fn fetch<'w>(
                fetch: &mut Self::Fetch<'w>,
                entity: #imports::Entity,
                table_row: usize,
            ) -> Self::Item<'w> {
                <#my_crate::change_detection::TraitAdded<&#trait_object> as #imports::WorldQuery>::fetch(
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
                <#my_crate::change_detection::TraitAdded<&#trait_object> as #imports::WorldQuery>::update_component_access(
                    state, access,
                );
            }

            #[inline]
            fn update_archetype_component_access(
                state: &Self::State,
                archetype: &#imports::Archetype,
                access: &mut #imports::Access<#imports::ArchetypeComponentId>,
            ) {
                <#my_crate::change_detection::TraitAdded<&#trait_object> as #imports::WorldQuery>::update_archetype_component_access(state, archetype, access);
            }

            #[inline]
            fn init_state(world: &mut #imports::World) -> Self::State {
                <#my_crate::change_detection::TraitAdded<&#trait_object> as #imports::WorldQuery>::init_state(world)
            }

            #[inline]
            fn matches_component_set(
                state: &Self::State,
                set_contains_id: &impl Fn(#imports::ComponentId) -> bool,
            ) -> bool {
                <#my_crate::change_detection::TraitAdded<&#trait_object> as #imports::WorldQuery>::matches_component_set(state, set_contains_id)
            }
        }

        unsafe impl #impl_generics_with_lifetime #imports::WorldQuery for #imports::Changed<&'__a #trait_object>
        #where_clause
        {
            type Item<'__w> = #my_crate::ReadTraits<'__w, #trait_object>;
            type Fetch<'__w> = #my_crate::change_detection::ChangeDetectionFetch<'__w, #trait_object>;
            type ReadOnly = Self;
            type State = #my_crate::TraitQueryState<#trait_object>;

            #[inline]
            unsafe fn init_fetch<'w>(
                world: &'w #imports::World,
                state: &Self::State,
                last_change_tick: u32,
                change_tick: u32,
            ) -> Self::Fetch<'w> {
                <#my_crate::change_detection::TraitChanged<&#trait_object> as #imports::WorldQuery>::init_fetch(
                    world,
                    state,
                    last_change_tick,
                    change_tick,
                )
            }

            #[inline]
            unsafe fn clone_fetch<'w>(
                fetch: &Self::Fetch<'w>,
            ) -> Self::Fetch<'w> {
                <#my_crate::change_detection::TraitChanged<&#trait_object> as #imports::WorldQuery>::clone_fetch(fetch)
            }

            #[inline]
            fn shrink<'wlong: 'wshort, 'wshort>(
                item: Self::Item<'wlong>,
            ) -> Self::Item<'wshort> {
                item
            }

            const IS_DENSE: bool = <#my_crate::change_detection::TraitChanged<&#trait_object> as #imports::WorldQuery>::IS_DENSE;
            const IS_ARCHETYPAL: bool =
                <#my_crate::change_detection::TraitChanged<&#trait_object> as #imports::WorldQuery>::IS_ARCHETYPAL;

            #[inline]
            unsafe fn set_archetype<'w>(
                fetch: &mut Self::Fetch<'w>,
                state: &Self::State,
                archetype: &'w #imports::Archetype,
                tables: &'w #imports::Table,
            ) {
                <#my_crate::change_detection::TraitChanged<&#trait_object> as #imports::WorldQuery>::set_archetype(
                    fetch, state, archetype, tables,
                );
            }

            #[inline]
            unsafe fn set_table<'w>(
                fetch: &mut Self::Fetch<'w>,
                state: &Self::State,
                table: &'w #imports::Table,
            ) {
                <#my_crate::change_detection::TraitChanged<&#trait_object> as #imports::WorldQuery>::set_table(fetch, state, table);
            }

            #[inline]
            unsafe fn fetch<'w>(
                fetch: &mut Self::Fetch<'w>,
                entity: #imports::Entity,
                table_row: usize,
            ) -> Self::Item<'w> {
                <#my_crate::change_detection::TraitChanged<&#trait_object> as #imports::WorldQuery>::fetch(
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
                <#my_crate::change_detection::TraitChanged<&#trait_object> as #imports::WorldQuery>::update_component_access(
                    state, access,
                );
            }

            #[inline]
            fn update_archetype_component_access(
                state: &Self::State,
                archetype: &#imports::Archetype,
                access: &mut #imports::Access<#imports::ArchetypeComponentId>,
            ) {
                <#my_crate::change_detection::TraitChanged<&#trait_object> as #imports::WorldQuery>::update_archetype_component_access(state, archetype, access);
            }

            #[inline]
            fn init_state(world: &mut #imports::World) -> Self::State {
                <#my_crate::change_detection::TraitChanged<&#trait_object> as #imports::WorldQuery>::init_state(world)
            }

            #[inline]
            fn matches_component_set(
                state: &Self::State,
                set_contains_id: &impl Fn(#imports::ComponentId) -> bool,
            ) -> bool {
                <#my_crate::change_detection::TraitChanged<&#trait_object> as #imports::WorldQuery>::matches_component_set(state, set_contains_id)
            }
        }
    };

    Ok(quote! {
        #trait_definition

        #marker_impl_code

        #trait_object_query_code
    })
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
