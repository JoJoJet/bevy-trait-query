use bevy_ecs::change_detection::{Mut, Ref};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use bevy_ecs::ptr::UnsafeCellDeref;
use bevy_ecs::{
    component::{ComponentId, Components, Tick},
    query::{QueryData, QueryItem, ReadOnlyQueryData, WorldQuery},
    storage::TableRow,
    world::unsafe_world_cell::UnsafeWorldCell,
};

use crate::{
    debug_unreachable, one::FetchStorage, zip_exact, OneTraitFetch, TraitQuery, TraitQueryState,
};

/// [`WorldQuery`] adapter that fetches entities with exactly one component implementing a trait.
///
/// Depending on whether you requested shared or exclusive access to the trait objects, iterating
/// over these queries yields types with different capacities
///
/// - `Query<One<&dyn Trait>>` yields a [`Ref`] object
/// - `Query<One<&mut dyn Trait>>` yields a [`Mut`] object
pub struct One<T>(pub T);

unsafe impl<'a, T: ?Sized + TraitQuery> QueryData for One<&'a T> {
    type ReadOnly = Self;
}
unsafe impl<'a, T: ?Sized + TraitQuery> ReadOnlyQueryData for One<&'a T> {}

unsafe impl<'a, T: ?Sized + TraitQuery> QueryData for One<&'a mut T> {
    type ReadOnly = One<&'a T>;
}

// SAFETY: We only access the components registered in TraitQueryState.
// This same set of components is used to match archetypes, and used to register world access.
unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for One<&'a Trait> {
    type Item<'w> = Ref<'w, Trait>;
    type Fetch<'w> = OneTraitFetch<'w, Trait>;
    type State = TraitQueryState<Trait>;

    #[inline]
    fn shrink<'wlong: 'wshort, 'wshort>(item: QueryItem<'wlong, Self>) -> QueryItem<'wshort, Self> {
        item
    }

    #[inline]
    unsafe fn init_fetch<'w>(
        world: UnsafeWorldCell<'w>,
        _state: &Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> OneTraitFetch<'w, Trait> {
        OneTraitFetch {
            storage: FetchStorage::Uninit,
            last_run: Tick::new(0),
            sparse_sets: &world.storages().sparse_sets,
            this_run: Tick::new(0),
        }
    }

    const IS_DENSE: bool = false;
    // const IS_ARCHETYPAL: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut OneTraitFetch<'w, Trait>,
        state: &Self::State,
        _archetype: &'w bevy_ecs::archetype::Archetype,
        table: &'w bevy_ecs::storage::Table,
    ) {
        // Search for a registered trait impl that is present in the archetype.
        // We check the table components first since it is faster to retrieve data of this type.
        //
        // without loss of generality we use the zero-th row since we only care about whether the
        // component exists in the table
        let row = TableRow::from_usize(0);
        for (&component, &meta) in zip_exact(&*state.components, &*state.meta) {
            if let Some((ptr, added, changed)) =
                table.get_component(component, row).and_then(|ptr| {
                    let added = table.get_added_ticks_slice_for(component)?;
                    let changed = table.get_changed_ticks_slice_for(component)?;
                    Some((ptr, added, changed))
                })
            {
                fetch.storage = FetchStorage::Table {
                    column: ptr,
                    added_ticks: added.into(),
                    changed_ticks: changed.into(),
                    meta,
                };
                return;
            }
        }
        for (&component, &meta) in zip_exact(&*state.components, &*state.meta) {
            if let Some(sparse_set) = fetch.sparse_sets.get(component) {
                fetch.storage = FetchStorage::SparseSet {
                    components: sparse_set,
                    meta,
                };
                return;
            }
        }
        // At least one of the components must be present in the table/sparse set.
        debug_unreachable()
    }

    #[inline]
    unsafe fn set_table<'w>(
        fetch: &mut OneTraitFetch<'w, Trait>,
        state: &Self::State,
        table: &'w bevy_ecs::storage::Table,
    ) {
        // Search for a registered trait impl that is present in the table.
        //
        // without loss of generality we use the zero-th row since we only care about whether the
        // component exists in the table
        let row = TableRow::from_usize(0);
        for (&component, &meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some((ptr, added, changed)) =
                table.get_component(component, row).and_then(|ptr| {
                    let added = table.get_added_ticks_slice_for(component)?;
                    let changed = table.get_changed_ticks_slice_for(component)?;
                    Some((ptr, added, changed))
                })
            {
                fetch.storage = FetchStorage::Table {
                    column: ptr,
                    added_ticks: added.into(),
                    changed_ticks: changed.into(),
                    meta,
                }
            }
        }
        // At least one of the components must be present in the table.
        debug_unreachable()
    }

    #[inline]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Self::Item<'w> {
        let table_row = table_row.as_usize();
        let dyn_ctor;
        let (ptr, added, changed) = match fetch.storage {
            // SAFETY: This function must have been called after `set_archetype`,
            // so we know that `self.storage` has been initialized.
            FetchStorage::Uninit => debug_unreachable(),
            FetchStorage::Table {
                column,
                added_ticks,
                changed_ticks,
                meta,
            } => {
                dyn_ctor = meta.dyn_ctor;
                let ptr = column.byte_add(table_row * meta.size_bytes);
                (
                    ptr,
                    // SAFETY: We have read access to the component, so by extension
                    // we have access to the corresponding `ComponentTicks`.
                    added_ticks.get(table_row).deref(),
                    changed_ticks.get(table_row).deref(),
                )
            }
            FetchStorage::SparseSet { components, meta } => {
                dyn_ctor = meta.dyn_ctor;
                let (ptr, ticks, _) = components
                    .get_with_ticks(entity)
                    .unwrap_or_else(|| debug_unreachable());
                (
                    ptr,
                    // SAFETY: We have read access to the component, so by extension
                    // we have access to the corresponding `ComponentTicks`.
                    ticks.added.deref(),
                    ticks.changed.deref(),
                )
            }
        };

        Ref::new(
            dyn_ctor.cast(ptr),
            added,
            changed,
            fetch.last_run,
            fetch.this_run,
        )
    }

    #[inline]
    fn update_component_access(
        state: &Self::State,
        access: &mut bevy_ecs::query::FilteredAccess<ComponentId>,
    ) {
        let mut new_access = access.clone();
        let mut not_first = false;
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
        state.matches_component_set_one(set_contains_id)
    }

    #[inline]
    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }
}

