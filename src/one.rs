use crate::change_detection::{Mut, Ticks};
use crate::{debug_unreachable, zip_exact, TraitImplMeta, TraitQuery, TraitQueryState};
use bevy::ecs::component::{ComponentId, ComponentTicks, Tick};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{QueryItem, ReadOnlyWorldQuery, WorldQuery};
use bevy::ecs::storage::{ComponentSparseSet, SparseSets, TableRow};
use bevy::ecs::world::World;
use bevy::ptr::{Ptr, ThinSlicePtr, UnsafeCellDeref};
use std::cell::UnsafeCell;

pub struct ReadTraitFetch<'w, Trait: ?Sized> {
    // While we have shared access to all sparse set components,
    // in practice we will only read the components specified in the `FetchState`.
    // These accesses have been registered, which prevents runtime conflicts.
    sparse_sets: &'w SparseSets,
    // After `Fetch::set_archetype` or `set_table` has been called,
    // this will carry the component data and metadata for the first trait impl found in the archetype.
    storage: ReadStorage<'w, Trait>,
}

enum ReadStorage<'w, Trait: ?Sized> {
    Uninit,
    Table {
        /// This points to one of the component table columns,
        /// corresponding to one of the `ComponentId`s in the fetch state.
        /// The fetch impl registers read access for all of these components,
        /// so there will be no runtime conflicts.
        column: Ptr<'w>,
        ticks: Option<ThinSlicePtr<'w, UnsafeCell<ComponentTicks>>>,
        meta: TraitImplMeta<Trait>,
    },
    SparseSet {
        /// This gives us access to one of the components implementing the trait.
        /// The fetch impl registers read access for all components implementing the trait,
        /// so there will not be any runtime conflicts.
        components: &'w ComponentSparseSet,
        meta: TraitImplMeta<Trait>,
    },
}

#[doc(hidden)]
pub struct WriteTraitFetch<'w, Trait: ?Sized> {
    // While we have shared mutable access to all sparse set components,
    // in practice we will only modify the components specified in the `FetchState`.
    // These accesses have been registered, which prevents runtime conflicts.
    sparse_sets: &'w SparseSets,

    // After `Fetch::set_archetype` or `set_table` has been called,
    // this will carry the component data and metadata for the first trait impl found in the archetype.
    storage: WriteStorage<'w, Trait>,

    last_change_tick: u32,
    change_tick: u32,
}

enum WriteStorage<'w, Trait: ?Sized> {
    Uninit,
    Table {
        /// This is a shared mutable pointer to one of the component table columns,
        /// corresponding to one of the `ComponentId`s in the fetch state.
        /// The fetch impl registers write access for all of these components,
        /// so there will be no runtime conflicts.
        column: Ptr<'w>,
        added_ticks: ThinSlicePtr<'w, UnsafeCell<Tick>>,
        changed_ticks: ThinSlicePtr<'w, UnsafeCell<Tick>>,
        meta: TraitImplMeta<Trait>,
    },
    SparseSet {
        /// This gives us shared mutable access to one of the components implementing the trait.
        /// The fetch impl registers write access for all components implementing the trait, so there will be no runtime conflicts.
        components: &'w ComponentSparseSet,
        meta: TraitImplMeta<Trait>,
    },
}

/// [`WorldQuery`] adapter that fetches entities with exactly one component implementing a trait.
pub struct One<T>(pub T);

unsafe impl<'a, T: ?Sized + TraitQuery> ReadOnlyWorldQuery for One<&'a T> {}

