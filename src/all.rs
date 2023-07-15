use crate::{
    debug_unreachable, trait_registry_error, zip_exact, TraitImplMeta, TraitImplRegistry,
    TraitQuery, TraitQueryState,
};
use bevy::ecs::change_detection::{DetectChanges, Mut, Ref};
use bevy::ecs::component::{ComponentId, Tick};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{QueryItem, ReadOnlyWorldQuery, WorldQuery};
use bevy::ecs::storage::{SparseSets, Table, TableRow};
use bevy::ecs::world::unsafe_world_cell::UnsafeWorldCell;
use bevy::ecs::world::World;
use bevy::ptr::UnsafeCellDeref;

/// Read-access to all components implementing a trait for a given entity.
pub struct ReadTraits<'a, Trait: ?Sized + TraitQuery> {
    // Read-only access to the global trait registry.
    // Since no one outside of the crate can name the registry type,
    // we can be confident that no write accesses will conflict with this.
    registry: &'a TraitImplRegistry<Trait>,
    table: &'a Table,
    table_row: TableRow,
    /// This grants shared access to all sparse set components,
    /// but in practice we will only read the components specified in `self.registry`.
    /// The fetch impl registers read-access for all of these components,
    /// so there will be no runtime conflicts.
    sparse_sets: &'a SparseSets,
    last_run: Tick,
    this_run: Tick,
}

#[doc(hidden)]
pub struct ReadTableTraitsIter<'a, Trait: ?Sized> {
    // SAFETY: These two iterators must have equal length.
    components: std::slice::Iter<'a, ComponentId>,
    meta: std::slice::Iter<'a, TraitImplMeta<Trait>>,
    table_row: TableRow,
    // Grants shared access to the components corresponding to `components` in this table.
    // Not all components are guaranteed to exist in the table.
    table: &'a Table,
    last_run: Tick,
    this_run: Tick,
}

#[doc(hidden)]
pub type CombinedReadTraitsIter<'a, Trait> =
    std::iter::Chain<ReadTableTraitsIter<'a, Trait>, ReadSparseTraitsIter<'a, Trait>>;

impl<'a, Trait: ?Sized + TraitQuery> Iterator for ReadTableTraitsIter<'a, Trait> {
    type Item = Ref<'a, Trait>;
    fn next(&mut self) -> Option<Self::Item> {
        // Iterate the remaining table components that are registered,
        // until we find one that exists in the table.
        let (column, meta) = unsafe { zip_exact(&mut self.components, &mut self.meta) }
            .find_map(|(&component, meta)| self.table.get_column(component).zip(Some(meta)))?;
        // SAFETY: We have shared access to the entire column.
        let ptr = unsafe {
            column
                .get_data_ptr()
                .byte_add(self.table_row.index() * meta.size_bytes)
        };
        let trait_object = unsafe { meta.dyn_ctor.cast(ptr) };

        // SAFETY: we know that the `table_row` is a valid index.
        // Read access has been registered, so we can dereference it immutably.
        let added_tick = unsafe { column.get_added_ticks_unchecked(self.table_row).deref() };
        let changed_tick = unsafe { column.get_changed_ticks_unchecked(self.table_row).deref() };

        Some(Ref::new(
            trait_object,
            added_tick,
            changed_tick,
            self.last_run,
            self.this_run,
        ))
    }
}

#[doc(hidden)]
pub struct ReadSparseTraitsIter<'a, Trait: ?Sized> {
    // SAFETY: These two iterators must have equal length.
    components: std::slice::Iter<'a, ComponentId>,
    meta: std::slice::Iter<'a, TraitImplMeta<Trait>>,
    entity: Entity,
    // Grants shared access to the components corresponding to both `components` and `entity`.
    sparse_sets: &'a SparseSets,
    last_run: Tick,
    this_run: Tick,
}

impl<'a, Trait: ?Sized + TraitQuery> Iterator for ReadSparseTraitsIter<'a, Trait> {
    type Item = Ref<'a, Trait>;
    fn next(&mut self) -> Option<Self::Item> {
        // Iterate the remaining sparse set components that are registered,
        // until we find one that exists in the archetype.
        let ((ptr, ticks_ptr), meta) = unsafe { zip_exact(&mut self.components, &mut self.meta) }
            .find_map(|(&component, meta)| {
            self.sparse_sets
                .get(component)
                .and_then(|set| set.get_with_ticks(self.entity))
                .zip(Some(meta))
        })?;
        let trait_object = unsafe { meta.dyn_ctor.cast(ptr) };
        let added_tick = unsafe { ticks_ptr.added.deref() };
        let changed_tick = unsafe { ticks_ptr.changed.deref() };
        Some(Ref::new(
            trait_object,
            added_tick,
            changed_tick,
            self.last_run,
            self.this_run,
        ))
    }
}

