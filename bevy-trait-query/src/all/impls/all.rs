use bevy_ecs::{
    component::{ComponentId, Components, Tick},
    entity::Entity,
    query::{QueryData, QueryItem, ReadOnlyQueryData, WorldQuery},
    storage::TableRow,
    world::{unsafe_world_cell::UnsafeWorldCell, World},
};

use crate::{
    debug_unreachable, trait_registry_error, AllTraitsFetch, ReadTraits, TraitQuery,
    TraitQueryState, WriteTraits,
};

/// [`WorldQuery`] adapter that fetches all implementations of a given trait for an entity.
///
/// You can usually just use `&dyn Trait` or `&mut dyn Trait` as a [`WorldQuery`] directly. To be
/// specific, the following queries are equivalent:
///
/// - `Query<All<&dyn Trait>>` has the same outcome as `Query<&dyn Trait>`
/// - `Query<All<&mut dyn Trait>>` has the same outcome as `Query<&mut dyn Trait>`
///
/// Depending on whether you requested shared or exclusive access to the trait objects, iterating
/// over these queries yields types with different capacities
///
/// - `Query<&dyn Trait>` yields a [`ReadTraits`] object
/// - `Query<&mut dyn Trait>` yields a [`WriteTraits`] object
pub struct All<T: ?Sized>(T);

unsafe impl<Trait: ?Sized + TraitQuery> QueryData for All<&Trait> {
    type ReadOnly = Self;
}
unsafe impl<Trait: ?Sized + TraitQuery> ReadOnlyQueryData for All<&Trait> {}

// SAFETY: We only access the components registered in the trait registry.
// This is known to match the set of components in the TraitQueryState,
// which is used to match archetypes and register world access.
unsafe impl<Trait: ?Sized + TraitQuery> WorldQuery for All<&Trait> {
    type Item<'w> = ReadTraits<'w, Trait>;
    type Fetch<'w> = AllTraitsFetch<'w, Trait>;
    type State = TraitQueryState<Trait>;

    #[inline]
    fn shrink<'wlong: 'wshort, 'wshort>(item: QueryItem<'wlong, Self>) -> QueryItem<'wshort, Self> {
        item
    }

    #[inline]
    unsafe fn init_fetch<'w>(
        world: UnsafeWorldCell<'w>,
        _state: &Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        AllTraitsFetch {
            registry: world
                .get_resource()
                .unwrap_or_else(|| trait_registry_error()),
            table: None,
            sparse_sets: &world.storages().sparse_sets,
            last_run,
            this_run,
        }
    }

    const IS_DENSE: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut Self::Fetch<'w>,
        _state: &Self::State,
        _archetype: &'w bevy_ecs::archetype::Archetype,
        table: &'w bevy_ecs::storage::Table,
    ) {
        fetch.table = Some(table);
    }

    unsafe fn set_table<'w>(
        fetch: &mut Self::Fetch<'w>,
        _state: &Self::State,
        table: &'w bevy_ecs::storage::Table,
    ) {
        fetch.table = Some(table);
    }

    #[inline]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        _entity: Entity,
        table_row: TableRow,
    ) -> Self::Item<'w> {
        let table = fetch.table.unwrap_or_else(|| debug_unreachable());

        ReadTraits {
            registry: fetch.registry,
            table,
            table_row,
            sparse_sets: fetch.sparse_sets,
            last_run: fetch.last_run,
            this_run: fetch.this_run,
        }
    }

    #[inline]
    fn update_component_access(
        state: &Self::State,
        access: &mut bevy_ecs::query::FilteredAccess<ComponentId>,
    ) {
        let mut not_first = false;
        let mut new_access = access.clone();
        for &component in &*state.components {
            assert!(
                !access.access().has_component_write(component),
                "&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                std::any::type_name::<Trait>(),
            );
            if not_first {
                let mut intermediate = access.clone();
                intermediate.add_component_read(component);
                new_access.append_or(&intermediate);
                new_access.extend_access(&intermediate);
            } else {
                new_access.and_with(component);
                new_access.access_mut().add_component_read(component);
                not_first = true;
            }
        }
        *access = new_access;
    }

    #[inline]
    fn init_state(world: &mut World) -> Self::State {
        TraitQueryState::init(world)
    }

    #[inline]
    fn get_state(_: &Components) -> Option<Self::State> {
        // TODO: fix this https://github.com/bevyengine/bevy/issues/13798
        panic!("transmuting and any other operations concerning the state of a query are currently broken and shouldn't be used. See https://github.com/JoJoJet/bevy-trait-query/issues/59");
    }

    #[inline]
    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        state.matches_component_set_any(set_contains_id)
    }

    #[inline]
    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }
}