/// SAFETY: We only access the components registered in `DynQueryState`.
/// This same set of components is used to match archetypes, and used to register world access.
unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for One<&'a Trait> {
    type Item<'w> = &'w Trait;
    type Fetch<'w> = ReadTraitFetch<'w, Trait>;
    type ReadOnly = Self;
    type State = TraitQueryState<Trait>;

    #[inline]
    fn shrink<'wlong: 'wshort, 'wshort>(item: QueryItem<'wlong, Self>) -> QueryItem<'wshort, Self> {
        item
    }

    #[inline]
    unsafe fn init_fetch<'w>(
        world: &'w World,
        _state: &Self::State,
        _last_change_tick: u32,
        _change_tick: u32,
    ) -> ReadTraitFetch<'w, Trait> {
        ReadTraitFetch {
            storage: ReadStorage::Uninit,
            sparse_sets: &world.storages().sparse_sets,
        }
    }

    #[inline]
    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        ReadTraitFetch {
            storage: match fetch.storage {
                ReadStorage::Uninit => ReadStorage::Uninit,
                ReadStorage::Table {
                    column,
                    ticks,
                    meta,
                } => ReadStorage::Table {
                    column,
                    ticks,
                    meta,
                },
                ReadStorage::SparseSet { components, meta } => {
                    ReadStorage::SparseSet { components, meta }
                }
            },
            sparse_sets: fetch.sparse_sets,
        }
    }

    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut ReadTraitFetch<'w, Trait>,
        state: &Self::State,
        _archetype: &'w bevy::ecs::archetype::Archetype,
        table: &'w bevy::ecs::storage::Table,
    ) {
        // Search for a registered trait impl that is present in the archetype.
        // We check the table components first since it is faster to retrieve data of this type.
        for (&component, &meta) in zip_exact(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                fetch.storage = ReadStorage::Table {
                    column: column.get_data_ptr(),
                    ticks: None,
                    meta,
                };
                return;
            }
        }
        for (&component, &meta) in zip_exact(&*state.components, &*state.meta) {
            if let Some(sparse_set) = fetch.sparse_sets.get(component) {
                fetch.storage = ReadStorage::SparseSet {
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
        fetch: &mut ReadTraitFetch<'w, Trait>,
        state: &Self::State,
        table: &'w bevy::ecs::storage::Table,
    ) {
        // Search for a registered trait impl that is present in the table.
        for (&component, &meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                fetch.storage = ReadStorage::Table {
                    column: column.get_data_ptr(),
                    ticks: None,
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
        let table_row = table_row.index();
        match fetch.storage {
            // SAFETY: This function must have been called after `set_archetype`,
            // so we know that `self.storage` has been initialized.
            ReadStorage::Uninit => debug_unreachable(),
            ReadStorage::Table { column, meta, .. } => {
                let ptr = column.byte_add(table_row * meta.size_bytes);
                meta.dyn_ctor.cast(ptr)
            }
            ReadStorage::SparseSet { components, meta } => {
                let ptr = components
                    .get(entity)
                    .unwrap_or_else(|| debug_unreachable());
                meta.dyn_ctor.cast(ptr)
            }
        }
    }

    #[inline]
    fn update_component_access(
        state: &Self::State,
        access: &mut bevy::ecs::query::FilteredAccess<ComponentId>,
    ) {
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
        archetype: &bevy::ecs::archetype::Archetype,
        access: &mut bevy::ecs::query::Access<bevy::ecs::archetype::ArchetypeComponentId>,
    ) {
        for &component in &*state.components {
            if let Some(archetype_component_id) = archetype.get_archetype_component_id(component) {
                access.add_read(archetype_component_id);
            }
        }
    }

    #[inline]
    fn init_state(world: &mut World) -> Self::State {
        TraitQueryState::init(world)
    }

    #[inline]
    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        state.matches_component_set_one(set_contains_id)
    }
}

/// SAFETY: We only access the components registered in `DynQueryState`.
/// This same set of components is used to match archetypes, and used to register world access.
unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for One<&'a mut Trait> {
    type Item<'w> = Mut<'w, Trait>;
    type Fetch<'w> = WriteTraitFetch<'w, Trait>;
    type ReadOnly = One<&'a Trait>;
    type State = TraitQueryState<Trait>;

    #[inline]
    fn shrink<'wlong: 'wshort, 'wshort>(item: QueryItem<'wlong, Self>) -> QueryItem<'wshort, Self> {
        item
    }

    #[inline]
    unsafe fn init_fetch<'w>(
        world: &'w World,
        _state: &Self::State,
        last_change_tick: u32,
        change_tick: u32,
    ) -> WriteTraitFetch<'w, Trait> {
        WriteTraitFetch {
            storage: WriteStorage::Uninit,
            sparse_sets: &world.storages().sparse_sets,
            last_change_tick,
            change_tick,
        }
    }

    #[inline]
    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        WriteTraitFetch {
            storage: match fetch.storage {
                WriteStorage::Uninit => WriteStorage::Uninit,
                WriteStorage::Table {
                    column,
                    meta,
                    added_ticks,
                    changed_ticks,
                } => WriteStorage::Table {
                    column,
                    meta,
                    added_ticks,
                    changed_ticks,
                },
                WriteStorage::SparseSet { components, meta } => {
                    WriteStorage::SparseSet { components, meta }
                }
            },
            sparse_sets: fetch.sparse_sets,
            last_change_tick: fetch.last_change_tick,
            change_tick: fetch.change_tick,
        }
    }

    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut WriteTraitFetch<'w, Trait>,
        state: &Self::State,
        _archetype: &'w bevy::ecs::archetype::Archetype,
        table: &'w bevy::ecs::storage::Table,
    ) {
        // Search for a registered trait impl that is present in the archetype.
        for (&component, &meta) in zip_exact(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                fetch.storage = WriteStorage::Table {
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
                fetch.storage = WriteStorage::SparseSet {
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
        fetch: &mut WriteTraitFetch<'w, Trait>,
        state: &Self::State,
        table: &'w bevy::ecs::storage::Table,
    ) {
        // Search for a registered trait impl that is present in the table.
        for (&component, &meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                fetch.storage = WriteStorage::Table {
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
        let table_row = table_row.index();
        let dyn_ctor;
        let (ptr, added, changed) = match fetch.storage {
            // SAFETY: This function must have been called after `set_archetype`,
            // so we know that `self.storage` has been initialized.
            WriteStorage::Uninit => debug_unreachable(),
            WriteStorage::Table {
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
            WriteStorage::SparseSet { components, meta } => {
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

        Mut {
            value: dyn_ctor.cast_mut(ptr),
            ticks: Ticks {
                added,
                changed,
                last_change_tick: fetch.last_change_tick,
                change_tick: fetch.change_tick,
            },
        }
    }

    #[inline]
    fn update_component_access(
        state: &Self::State,
        access: &mut bevy::ecs::query::FilteredAccess<ComponentId>,
    ) {
        for &component in &*state.components {
            assert!(
                !access.access().has_write(component),
                "&mut {} conflicts with a previous access in this query. Mutable component access must be unique.",
                std::any::type_name::<Trait>(),
            );
            access.add_write(component);
        }
    }

    #[inline]
    fn update_archetype_component_access(
        state: &Self::State,
        archetype: &bevy::ecs::archetype::Archetype,
        access: &mut bevy::ecs::query::Access<bevy::ecs::archetype::ArchetypeComponentId>,
    ) {
        for &component in &*state.components {
            if let Some(archetype_component_id) = archetype.get_archetype_component_id(component) {
                access.add_write(archetype_component_id);
            }
        }
    }

    #[inline]
    fn init_state(world: &mut World) -> Self::State {
        TraitQueryState::init(world)
    }
    #[inline]
    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        state.matches_component_set_one(set_contains_id)
    }
}