impl<'w, Trait: ?Sized + TraitQuery> IntoIterator for ReadTraits<'w, Trait> {
    type Item = Ref<'w, Trait>;
    type IntoIter = CombinedReadTraitsIter<'w, Trait>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let table = ReadTableTraitsIter {
            components: self.registry.table_components.iter(),
            meta: self.registry.table_meta.iter(),
            table: self.table,
            table_row: self.table_row,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        let sparse = ReadSparseTraitsIter {
            components: self.registry.sparse_components.iter(),
            meta: self.registry.sparse_meta.iter(),
            entity: self.table.entities()[self.table_row.index()],
            sparse_sets: self.sparse_sets,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        table.chain(sparse)
    }
}

impl<'w, Trait: ?Sized + TraitQuery> IntoIterator for &ReadTraits<'w, Trait> {
    type Item = Ref<'w, Trait>;
    type IntoIter = CombinedReadTraitsIter<'w, Trait>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let table = ReadTableTraitsIter {
            components: self.registry.table_components.iter(),
            meta: self.registry.table_meta.iter(),
            table: self.table,
            table_row: self.table_row,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        let sparse = ReadSparseTraitsIter {
            components: self.registry.sparse_components.iter(),
            meta: self.registry.sparse_meta.iter(),
            entity: self.table.entities()[self.table_row.index()],
            sparse_sets: self.sparse_sets,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        table.chain(sparse)
    }
}

impl<'w, Trait: ?Sized + TraitQuery> ReadTraits<'w, Trait> {
    /// Returns an iterator over the components implementing `Trait` for the current entity.
    pub fn iter(&self) -> CombinedReadTraitsIter<'w, Trait> {
        self.into_iter()
    }
}

#[doc(hidden)]
pub struct AllTraitsFetch<'w, Trait: ?Sized> {
    registry: &'w TraitImplRegistry<Trait>,
    table: Option<&'w Table>,
    sparse_sets: &'w SparseSets,
    last_run: Tick,
    this_run: Tick,
}

/// Write-access to all components implementing a trait for a given entity.
pub struct WriteTraits<'a, Trait: ?Sized + TraitQuery> {
    // Read-only access to the global trait registry.
    // Since no one outside of the crate can name the registry type,
    // we can be confident that no write accesses will conflict with this.
    registry: &'a TraitImplRegistry<Trait>,

    table: &'a Table,
    table_row: TableRow,

    last_run: Tick,
    this_run: Tick,

    /// This grants shared mutable access to all sparse set components,
    /// but in practice we will only modify the components specified in `self.registry`.
    /// The fetch impl registers write-access for all of these components,
    /// guaranteeing us exclusive access at runtime.
    sparse_sets: &'a SparseSets,
}

#[doc(hidden)]
pub type CombinedWriteTraitsIter<'a, Trait> =
    std::iter::Chain<WriteTableTraitsIter<'a, Trait>, WriteSparseTraitsIter<'a, Trait>>;

#[doc(hidden)]
pub struct WriteTableTraitsIter<'a, Trait: ?Sized> {
    // SAFETY: These two iterators must have equal length.
    components: std::slice::Iter<'a, ComponentId>,
    meta: std::slice::Iter<'a, TraitImplMeta<Trait>>,
    table: &'a Table,
    /// SAFETY: Given the same trait type and same archetype,
    /// no two instances of this struct may have the same `table_row`.
    table_row: TableRow,
    last_run: Tick,
    this_run: Tick,
}