unsafe impl<'a, Trait: ?Sized + TraitQuery> QueryData for All<&'a mut Trait> {
    type ReadOnly = All<&'a Trait>;
}

// SAFETY: We only access the components registered in the trait registry.
// This is known to match the set of components in the TraitQueryState,
// which is used to match archetypes and register world access.
unsafe impl<Trait: ?Sized + TraitQuery> WorldQuery for All<&mut Trait> {
    type Item<'w> = WriteTraits<'w, Trait>;
    type Fetch<'w> = AllTraitsFetch<'w, Trait>;
    type State = TraitQueryState<Trait>;

    #[inline]
    fn shrink<'wlong: 'wshort, 'wshort>(item: QueryItem<'wlong, Self>) -> QueryItem<'wshort, Self> {
        item
    }

    #[inline]
    unsafe fn init_fetch<'w>(
        world: UnsafeWorldCell<'w>,
        _state: &Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        AllTraitsFetch {
            registry: world
                .get_resource()
                .unwrap_or_else(|| trait_registry_error()),
            table: None,
            sparse_sets: &world.storages().sparse_sets,
            last_run,
            this_run,
        }
    }

    const IS_DENSE: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut Self::Fetch<'w>,
        _state: &Self::State,
        _archetype: &'w bevy_ecs::archetype::Archetype,
        table: &'w bevy_ecs::storage::Table,
    ) {
        fetch.table = Some(table);
    }

    #[inline]
    unsafe fn set_table<'w>(
        fetch: &mut Self::Fetch<'w>,
        _state: &Self::State,
        table: &'w bevy_ecs::storage::Table,
    ) {
        fetch.table = Some(table);
    }

    #[inline]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        _entity: Entity,
        table_row: TableRow,
    ) -> Self::Item<'w> {
        let table = fetch.table.unwrap_or_else(|| debug_unreachable());

        WriteTraits {
            registry: fetch.registry,
            table,
            table_row,
            sparse_sets: fetch.sparse_sets,
            last_run: fetch.last_run,
            this_run: fetch.this_run,
        }
    }

    #[inline]
    fn update_component_access(
        state: &Self::State,
        access: &mut bevy_ecs::query::FilteredAccess<ComponentId>,
    ) {
        let mut not_first = false;
        let mut new_access = access.clone();
        for &component in &*state.components {
            assert!(
                !access.access().has_component_write(component),
                "&mut {} conflicts with a previous access in this query. Mutable component access must be unique.",
                std::any::type_name::<Trait>(),
            );
            if not_first {
                let mut intermediate = access.clone();
                intermediate.add_component_write(component);
                new_access.append_or(&intermediate);
                new_access.extend_access(&intermediate);
            } else {
                new_access.and_with(component);
                new_access.access_mut().add_component_write(component);
                not_first = true;
            }
        }
        *access = new_access;
    }

    #[inline]
    fn init_state(world: &mut World) -> Self::State {
        TraitQueryState::init(world)
    }

    #[inline]
    fn get_state(_: &Components) -> Option<Self::State> {
        // TODO: fix this https://github.com/bevyengine/bevy/issues/13798
        panic!("transmuting and any other operations concerning the state of a query are currently broken and shouldn't be used. See https://github.com/JoJoJet/bevy-trait-query/issues/59");
    }

    #[inline]
    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        state.matches_component_set_any(set_contains_id)
    }

    #[inline]
    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }
}
