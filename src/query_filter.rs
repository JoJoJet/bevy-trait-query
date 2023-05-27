use crate::{debug_unreachable, TraitImplRegistry, TraitQuery, TraitQueryState};
use bevy::ecs::archetype::{Archetype, ArchetypeComponentId};
use bevy::ecs::component::{ComponentId, Tick};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Access, FilteredAccess, ReadOnlyWorldQuery, WorldQuery};
use bevy::ecs::storage::{ComponentSparseSet, SparseSets, Table, TableRow};
use bevy::ecs::world::World;
use bevy::ptr::{ThinSlicePtr, UnsafeCellDeref};
use std::cell::UnsafeCell;
use std::marker::PhantomData;

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

pub struct OneAddedFilter<'a, Trait: ?Sized + TraitQuery> {
    marker: PhantomData<&'a Trait>,
}

pub struct ChangeDetectionFetch<'w, Trait: ?Sized + TraitQuery> {
    registry: &'w TraitImplRegistry<Trait>,
    storage: ChangeDetectionStorage<'w>,
    sparse_sets: &'w SparseSets,
    last_run: Tick,
    this_run: Tick,
}

unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for OneAddedFilter<'a, Trait> {
    type Item<'w> = bool;
    type Fetch<'w> = ChangeDetectionFetch<'w, Trait>;
    type ReadOnly = Self;
    type State = TraitQueryState<Trait>;

    fn shrink<'wlong: 'wshort, 'wshort>(item: Self::Item<'wlong>) -> Self::Item<'wshort> {
        item
    }

    unsafe fn init_fetch<'w>(
        world: &'w World,
        _state: &Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        Self::Fetch::<'w> {
            registry: world.resource(),
            storage: ChangeDetectionStorage::Uninit,
            sparse_sets: &world.storages().sparse_sets,
            last_run,
            this_run,
        }
    }

    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        Self::Fetch {
            registry: fetch.registry,
            storage: match fetch.storage {
                ChangeDetectionStorage::Uninit => ChangeDetectionStorage::Uninit,
                ChangeDetectionStorage::Table { ticks } => ChangeDetectionStorage::Table { ticks },
                ChangeDetectionStorage::SparseSet { components } => {
                    ChangeDetectionStorage::SparseSet { components }
                }
            },
            sparse_sets: fetch.sparse_sets,
            last_run: fetch.last_run,
            this_run: fetch.this_run,
        }
    }

    // This will always be false for us, as we (so far) do not know at compile time whether the
    // components our trait has been impl'd for are stored in table or in sparse set
    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = false;

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
            ChangeDetectionStorage::Table { ticks } => ticks.get(table_row.index()),
            ChangeDetectionStorage::SparseSet { components } => components
                .get_added_ticks(entity)
                .unwrap_or_else(|| debug_unreachable()),
        };

        (*ticks_ptr)
            .deref()
            .is_newer_than(fetch.last_run, fetch.this_run)
    }

    #[inline(always)]
    unsafe fn filter_fetch(
        fetch: &mut Self::Fetch<'_>,
        entity: Entity,
        table_row: TableRow,
    ) -> bool {
        Self::fetch(fetch, entity, table_row)
    }

    #[inline]
    fn update_component_access(state: &Self::State, access: &mut FilteredAccess<ComponentId>) {
        for &component in &*state.components {
            assert!(
                !access.access().has_write(component),
                "&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                std::any::type_name::<Trait>(),
            );
            access.add_read(component);
        }
    }

    #[inline]
    fn update_archetype_component_access(
        state: &Self::State,
        archetype: &Archetype,
        access: &mut Access<ArchetypeComponentId>,
    ) {
        for &component in &*state.components {
            if let Some(archetype_component_id) = archetype.get_archetype_component_id(component) {
                access.add_read(archetype_component_id);
            }
        }
    }

    fn init_state(world: &mut World) -> Self::State {
        TraitQueryState::init(world)
    }

    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        state.matches_component_set_one(set_contains_id)
    }
}

/// SAFETY: read-only access
unsafe impl<'a, Trait: ?Sized + TraitQuery> ReadOnlyWorldQuery for OneAddedFilter<'a, Trait> {}

pub struct OneChangedFilter<'a, Trait: ?Sized + TraitQuery> {
    marker: PhantomData<&'a Trait>,
}

unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for OneChangedFilter<'a, Trait> {
    type Item<'w> = bool;
    type Fetch<'w> = ChangeDetectionFetch<'w, Trait>;
    type ReadOnly = Self;
    type State = TraitQueryState<Trait>;

    fn shrink<'wlong: 'wshort, 'wshort>(item: Self::Item<'wlong>) -> Self::Item<'wshort> {
        item
    }

    unsafe fn init_fetch<'w>(
        world: &'w World,
        _state: &Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        Self::Fetch::<'w> {
            registry: world.resource(),
            storage: ChangeDetectionStorage::Uninit,
            sparse_sets: &world.storages().sparse_sets,
            last_run,
            this_run,
        }
    }

    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        Self::Fetch {
            registry: fetch.registry,
            storage: match fetch.storage {
                ChangeDetectionStorage::Uninit => ChangeDetectionStorage::Uninit,
                ChangeDetectionStorage::Table { ticks } => ChangeDetectionStorage::Table { ticks },
                ChangeDetectionStorage::SparseSet { components } => {
                    ChangeDetectionStorage::SparseSet { components }
                }
            },
            sparse_sets: fetch.sparse_sets,
            last_run: fetch.last_run,
            this_run: fetch.this_run,
        }
    }

    // This will always be false for us, as we (so far) do not know at compile time whether the
    // components our trait has been impl'd for are stored in table or in sparse set
    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = false;

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
            ChangeDetectionStorage::Table { ticks } => ticks.get(table_row.index()),
            ChangeDetectionStorage::SparseSet { components } => components
                .get_changed_ticks(entity)
                .unwrap_or_else(|| debug_unreachable()),
        };

        (*ticks_ptr)
            .deref()
            .is_newer_than(fetch.last_run, fetch.this_run)
    }

    #[inline(always)]
    unsafe fn filter_fetch(
        fetch: &mut Self::Fetch<'_>,
        entity: Entity,
        table_row: TableRow,
    ) -> bool {
        Self::fetch(fetch, entity, table_row)
    }

    #[inline]
    fn update_component_access(state: &Self::State, access: &mut FilteredAccess<ComponentId>) {
        for &component in &*state.components {
            assert!(
                !access.access().has_write(component),
                "&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                std::any::type_name::<Trait>(),
            );
            access.add_read(component);
        }
    }

    #[inline]
    fn update_archetype_component_access(
        state: &Self::State,
        archetype: &Archetype,
        access: &mut Access<ArchetypeComponentId>,
    ) {
        for &component in &*state.components {
            if let Some(archetype_component_id) = archetype.get_archetype_component_id(component) {
                access.add_read(archetype_component_id);
            }
        }
    }

    fn init_state(world: &mut World) -> Self::State {
        TraitQueryState::init(world)
    }

    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        state.matches_component_set_one(set_contains_id)
    }
}

/// SAFETY: read-only access
unsafe impl<'a, Trait: ?Sized + TraitQuery> ReadOnlyWorldQuery for OneChangedFilter<'a, Trait> {}