impl<'a, Trait: ?Sized + TraitQuery> Iterator for WriteTableTraitsIter<'a, Trait> {
    type Item = Mut<'a, Trait>;
    fn next(&mut self) -> Option<Self::Item> {
        // Iterate the remaining table components that are registered,
        // until we find one that exists in the table.
        let (column, meta) = unsafe { zip_exact(&mut self.components, &mut self.meta) }
            .find_map(|(&component, meta)| self.table.get_column(component).zip(Some(meta)))?;
        let ptr = unsafe {
            column
                .get_data_ptr()
                .byte_add(self.table_row.index() * meta.size_bytes)
        };
        // SAFETY: The instance of `WriteTraits` that created this iterator
        // has exclusive access to all table components registered with the trait.
        //
        // Since `self.table_row` is guaranteed to be unique, we know that other instances
        // of `WriteTableTraitsIter` will not conflict with this pointer.
        let ptr = unsafe { ptr.assert_unique() };
        let trait_object = unsafe { meta.dyn_ctor.cast_mut(ptr) };
        // SAFETY: We have exclusive access to the component, so by extension
        // we have exclusive access to the corresponding `ComponentTicks`.
        let added = unsafe { column.get_added_ticks_unchecked(self.table_row).deref_mut() };
        let changed = unsafe {
            column
                .get_changed_ticks_unchecked(self.table_row)
                .deref_mut()
        };
        Some(Mut::new(
            trait_object,
            added,
            changed,
            self.last_run,
            self.this_run,
        ))
    }
}

#[doc(hidden)]
pub struct WriteSparseTraitsIter<'a, Trait: ?Sized> {
    // SAFETY: These two iterators must have equal length.
    components: std::slice::Iter<'a, ComponentId>,
    meta: std::slice::Iter<'a, TraitImplMeta<Trait>>,
    /// SAFETY: Given the same trait type and same archetype,
    /// no two instances of this struct may have the same `entity`.
    entity: Entity,
    sparse_sets: &'a SparseSets,
    last_run: Tick,
    this_run: Tick,
}

impl<'a, Trait: ?Sized + TraitQuery> Iterator for WriteSparseTraitsIter<'a, Trait> {
    type Item = Mut<'a, Trait>;
    fn next(&mut self) -> Option<Self::Item> {
        // Iterate the remaining sparse set components we have registered,
        // until we find one that exists in the archetype.
        let ((ptr, component_ticks), meta) =
            unsafe { zip_exact(&mut self.components, &mut self.meta) }.find_map(
                |(&component, meta)| {
                    self.sparse_sets
                        .get(component)
                        .and_then(|set| set.get_with_ticks(self.entity))
                        .zip(Some(meta))
                },
            )?;

        // SAFETY: The instance of `WriteTraits` that created this iterator
        // has exclusive access to all sparse set components registered with the trait.
        //
        // Since `self.entity` is guaranteed to be unique, we know that other instances
        // of `WriteSparseTraitsIter` will not conflict with this pointer.
        let ptr = unsafe { ptr.assert_unique() };
        let trait_object = unsafe { meta.dyn_ctor.cast_mut(ptr) };
        // SAFETY: We have exclusive access to the component, so by extension
        // we have exclusive access to the corresponding `ComponentTicks`.
        let added = unsafe { component_ticks.added.deref_mut() };
        let changed = unsafe { component_ticks.changed.deref_mut() };

        Some(Mut::new(
            trait_object,
            added,
            changed,
            self.last_run,
            self.this_run,
        ))
    }
}

impl<'w, Trait: ?Sized + TraitQuery> WriteTraits<'w, Trait> {
    /// Returns an iterator over the components implementing `Trait` for the current entity.
    pub fn iter(&self) -> CombinedReadTraitsIter<'_, Trait> {
        self.into_iter()
    }
    /// Returns a mutable iterator over the components implementing `Trait` for the current entity.
    pub fn iter_mut(&mut self) -> CombinedWriteTraitsIter<'_, Trait> {
        self.into_iter()
    }
}

impl<'w, Trait: ?Sized + TraitQuery> IntoIterator for WriteTraits<'w, Trait> {
    type Item = Mut<'w, Trait>;
    type IntoIter = CombinedWriteTraitsIter<'w, Trait>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let table = WriteTableTraitsIter {
            components: self.registry.table_components.iter(),
            meta: self.registry.table_meta.iter(),
            table: self.table,
            table_row: self.table_row,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        let sparse = WriteSparseTraitsIter {
            components: self.registry.sparse_components.iter(),
            meta: self.registry.sparse_meta.iter(),
            entity: self.table.entities()[self.table_row.index()],
            sparse_sets: self.sparse_sets,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        table.chain(sparse)
    }
}

