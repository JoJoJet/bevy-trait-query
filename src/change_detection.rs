use crate::{TraitImplRegistry, TraitQuery, TraitQueryState};
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

pub(crate) enum ChangeDetectionMode {
    None,
    Added,
    Changed
}