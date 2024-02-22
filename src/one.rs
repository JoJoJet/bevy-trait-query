use std::{cell::UnsafeCell, marker::PhantomData};

use bevy_ecs::{
    archetype::Archetype,
    change_detection::{Mut, Ref},
    component::{ComponentId, Tick},
    entity::Entity,
    ptr::{Ptr, ThinSlicePtr, UnsafeCellDeref},
    query::{FilteredAccess, QueryData, QueryFilter, QueryItem, ReadOnlyQueryData, WorldQuery},
    storage::{ComponentSparseSet, SparseSets, Table, TableRow},
    world::{unsafe_world_cell::UnsafeWorldCell, World},
};

use crate::{debug_unreachable, zip_exact, TraitImplMeta, TraitQuery, TraitQueryState};

pub struct OneTraitFetch<'w, Trait: ?Sized> {
    // While we have shared access to all sparse set components,
    // in practice we will only access the components specified in the `FetchState`.
    // These accesses have been registered, which prevents runtime conflicts.
    sparse_sets: &'w SparseSets,
    // After `Fetch::set_archetype` or `set_table` has been called,
    // this will carry the component data and metadata for the first trait impl found in the archetype.
    storage: FetchStorage<'w, Trait>,
    last_run: Tick,
    this_run: Tick,
}

impl<Trait: ?Sized> Clone for OneTraitFetch<'_, Trait> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Trait: ?Sized> Copy for OneTraitFetch<'_, Trait> {}

enum FetchStorage<'w, Trait: ?Sized> {
    Uninit,
    Table {
        /// This points to one of the component table columns,
        /// corresponding to one of the `ComponentId`s in the fetch state.
        /// The fetch impl registers access for all of these components,
        /// so there will be no runtime conflicts.
        column: Ptr<'w>,
        added_ticks: ThinSlicePtr<'w, UnsafeCell<Tick>>,
        changed_ticks: ThinSlicePtr<'w, UnsafeCell<Tick>>,
        meta: TraitImplMeta<Trait>,
    },
    SparseSet {
        /// This gives us access to one of the components implementing the trait.
        /// The fetch impl registers access for all components implementing the trait,
        /// so there will not be any runtime conflicts.
        components: &'w ComponentSparseSet,
        meta: TraitImplMeta<Trait>,
    },
}

impl<Trait: ?Sized> Clone for FetchStorage<'_, Trait> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<Trait: ?Sized> Copy for FetchStorage<'_, Trait> {}