impl<'world, 'local, Trait: ?Sized + TraitQuery> IntoIterator
    for &'local WriteTraits<'world, Trait>
{
    type Item = Ref<'local, Trait>;
    type IntoIter = CombinedReadTraitsIter<'local, Trait>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let table = ReadTableTraitsIter {
            components: self.registry.table_components.iter(),
            meta: self.registry.table_meta.iter(),
            table: self.table,
            table_row: self.table_row,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        let sparse = ReadSparseTraitsIter {
            components: self.registry.sparse_components.iter(),
            meta: self.registry.sparse_meta.iter(),
            entity: self.table.entities()[self.table_row.index()],
            sparse_sets: self.sparse_sets,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        table.chain(sparse)
    }
}

impl<'world, 'local, Trait: ?Sized + TraitQuery> IntoIterator
    for &'local mut WriteTraits<'world, Trait>
{
    type Item = Mut<'local, Trait>;
    type IntoIter = CombinedWriteTraitsIter<'local, Trait>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let table = WriteTableTraitsIter {
            components: self.registry.table_components.iter(),
            meta: self.registry.table_meta.iter(),
            table: self.table,
            table_row: self.table_row,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        let sparse = WriteSparseTraitsIter {
            components: self.registry.sparse_components.iter(),
            meta: self.registry.sparse_meta.iter(),
            entity: self.table.entities()[self.table_row.index()],
            sparse_sets: self.sparse_sets,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        table.chain(sparse)
    }
}

/// `WorldQuery` adapter that fetches all implementations of a given trait for an entity.
///
/// You can usually just use `&dyn Trait` or `&mut dyn Trait` as a `WorldQuery` directly.
pub struct All<T: ?Sized>(T);

unsafe impl<'a, Trait: ?Sized + TraitQuery> ReadOnlyWorldQuery for All<&'a Trait> {}

// SAFETY: We only access the components registered in the trait registry.
// This is known to match the set of components in the TraitQueryState,
// which is used to match archetypes and register world access.
unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for All<&'a Trait> {
    type Item<'w> = ReadTraits<'w, Trait>;
    type Fetch<'w> = AllTraitsFetch<'w, Trait>;
    type ReadOnly = Self;
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

    #[inline]
    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        AllTraitsFetch {
            registry: fetch.registry,
            table: fetch.table,
            sparse_sets: fetch.sparse_sets,
            last_run: fetch.last_run,
            this_run: fetch.this_run,
        }
    }

    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut Self::Fetch<'w>,
        _state: &Self::State,
        _archetype: &'w bevy::ecs::archetype::Archetype,
        table: &'w bevy::ecs::storage::Table,
    ) {
        fetch.table = Some(table);
    }

    unsafe fn set_table<'w>(
        fetch: &mut Self::Fetch<'w>,
        _state: &Self::State,
        table: &'w bevy::ecs::storage::Table,
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
        state.matches_component_set_any(set_contains_id)
    }
}

// SAFETY: We only access the components registered in the trait registry.
// This is known to match the set of components in the TraitQueryState,
// which is used to match archetypes and register world access.
unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for All<&'a mut Trait> {
    type Item<'w> = WriteTraits<'w, Trait>;
    type Fetch<'w> = AllTraitsFetch<'w, Trait>;
    type ReadOnly = All<&'a Trait>;
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

    #[inline]
    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        AllTraitsFetch {
            registry: fetch.registry,
            table: fetch.table,
            sparse_sets: fetch.sparse_sets,
            last_run: fetch.last_run,
            this_run: fetch.this_run,
        }
    }

    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut Self::Fetch<'w>,
        _state: &Self::State,
        _archetype: &'w bevy::ecs::archetype::Archetype,
        table: &'w bevy::ecs::storage::Table,
    ) {
        fetch.table = Some(table);
    }

    #[inline]
    unsafe fn set_table<'w>(
        fetch: &mut Self::Fetch<'w>,
        _state: &Self::State,
        table: &'w bevy::ecs::storage::Table,
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
        state.matches_component_set_any(set_contains_id)
    }
}