// SAFETY: We only access the components registered in TraitQueryState.
// This same set of components is used to match archetypes, and used to register world access.
unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for One<&'a mut Trait> {
    type Item<'w> = Mut<'w, Trait>;
    type Fetch<'w> = OneTraitFetch<'w, Trait>;
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
    ) -> OneTraitFetch<'w, Trait> {
        OneTraitFetch {
            storage: FetchStorage::Uninit,
            sparse_sets: &world.storages().sparse_sets,
            last_run,
            this_run,
        }
    }

    const IS_DENSE: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut OneTraitFetch<'w, Trait>,
        state: &Self::State,
        _archetype: &'w bevy_ecs::archetype::Archetype,
        table: &'w bevy_ecs::storage::Table,
    ) {
        // Search for a registered trait impl that is present in the archetype.
        //
        // without loss of generality we use the zero-th row since we only care about whether the
        // component exists in the table
        let row = TableRow::from_usize(0);
        for (&component, &meta) in zip_exact(&*state.components, &*state.meta) {
            if let Some((ptr, added, changed)) =
                table.get_component(component, row).and_then(|ptr| {
                    let added = table.get_added_ticks_slice_for(component)?;
                    let changed = table.get_changed_ticks_slice_for(component)?;
                    Some((ptr, added, changed))
                })
            {
                fetch.storage = FetchStorage::Table {
                    column: ptr,
                    added_ticks: added.into(),
                    changed_ticks: changed.into(),
                    meta,
                };
                return;
            }
        }
        for (&component, &meta) in zip_exact(&*state.components, &*state.meta) {
            if let Some(sparse_set) = fetch.sparse_sets.get(component) {
                fetch.storage = FetchStorage::SparseSet {
                    components: sparse_set,
                    meta,
                };
                return;
            }
        }
        // At least one of the components must be present in the table/sparse set.
        debug_unreachable()
    }

    #[inline]
    unsafe fn set_table<'w>(
        fetch: &mut OneTraitFetch<'w, Trait>,
        state: &Self::State,
        table: &'w bevy_ecs::storage::Table,
    ) {
        // Search for a registered trait impl that is present in the table.
        //
        // without loss of generality we use the zero-th row since we only care about whether the
        // component exists in the table
        let row = TableRow::from_usize(0);
        for (&component, &meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some((ptr, added, changed)) =
                table.get_component(component, row).and_then(|ptr| {
                    let added = table.get_added_ticks_slice_for(component)?;
                    let changed = table.get_changed_ticks_slice_for(component)?;
                    Some((ptr, added, changed))
                })
            {
                fetch.storage = FetchStorage::Table {
                    column: ptr,
                    added_ticks: added.into(),
                    changed_ticks: changed.into(),
                    meta,
                };
                return;
            }
        }
        // At least one of the components must be present in the table.
        debug_unreachable()
    }

    #[inline]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Mut<'w, Trait> {
        let table_row = table_row.as_usize();
        let dyn_ctor;
        let (ptr, added, changed) = match fetch.storage {
            // SAFETY: This function must have been called after `set_archetype`,
            // so we know that `self.storage` has been initialized.
            FetchStorage::Uninit => debug_unreachable(),
            FetchStorage::Table {
                column,
                added_ticks,
                changed_ticks,
                meta,
            } => {
                dyn_ctor = meta.dyn_ctor;
                let ptr = column.byte_add(table_row * meta.size_bytes);
                (
                    // SAFETY: `column` allows for shared mutable access.
                    // So long as the caller does not invoke this function twice with the same archetype_index,
                    // this pointer will never be aliased.
                    ptr.assert_unique(),
                    // SAFETY: We have exclusive access to the component, so by extension
                    // we have exclusive access to the corresponding `ComponentTicks`.
                    added_ticks.get(table_row).deref_mut(),
                    changed_ticks.get(table_row).deref_mut(),
                )
            }
            FetchStorage::SparseSet { components, meta } => {
                dyn_ctor = meta.dyn_ctor;
                let (ptr, ticks, _) = components
                    .get_with_ticks(entity)
                    .unwrap_or_else(|| debug_unreachable());
                (
                    // SAFETY: We have exclusive access to the sparse set `components`.
                    // So long as the caller does not invoke this function twice with the same archetype_index,
                    // this pointer will never be aliased.
                    ptr.assert_unique(),
                    // SAFETY: We have exclusive access to the component, so by extension
                    // we have exclusive access to the corresponding `ComponentTicks`.
                    ticks.added.deref_mut(),
                    ticks.changed.deref_mut(),
                )
            }
        };

        Mut::new(
            dyn_ctor.cast_mut(ptr),
            added,
            changed,
            fetch.last_run,
            fetch.this_run,
        )
    }

    #[inline]
    fn update_component_access(
        state: &Self::State,
        access: &mut bevy_ecs::query::FilteredAccess<ComponentId>,
    ) {
        let mut new_access = access.clone();
        let mut not_first = false;
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
        state.matches_component_set_one(set_contains_id)
    }

    #[inline]
    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }
}
