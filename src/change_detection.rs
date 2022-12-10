use crate::{debug_unreachable, TraitImplRegistry, TraitQuery, TraitQueryState};
use bevy::ecs::archetype::{Archetype, ArchetypeComponentId};
use bevy::ecs::component::{ComponentId, ComponentTicks};
use bevy::prelude::DetectChanges;
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Access, FilteredAccess, ReadOnlyWorldQuery, WorldQuery};
use bevy::ecs::storage::{ComponentSparseSet, SparseSets, Table};
use bevy::ecs::world::World;
use bevy::ptr::{ThinSlicePtr, UnsafeCellDeref};

/// Unique mutable borrow of an entity's component
pub struct Mut<'a, T: ?Sized> {
    pub(crate) value: &'a mut T,
    pub(crate) ticks: Ticks<'a>,
}

pub struct Ticks<'a> {
    pub component_ticks: &'a mut ComponentTicks,
    pub last_change_tick: u32,
    pub change_tick: u32,
}

impl<T: ?Sized> DetectChanges for Mut<'_, T> {
    type Inner = T;

    #[inline]
    fn is_added(&self) -> bool {
        self.ticks
            .component_ticks
            .is_added(self.ticks.last_change_tick, self.ticks.change_tick)
    }

    #[inline]
    fn is_changed(&self) -> bool {
        self.ticks
            .component_ticks
            .is_changed(self.ticks.last_change_tick, self.ticks.change_tick)
    }

    #[inline]
    fn set_changed(&mut self) {
        self.ticks
            .component_ticks
            .set_changed(self.ticks.change_tick);
    }

    #[inline]
    fn last_changed(&self) -> u32 {
        self.ticks.last_change_tick
    }

    #[inline]
    fn set_last_changed(&mut self, last_change_tick: u32) {
        self.ticks.last_change_tick = last_change_tick;
    }

    #[inline]
    fn bypass_change_detection(&mut self) -> &mut Self::Inner {
        self.value
    }
}

impl<'a, T: ?Sized> Mut<'a, T> {
    /// Consume `self` and return a mutable reference to the
    /// contained value while marking `self` as "changed".
    #[inline]
    pub fn into_inner(mut self) -> &'a mut T {
        self.set_changed();
        self.value
    }
}

impl<T: ?Sized> std::fmt::Debug for Mut<'_, T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Mut").field(&self.value).finish()
    }
}

impl<T: ?Sized> Deref for Mut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<T: ?Sized> DerefMut for Mut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.set_changed();
        self.value
    }
}

impl<T: ?Sized> AsRef<T> for Mut<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        self.deref()
    }
}

impl<T: ?Sized> AsMut<T> for Mut<'_, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self.deref_mut()
    }
}

pub struct TraitAdded<'a, Trait: ?Sized + TraitQuery> {
    marker: PhantomData<&'a Trait>,
}

pub struct ChangeDetectionFetch<'w, Trait: ?Sized + TraitQuery> {
    registry: &'w TraitImplRegistry<Trait>,
    table_ticks: Vec<ThinSlicePtr<'w, UnsafeCell<ComponentTicks>>>,
    sparse_components: Vec<&'w ComponentSparseSet>,
    sparse_sets: &'w SparseSets,
    last_change_tick: u32,
    change_tick: u32,
}

unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for TraitAdded<'a, Trait> {
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
        last_change_tick: u32,
        change_tick: u32,
    ) -> Self::Fetch<'w> {
        Self::Fetch::<'w> {
            registry: world.resource(),
            table_ticks: Vec::new(),
            sparse_components: Vec::new(),
            sparse_sets: &world.storages().sparse_sets,
            last_change_tick,
            change_tick,
        }
    }

    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        Self::Fetch {
            registry: fetch.registry,
            table_ticks: fetch.table_ticks.clone(),
            sparse_components: fetch.sparse_components.clone(),
            sparse_sets: fetch.sparse_sets,
            last_change_tick: fetch.last_change_tick,
            change_tick: fetch.change_tick,
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
        // Search for registered trait impls that are present in the table.
        fetch.table_ticks = Vec::from_iter(state.components.iter().filter_map(|component| {
            table
                .get_column(*component)
                .map(|column| ThinSlicePtr::from(column.get_ticks_slice()))
        }));

        fetch.sparse_components = Vec::from_iter(
            state
                .components
                .iter()
                .filter_map(|component| fetch.sparse_sets.get(*component)),
        );
    }

    #[inline]
    unsafe fn set_table<'w>(_fetch: &mut Self::Fetch<'w>, _state: &Self::State, _table: &'w Table) {
        // only gets called if IS_DENSE == true, which does not hold for us
        unimplemented!()
    }

    #[inline(always)]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: usize,
    ) -> Self::Item<'w> {
        fetch
            .table_ticks
            .iter()
            .map(|ticks_slice| ticks_slice.get(table_row))
            .chain(
                fetch
                    .sparse_components
                    .iter()
                    .filter_map(|component_sparse_set| component_sparse_set.get_ticks(entity)),
            )
            .any(|ticks_ptr| {
                ticks_ptr
                    .deref()
                    .is_added(fetch.last_change_tick, fetch.change_tick)
            })
    }

    #[inline(always)]
    unsafe fn filter_fetch(fetch: &mut Self::Fetch<'_>, entity: Entity, table_row: usize) -> bool {
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
        state.matches_component_set_any(set_contains_id)
    }
}

/// SAFETY: read-only access
unsafe impl<'a, Trait: ?Sized + TraitQuery> ReadOnlyWorldQuery for TraitAdded<'a, Trait> {}

pub struct TraitChanged<'a, Trait: ?Sized + TraitQuery> {
    marker: PhantomData<&'a Trait>,
}

unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for TraitChanged<'a, Trait> {
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
        last_change_tick: u32,
        change_tick: u32,
    ) -> Self::Fetch<'w> {
        Self::Fetch::<'w> {
            registry: world.resource(),
            table_ticks: Vec::new(),
            sparse_components: Vec::new(),
            sparse_sets: &world.storages().sparse_sets,
            last_change_tick,
            change_tick,
        }
    }

    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        Self::Fetch {
            registry: fetch.registry,
            table_ticks: fetch.table_ticks.clone(),
            sparse_components: fetch.sparse_components.clone(),
            sparse_sets: fetch.sparse_sets,
            last_change_tick: fetch.last_change_tick,
            change_tick: fetch.change_tick,
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
        // Search for registered trait impls that are present in the table.
        fetch.table_ticks = Vec::from_iter(state.components.iter().filter_map(|component| {
            table
                .get_column(*component)
                .map(|column| ThinSlicePtr::from(column.get_ticks_slice()))
        }));

        fetch.sparse_components = Vec::from_iter(
            state
                .components
                .iter()
                .filter_map(|component| fetch.sparse_sets.get(*component)),
        );
    }

    #[inline]
    unsafe fn set_table<'w>(_fetch: &mut Self::Fetch<'w>, _state: &Self::State, _table: &'w Table) {
        // only gets called if IS_DENSE == true, which does not hold for us
        unimplemented!()
    }

    #[inline(always)]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: usize,
    ) -> Self::Item<'w> {
        fetch
            .table_ticks
            .iter()
            .map(|ticks_slice| ticks_slice.get(table_row))
            .chain(
                fetch
                    .sparse_components
                    .iter()
                    .filter_map(|component_sparse_set| component_sparse_set.get_ticks(entity)),
            )
            .any(|ticks_ptr| {
                ticks_ptr
                    .deref()
                    .is_changed(fetch.last_change_tick, fetch.change_tick)
            })
    }

    #[inline(always)]
    unsafe fn filter_fetch(fetch: &mut Self::Fetch<'_>, entity: Entity, table_row: usize) -> bool {
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
        state.matches_component_set_any(set_contains_id)
    }
}

/// SAFETY: read-only access
unsafe impl<'a, Trait: ?Sized + TraitQuery> ReadOnlyWorldQuery for TraitChanged<'a, Trait> {}