// SAFETY: We only access the components registered in the trait registry.
// This is known to match the set of components in the TraitQueryState,
// which is used to match archetypes and register world access.
unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for AllAdded<&'a mut Trait> {
    type Item<'w> = AddedWriteTraits<'w, Trait>;
    type Fetch<'w> = AllTraitsFetch<'w, Trait>;
    type ReadOnly = All<&'a Trait>;
    type State = TraitQueryState<Trait>;

    #[inline]
    fn shrink<'wlong: 'wshort, 'wshort>(item: QueryItem<'wlong, Self>) -> QueryItem<'wshort, Self> {
        item
    }

    #[inline]
    unsafe fn init_fetch<'w>(
        world: UnsafeWorldCell<'w>,
        state: &Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        <All<&'a mut Trait> as WorldQuery>::init_fetch(world, state, last_run, this_run)
    }

    #[inline]
    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        <All<&'a mut Trait> as WorldQuery>::clone_fetch(fetch)
    }

    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut Self::Fetch<'w>,
        state: &Self::State,
        archetype: &'w bevy::ecs::archetype::Archetype,
        table: &'w bevy::ecs::storage::Table,
    ) {
        <All<&'a mut Trait> as WorldQuery>::set_archetype(fetch, state, archetype, table);
    }

    #[inline]
    unsafe fn set_table<'w>(
        fetch: &mut Self::Fetch<'w>,
        state: &Self::State,
        table: &'w bevy::ecs::storage::Table,
    ) {
        <All<&'a mut Trait> as WorldQuery>::set_table(fetch, state, table);
    }

    #[inline]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Self::Item<'w> {
        AddedWriteTraits(<All<&'a mut Trait> as WorldQuery>::fetch(
            fetch, entity, table_row,
        ))
    }

    #[inline]
    fn update_component_access(
        state: &Self::State,
        access: &mut bevy::ecs::query::FilteredAccess<ComponentId>,
    ) {
        <All<&'a mut Trait> as WorldQuery>::update_component_access(state, access);
    }

    #[inline]
    fn update_archetype_component_access(
        state: &Self::State,
        archetype: &bevy::ecs::archetype::Archetype,
        access: &mut bevy::ecs::query::Access<bevy::ecs::archetype::ArchetypeComponentId>,
    ) {
        <All<&'a mut Trait> as WorldQuery>::update_archetype_component_access(
            state, archetype, access,
        );
    }

    #[inline]
    fn init_state(world: &mut World) -> Self::State {
        <All<&'a mut Trait> as WorldQuery>::init_state(world)
    }
    #[inline]
    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        <All<&'a mut Trait> as WorldQuery>::matches_component_set(state, set_contains_id)
    }
}

// SAFETY: We only access the components registered in the trait registry.
// This is known to match the set of components in the TraitQueryState,
// which is used to match archetypes and register world access.
unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for AllChanged<&'a mut Trait> {
    type Item<'w> = ChangedWriteTraits<'w, Trait>;
    type Fetch<'w> = AllTraitsFetch<'w, Trait>;
    type ReadOnly = All<&'a Trait>;
    type State = TraitQueryState<Trait>;

    #[inline]
    fn shrink<'wlong: 'wshort, 'wshort>(item: QueryItem<'wlong, Self>) -> QueryItem<'wshort, Self> {
        item
    }

    #[inline]
    unsafe fn init_fetch<'w>(
        world: UnsafeWorldCell<'w>,
        state: &Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        <All<&'a mut Trait> as WorldQuery>::init_fetch(world, state, last_run, this_run)
    }

    #[inline]
    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        <All<&'a mut Trait> as WorldQuery>::clone_fetch(fetch)
    }

    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut Self::Fetch<'w>,
        state: &Self::State,
        archetype: &'w bevy::ecs::archetype::Archetype,
        table: &'w bevy::ecs::storage::Table,
    ) {
        <All<&'a mut Trait> as WorldQuery>::set_archetype(fetch, state, archetype, table);
    }

    #[inline]
    unsafe fn set_table<'w>(
        fetch: &mut Self::Fetch<'w>,
        state: &Self::State,
        table: &'w bevy::ecs::storage::Table,
    ) {
        <All<&'a mut Trait> as WorldQuery>::set_table(fetch, state, table);
    }

    #[inline]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Self::Item<'w> {
        ChangedWriteTraits(<All<&'a mut Trait> as WorldQuery>::fetch(
            fetch, entity, table_row,
        ))
    }

    #[inline]
    fn update_component_access(
        state: &Self::State,
        access: &mut bevy::ecs::query::FilteredAccess<ComponentId>,
    ) {
        <All<&'a mut Trait> as WorldQuery>::update_component_access(state, access);
    }

    #[inline]
    fn update_archetype_component_access(
        state: &Self::State,
        archetype: &bevy::ecs::archetype::Archetype,
        access: &mut bevy::ecs::query::Access<bevy::ecs::archetype::ArchetypeComponentId>,
    ) {
        <All<&'a mut Trait> as WorldQuery>::update_archetype_component_access(
            state, archetype, access,
        );
    }

    #[inline]
    fn init_state(world: &mut World) -> Self::State {
        <All<&'a mut Trait> as WorldQuery>::init_state(world)
    }
    #[inline]
    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        <All<&'a mut Trait> as WorldQuery>::matches_component_set(state, set_contains_id)
    }
}