/// [`WorldQuery`] adapter that fetches entities with exactly one component implementing a trait.
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
        for (&component, &meta) in zip_exact(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                fetch.storage = FetchStorage::Table {
                    column: column.get_data_ptr(),
                    added_ticks: column.get_added_ticks_slice().into(),
                    changed_ticks: column.get_changed_ticks_slice().into(),
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
        for (&component, &meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                fetch.storage = FetchStorage::Table {
                    column: column.get_data_ptr(),
                    added_ticks: column.get_added_ticks_slice().into(),
                    changed_ticks: column.get_changed_ticks_slice().into(),
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
                let (ptr, ticks) = components
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
                !access.access().has_write(component),
                "&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                std::any::type_name::<Trait>(),
            );
            if not_first {
                let mut intermediate = access.clone();
                intermediate.add_read(component);
                new_access.append_or(&intermediate);
                new_access.extend_access(&intermediate);
            } else {
                new_access.add_read(component);
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
    fn get_state(world: &World) -> Option<Self::State> {
        TraitQueryState::get(world)
    }

    #[inline]
    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        state.matches_component_set_one(set_contains_id)
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
        for (&component, &meta) in zip_exact(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                fetch.storage = FetchStorage::Table {
                    column: column.get_data_ptr(),
                    added_ticks: column.get_added_ticks_slice().into(),
                    changed_ticks: column.get_changed_ticks_slice().into(),
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
        for (&component, &meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                fetch.storage = FetchStorage::Table {
                    column: column.get_data_ptr(),
                    added_ticks: column.get_added_ticks_slice().into(),
                    changed_ticks: column.get_changed_ticks_slice().into(),
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
                let (ptr, ticks) = components
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
                !access.access().has_write(component),
                "&mut {} conflicts with a previous access in this query. Mutable component access must be unique.",
                std::any::type_name::<Trait>(),
            );
            if not_first {
                let mut intermediate = access.clone();
                intermediate.add_write(component);
                new_access.append_or(&intermediate);
                new_access.extend_access(&intermediate);
            } else {
                new_access.add_write(component);
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
    fn get_state(world: &World) -> Option<Self::State> {
        TraitQueryState::get(world)
    }

    #[inline]
    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        state.matches_component_set_one(set_contains_id)
    }
}

#[derive(Clone, Copy)]
enum ChangeDetectionStorage<'w> {
    Uninit,
    Table {
        /// This points to one of the component table columns,
        /// corresponding to one of the `ComponentId`s in the fetch state.
        /// The fetch impl registers read access for all of these components,
        /// so there will be no runtime conflicts.
        ticks: ThinSlicePtr<'w, UnsafeCell<Tick>>,
    },
    SparseSet {
        /// This gives us access to one of the components implementing the trait.
        /// The fetch impl registers read access for all components implementing the trait,
        /// so there will not be any runtime conflicts.
        components: &'w ComponentSparseSet,
    },
}

/// [`WorldQuery`] filter for entities with exactly [one](crate::One) component
/// implementing a trait, whose value has changed since the last time the system ran.
pub struct OneAdded<Trait: ?Sized + TraitQuery> {
    marker: PhantomData<&'static Trait>,
}

#[derive(Clone, Copy)]
pub struct ChangeDetectionFetch<'w> {
    storage: ChangeDetectionStorage<'w>,
    sparse_sets: &'w SparseSets,
    last_run: Tick,
    this_run: Tick,
}

unsafe impl<Trait: ?Sized + TraitQuery> WorldQuery for OneAdded<Trait> {
    type Item<'w> = bool;
    type Fetch<'w> = ChangeDetectionFetch<'w>;
    type State = TraitQueryState<Trait>;

    fn shrink<'wlong: 'wshort, 'wshort>(item: Self::Item<'wlong>) -> Self::Item<'wshort> {
        item
    }

    unsafe fn init_fetch<'w>(
        world: UnsafeWorldCell<'w>,
        _state: &Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        Self::Fetch::<'w> {
            storage: ChangeDetectionStorage::Uninit,
            sparse_sets: &world.storages().sparse_sets,
            last_run,
            this_run,
        }
    }

    // This will always be false for us, as we (so far) do not know at compile time whether the
    // components our trait has been impl'd for are stored in table or in sparse set
    const IS_DENSE: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut Self::Fetch<'w>,
        state: &Self::State,
        _archetype: &'w Archetype,
        table: &'w Table,
    ) {
        // Search for a registered trait impl that is present in the archetype.
        // We check the table components first since it is faster to retrieve data of this type.
        for &component in &*state.components {
            if let Some(column) = table.get_column(component) {
                fetch.storage = ChangeDetectionStorage::Table {
                    ticks: ThinSlicePtr::from(column.get_added_ticks_slice()),
                };
                return;
            }
        }
        for &component in &*state.components {
            if let Some(components) = fetch.sparse_sets.get(component) {
                fetch.storage = ChangeDetectionStorage::SparseSet { components };
                return;
            }
        }
        // At least one of the components must be present in the table/sparse set.
        debug_unreachable()
    }

    #[inline]
    unsafe fn set_table<'w>(_fetch: &mut Self::Fetch<'w>, _state: &Self::State, _table: &'w Table) {
        // only gets called if IS_DENSE == true, which does not hold for us
        debug_unreachable()
    }

    #[inline(always)]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Self::Item<'w> {
        let ticks_ptr = match fetch.storage {
            ChangeDetectionStorage::Uninit => {
                // set_archetype must have been called already
                debug_unreachable()
            }
            ChangeDetectionStorage::Table { ticks } => ticks.get(table_row.as_usize()),
            ChangeDetectionStorage::SparseSet { components } => components
                .get_added_tick(entity)
                .unwrap_or_else(|| debug_unreachable()),
        };

        (*ticks_ptr)
            .deref()
            .is_newer_than(fetch.last_run, fetch.this_run)
    }

    #[inline]
    fn update_component_access(state: &Self::State, access: &mut FilteredAccess<ComponentId>) {
        let mut new_access = access.clone();
        let mut not_first = false;
        for &component in &*state.components {
            assert!(
                !access.access().has_write(component),
                "&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                std::any::type_name::<Trait>(),
            );
            if not_first {
                let mut intermediate = access.clone();
                intermediate.add_read(component);
                new_access.append_or(&intermediate);
                new_access.extend_access(&intermediate);
            } else {
                new_access.add_read(component);
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
    fn get_state(world: &World) -> Option<Self::State> {
        TraitQueryState::get(world)
    }

    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        state.matches_component_set_one(set_contains_id)
    }
}

unsafe impl<Trait: ?Sized + TraitQuery> QueryData for OneAdded<Trait> {
    type ReadOnly = Self;
}
/// SAFETY: read-only access
unsafe impl<Trait: ?Sized + TraitQuery> ReadOnlyQueryData for OneAdded<Trait> {}
impl<Trait: ?Sized + TraitQuery> QueryFilter for OneAdded<Trait> {
    const IS_ARCHETYPAL: bool = false;
    unsafe fn filter_fetch(
        fetch: &mut Self::Fetch<'_>,
        entity: Entity,
        table_row: TableRow,
    ) -> bool {
        <Self as WorldQuery>::fetch(fetch, entity, table_row)
    }
}

/// [`WorldQuery`] filter for entities with exactly [one](crate::One) component
/// implementing a trait, which was added since the last time the system ran.
pub struct OneChanged<Trait: ?Sized + TraitQuery> {
    marker: PhantomData<&'static Trait>,
}

unsafe impl<Trait: ?Sized + TraitQuery> WorldQuery for OneChanged<Trait> {
    type Item<'w> = bool;
    type Fetch<'w> = ChangeDetectionFetch<'w>;
    type State = TraitQueryState<Trait>;

    fn shrink<'wlong: 'wshort, 'wshort>(item: Self::Item<'wlong>) -> Self::Item<'wshort> {
        item
    }

    unsafe fn init_fetch<'w>(
        world: UnsafeWorldCell<'w>,
        _state: &Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        Self::Fetch::<'w> {
            storage: ChangeDetectionStorage::Uninit,
            sparse_sets: &world.storages().sparse_sets,
            last_run,
            this_run,
        }
    }

    // This will always be false for us, as we (so far) do not know at compile time whether the
    // components our trait has been impl'd for are stored in table or in sparse set
    const IS_DENSE: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut Self::Fetch<'w>,
        state: &Self::State,
        _archetype: &'w Archetype,
        table: &'w Table,
    ) {
        // Search for a registered trait impl that is present in the archetype.
        // We check the table components first since it is faster to retrieve data of this type.
        for &component in &*state.components {
            if let Some(column) = table.get_column(component) {
                fetch.storage = ChangeDetectionStorage::Table {
                    ticks: column.get_changed_ticks_slice().into(),
                };
                return;
            }
        }
        for &component in &*state.components {
            if let Some(components) = fetch.sparse_sets.get(component) {
                fetch.storage = ChangeDetectionStorage::SparseSet { components };
                return;
            }
        }
        // At least one of the components must be present in the table/sparse set.
        debug_unreachable()
    }

    #[inline]
    unsafe fn set_table<'w>(_fetch: &mut Self::Fetch<'w>, _state: &Self::State, _table: &'w Table) {
        // only gets called if IS_DENSE == true, which does not hold for us
        debug_unreachable()
    }

    #[inline(always)]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Self::Item<'w> {
        let ticks_ptr = match fetch.storage {
            ChangeDetectionStorage::Uninit => {
                // set_archetype must have been called already
                debug_unreachable()
            }
            ChangeDetectionStorage::Table { ticks } => ticks.get(table_row.as_usize()),
            ChangeDetectionStorage::SparseSet { components } => components
                .get_changed_tick(entity)
                .unwrap_or_else(|| debug_unreachable()),
        };

        (*ticks_ptr)
            .deref()
            .is_newer_than(fetch.last_run, fetch.this_run)
    }

    #[inline]
    fn update_component_access(state: &Self::State, access: &mut FilteredAccess<ComponentId>) {
        let mut new_access = access.clone();
        let mut not_first = false;
        for &component in &*state.components {
            assert!(
                !access.access().has_write(component),
                "&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                std::any::type_name::<Trait>(),
            );
            if not_first {
                let mut intermediate = access.clone();
                intermediate.add_read(component);
                new_access.append_or(&intermediate);
                new_access.extend_access(&intermediate);
            } else {
                new_access.add_read(component);
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
    fn get_state(world: &World) -> Option<Self::State> {
        TraitQueryState::get(world)
    }

    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        state.matches_component_set_one(set_contains_id)
    }
}

/// SAFETY: read-only access
unsafe impl<Trait: ?Sized + TraitQuery> QueryData for OneChanged<Trait> {
    type ReadOnly = Self;
}
unsafe impl<Trait: ?Sized + TraitQuery> ReadOnlyQueryData for OneChanged<Trait> {}
impl<Trait: ?Sized + TraitQuery> QueryFilter for OneChanged<Trait> {
    const IS_ARCHETYPAL: bool = false;
    unsafe fn filter_fetch(
        fetch: &mut Self::Fetch<'_>,
        entity: Entity,
        table_row: TableRow,
    ) -> bool {
        <Self as WorldQuery>::fetch(fetch, entity, table_row)
    }
}
