use crate::{debug_unreachable, TraitImplRegistry, TraitQuery, TraitQueryState};
use bevy::ecs::component::{ComponentId, StorageType};
use bevy::{ecs::component::ComponentTicks, prelude::DetectChanges};
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use bevy::ecs::query::{Added, WorldQuery};
use bevy::ecs::storage::{SparseSets, Table};
use bevy::ecs::world::World;
use bevy::ptr::ThinSlicePtr;

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

pub struct AddedFetch<'w, Trait: ?Sized + TraitQuery> {
    registry: &'w TraitImplRegistry<Trait>,
    table_ticks: Option<ThinSlicePtr<'w, UnsafeCell<Tick>>>,
    sparse_sets: &'w SparseSets,
    last_change_tick: u32,
    change_tick: u32,
}

unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for Added<Trait> {
    type Fetch<'w> = AddedFetch<'w, Trait>;
    type Item<'w> = bool;
    type ReadOnly = Self;
    type State = TraitQueryState<Trait>;

    fn shrink<'wlong: 'wshort, 'wshort>(item: Self::Item<'wlong>) -> Self::Item<'wshort> {
        item
    }

    unsafe fn init_fetch<'w>(
        world: &'w World,
        &id: &ComponentId,
        last_change_tick: u32,
        change_tick: u32,
    ) -> Self::Fetch<'w> {
        Self::Fetch::<'w> {
            table_ticks: None,
            sparse_set: &world.storages().sparse_sets,
            last_change_tick,
            change_tick,
        }
    }

    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        Self::Fetch {
            table_ticks: fetch.table_ticks,
            sparse_set: fetch.sparse_set,
            last_change_tick: fetch.last_change_tick,
            change_tick: fetch.change_tick,
        }
    }

    // This should be the AND of all IS_DENSE for traits in the trait query
    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = false;

    #[inline]
    unsafe fn set_table<'w>(fetch: &mut Self::Fetch<'w>, state: &Self::State, table: &'w Table) {
        // Search for a registered trait impl that is present in the table.
        for (&component, &meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                fetch.table_ticks = Some(column.get_added_ticks_slice())
            }
        }
        // At least one of the components must be present in the table.
        debug_unreachable()
    }

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut Self::Fetch<'w>,
        component_id: &ComponentId,
        _archetype: &'w Archetype,
        table: &'w Table,
    ) {
        if Self::IS_DENSE {
            Self::set_table(fetch, component_id, table);
        }
    }
}
// #[inline(always)]
// unsafe fn fetch<'w>(
// fetch: &mut Self::Fetch<'w>,
// entity: Entity,
// table_row: usize
// ) -> Self::Item<'w> {
// match T::Storage::STORAGE_TYPE {
// StorageType::Table => {
// fetch
// .table_ticks
// .debug_checked_unwrap()
// .get(table_row)
// .deref()
// .is_older_than(fetch.last_change_tick, fetch.change_tick)
// }
// StorageType::SparseSet => {
// let sparse_set = &fetch
// .sparse_set
// .debug_checked_unwrap();
// $get_sparse_set(sparse_set, entity)
// .debug_checked_unwrap()
// .deref()
// .is_older_than(fetch.last_change_tick, fetch.change_tick)
// }
// }
// }
//
// #[inline(always)]
// unsafe fn filter_fetch<'w>(
// fetch: &mut Self::Fetch<'w>,
// entity: Entity,
// table_row: usize
// ) -> bool {
// Self::fetch(fetch, entity, table_row)
// }
//
// #[inline]
// fn update_component_access(&id: &ComponentId, access: &mut FilteredAccess<ComponentId>) {
// if access.access().has_write(id) {
// panic!("$state_name<{}> conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
// std::any::type_name::<T>());
// }
// access.add_read(id);
// }
//
// #[inline]
// fn update_archetype_component_access(
// &id: &ComponentId,
// archetype: &Archetype,
// access: &mut Access<ArchetypeComponentId>,
// ) {
// if let Some(archetype_component_id) = archetype.get_archetype_component_id(id) {
// access.add_read(archetype_component_id);
// }
// }
//
// fn init_state(world: &mut World) -> ComponentId {
// world.init_component::<T>()
// }
//
// fn matches_component_set(&id: &ComponentId, set_contains_id: &impl Fn(ComponentId) -> bool) -> bool {
// set_contains_id(id)
// }
// }
//
// /// SAFETY: read-only access
// unsafe impl<'a, Trait: ?Sized + TraitQuery> ReadOnlyWorldQuery for Added<&'a Trait> {}