/// Read-access to all components implementing a trait for a given entity.
pub struct AddedReadTraits<'a, Trait: ?Sized + TraitQuery>(ReadTraits<'a, Trait>);

/// Read-access to all components implementing a trait for a given entity.
pub struct ChangedReadTraits<'a, Trait: ?Sized + TraitQuery>(ReadTraits<'a, Trait>);

#[doc(hidden)]
pub struct AddedIteratorFilter<I>(I);

impl<I> Iterator for AddedIteratorFilter<I>
where
    I: Iterator,
    I::Item: DetectChanges,
{
    type Item = <I as Iterator>::Item;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.find(DetectChanges::is_added)
    }
}

#[doc(hidden)]
pub struct ChangedIteratorFilter<I>(I);

impl<I> Iterator for ChangedIteratorFilter<I>
where
    I: Iterator,
    I::Item: DetectChanges,
{
    type Item = <I as Iterator>::Item;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.find(DetectChanges::is_changed)
    }
}

impl<'w, Trait: ?Sized + TraitQuery> IntoIterator for AddedReadTraits<'w, Trait> {
    type Item = Ref<'w, Trait>;
    type IntoIter = AddedIteratorFilter<CombinedReadTraitsIter<'w, Trait>>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        AddedIteratorFilter(self.0.into_iter())
    }
}

impl<'w, Trait: ?Sized + TraitQuery> IntoIterator for ChangedReadTraits<'w, Trait> {
    type Item = Ref<'w, Trait>;
    type IntoIter = ChangedIteratorFilter<CombinedReadTraitsIter<'w, Trait>>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        ChangedIteratorFilter(self.0.into_iter())
    }
}

impl<'w, Trait: ?Sized + TraitQuery> IntoIterator for &AddedReadTraits<'w, Trait> {
    type Item = Ref<'w, Trait>;
    type IntoIter = AddedIteratorFilter<CombinedReadTraitsIter<'w, Trait>>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        AddedIteratorFilter((&self.0).into_iter())
    }
}

impl<'w, Trait: ?Sized + TraitQuery> IntoIterator for &ChangedReadTraits<'w, Trait> {
    type Item = Ref<'w, Trait>;
    type IntoIter = ChangedIteratorFilter<CombinedReadTraitsIter<'w, Trait>>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        ChangedIteratorFilter((&self.0).into_iter())
    }
}

impl<'w, Trait: ?Sized + TraitQuery> AddedReadTraits<'w, Trait> {
    /// Returns an iterator over the components implementing `Trait` for the current entity.
    pub fn iter(&self) -> impl Iterator<Item = Ref<'w, Trait>> {
        self.into_iter()
    }
}

impl<'w, Trait: ?Sized + TraitQuery> ChangedReadTraits<'w, Trait> {
    /// Returns an iterator over the components implementing `Trait` for the current entity.
    pub fn iter(&self) -> impl Iterator<Item = Ref<'w, Trait>> {
        self.into_iter()
    }
}

/// Write-access to all components implementing a trait for a given entity, and have been newly
/// added in the last tick.
pub struct AddedWriteTraits<'a, Trait: ?Sized + TraitQuery>(WriteTraits<'a, Trait>);

/// Write-access to all components implementing a trait for a given entity, and have been changed in
/// the last tick.
pub struct ChangedWriteTraits<'a, Trait: ?Sized + TraitQuery>(WriteTraits<'a, Trait>);

impl<'w, Trait: ?Sized + TraitQuery> AddedWriteTraits<'w, Trait> {
    /// Returns an iterator over the components implementing `Trait` for the current entity.
    pub fn iter(&self) -> impl Iterator<Item = Ref<'_, Trait>> {
        self.into_iter()
    }
    /// Returns a mutable iterator over the components implementing `Trait` for the current entity.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = Mut<'_, Trait>> {
        self.into_iter()
    }
}

