use std::cell::UnsafeCell;

use bevy_ecs::{
    component::Tick,
    ptr::ThinSlicePtr,
    storage::{ComponentSparseSet, SparseSets},
};

#[derive(Clone, Copy)]
pub(crate) enum ChangeDetectionStorage<'w> {
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
#[derive(Clone, Copy)]
pub struct ChangeDetectionFetch<'w> {
    pub(crate) storage: ChangeDetectionStorage<'w>,
    pub(crate) sparse_sets: &'w SparseSets,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}