impl<'w, Trait: ?Sized + TraitQuery> ChangedWriteTraits<'w, Trait> {
    /// Returns an iterator over the components implementing `Trait` for the current entity.
    pub fn iter(&self) -> impl Iterator<Item = Ref<'_, Trait>> {
        self.into_iter()
    }
    /// Returns a mutable iterator over the components implementing `Trait` for the current entity.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = Mut<'_, Trait>> {
        self.into_iter()
    }
}

impl<'w, Trait: ?Sized + TraitQuery> IntoIterator for AddedWriteTraits<'w, Trait> {
    type Item = Mut<'w, Trait>;
    type IntoIter = AddedIteratorFilter<CombinedWriteTraitsIter<'w, Trait>>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        AddedIteratorFilter(self.0.into_iter())
    }
}

impl<'w, Trait: ?Sized + TraitQuery> IntoIterator for ChangedWriteTraits<'w, Trait> {
    type Item = Mut<'w, Trait>;
    type IntoIter = ChangedIteratorFilter<CombinedWriteTraitsIter<'w, Trait>>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        ChangedIteratorFilter(self.0.into_iter())
    }
}

impl<'world, 'local, Trait: ?Sized + TraitQuery> IntoIterator
    for &'local AddedWriteTraits<'world, Trait>
{
    type Item = Ref<'local, Trait>;
    type IntoIter = AddedIteratorFilter<CombinedReadTraitsIter<'local, Trait>>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        AddedIteratorFilter((&self.0).into_iter())
    }
}

impl<'world, 'local, Trait: ?Sized + TraitQuery> IntoIterator
    for &'local ChangedWriteTraits<'world, Trait>
{
    type Item = Ref<'local, Trait>;
    type IntoIter = ChangedIteratorFilter<CombinedReadTraitsIter<'local, Trait>>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        ChangedIteratorFilter((&self.0).into_iter())
    }
}

impl<'world, 'local, Trait: ?Sized + TraitQuery> IntoIterator
    for &'local mut AddedWriteTraits<'world, Trait>
{
    type Item = Mut<'local, Trait>;
    type IntoIter = AddedIteratorFilter<CombinedWriteTraitsIter<'local, Trait>>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        AddedIteratorFilter((&mut self.0).into_iter())
    }
}

impl<'world, 'local, Trait: ?Sized + TraitQuery> IntoIterator
    for &'local mut ChangedWriteTraits<'world, Trait>
{
    type Item = Mut<'local, Trait>;
    type IntoIter = ChangedIteratorFilter<CombinedWriteTraitsIter<'local, Trait>>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        ChangedIteratorFilter((&mut self.0).into_iter())
    }
}

/// `WorldQuery` adapter that fetches all implementations of a given trait for an entity, with
/// the additional condition that they have been added since the last tick.
pub struct AllAdded<T: ?Sized>(T);

/// `WorldQuery` adapter that fetches all implementations of a given trait for an entity, with
/// the additional condition that they have also changed since the last tick.
pub struct AllChanged<T: ?Sized>(T);

unsafe impl<'a, Trait: ?Sized + TraitQuery> ReadOnlyWorldQuery for AllAdded<&'a Trait> {}

unsafe impl<'a, Trait: ?Sized + TraitQuery> ReadOnlyWorldQuery for AllChanged<&'a Trait> {}

// SAFETY: We only access the components registered in the trait registry.
// This is known to match the set of components in the TraitQueryState,
// which is used to match archetypes and register world access.
unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for AllAdded<&'a Trait> {
    type Item<'w> = AddedReadTraits<'w, Trait>;
    type Fetch<'w> = AllTraitsFetch<'w, Trait>;
    type ReadOnly = Self;
    type State = TraitQueryState<Trait>;

    #[inline]
    fn shrink<'wlong: 'wshort, 'wshort>(item: QueryItem<'wlong, Self>) -> QueryItem<'wshort, Self> {
        item
    }

    #[inline]
    unsafe fn init_fetch<'w>(
        world: UnsafeWorldCell<'w>,
        state: &Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        <All<&'a Trait> as WorldQuery>::init_fetch(world, state, last_run, this_run)
    }

    #[inline]
    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        <All<&'a Trait> as WorldQuery>::clone_fetch(fetch)
    }

    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut Self::Fetch<'w>,
        state: &Self::State,
        archetype: &'w bevy::ecs::archetype::Archetype,
        table: &'w bevy::ecs::storage::Table,
    ) {
        <All<&'a Trait> as WorldQuery>::set_archetype(fetch, state, archetype, table);
    }

    unsafe fn set_table<'w>(
        fetch: &mut Self::Fetch<'w>,
        state: &Self::State,
        table: &'w bevy::ecs::storage::Table,
    ) {
        <All<&'a Trait> as WorldQuery>::set_table(fetch, state, table);
    }

    #[inline]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Self::Item<'w> {
        AddedReadTraits(<All<&'a Trait> as WorldQuery>::fetch(
            fetch, entity, table_row,
        ))
    }

    #[inline]
    fn update_component_access(
        state: &Self::State,
        access: &mut bevy::ecs::query::FilteredAccess<ComponentId>,
    ) {
        <All<&'a Trait> as WorldQuery>::update_component_access(state, access);
    }

    #[inline]
    fn update_archetype_component_access(
        state: &Self::State,
        archetype: &bevy::ecs::archetype::Archetype,
        access: &mut bevy::ecs::query::Access<bevy::ecs::archetype::ArchetypeComponentId>,
    ) {
        <All<&'a Trait> as WorldQuery>::update_archetype_component_access(state, archetype, access);
    }

    #[inline]
    fn init_state(world: &mut World) -> Self::State {
        <All<&'a Trait> as WorldQuery>::init_state(world)
    }
    #[inline]
    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        <All<&'a Trait> as WorldQuery>::matches_component_set(state, set_contains_id)
    }
}

// SAFETY: We only access the components registered in the trait registry.
// This is known to match the set of components in the TraitQueryState,
// which is used to match archetypes and register world access.
unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for AllChanged<&'a Trait> {
    type Item<'w> = ChangedReadTraits<'w, Trait>;
    type Fetch<'w> = AllTraitsFetch<'w, Trait>;
    type ReadOnly = Self;
    type State = TraitQueryState<Trait>;

    #[inline]
    fn shrink<'wlong: 'wshort, 'wshort>(item: QueryItem<'wlong, Self>) -> QueryItem<'wshort, Self> {
        item
    }

    #[inline]
    unsafe fn init_fetch<'w>(
        world: UnsafeWorldCell<'w>,
        state: &Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        <All<&'a Trait> as WorldQuery>::init_fetch(world, state, last_run, this_run)
    }

    #[inline]
    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        <All<&'a Trait> as WorldQuery>::clone_fetch(fetch)
    }

    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut Self::Fetch<'w>,
        state: &Self::State,
        archetype: &'w bevy::ecs::archetype::Archetype,
        table: &'w bevy::ecs::storage::Table,
    ) {
        <All<&'a Trait> as WorldQuery>::set_archetype(fetch, state, archetype, table);
    }

    unsafe fn set_table<'w>(
        fetch: &mut Self::Fetch<'w>,
        state: &Self::State,
        table: &'w bevy::ecs::storage::Table,
    ) {
        <All<&'a Trait> as WorldQuery>::set_table(fetch, state, table);
    }

    #[inline]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Self::Item<'w> {
        ChangedReadTraits(<All<&'a Trait> as WorldQuery>::fetch(
            fetch, entity, table_row,
        ))
    }

    #[inline]
    fn update_component_access(
        state: &Self::State,
        access: &mut bevy::ecs::query::FilteredAccess<ComponentId>,
    ) {
        <All<&'a Trait> as WorldQuery>::update_component_access(state, access);
    }

    #[inline]
    fn update_archetype_component_access(
        state: &Self::State,
        archetype: &bevy::ecs::archetype::Archetype,
        access: &mut bevy::ecs::query::Access<bevy::ecs::archetype::ArchetypeComponentId>,
    ) {
        <All<&'a Trait> as WorldQuery>::update_archetype_component_access(state, archetype, access);
    }

    #[inline]
    fn init_state(world: &mut World) -> Self::State {
        <All<&'a Trait> as WorldQuery>::init_state(world)
    }
    #[inline]
    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        <All<&'a Trait> as WorldQuery>::matches_component_set(state, set_contains_id)
    }
}
