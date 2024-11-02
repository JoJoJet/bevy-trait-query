use std::cell::UnsafeCell;

use bevy_ecs::{
    component::Tick,
    ptr::{Ptr, ThinSlicePtr},
    storage::{ComponentSparseSet, SparseSets},
};

use crate::TraitImplMeta;

pub struct OneTraitFetch<'w, Trait: ?Sized> {
    // While we have shared access to all sparse set components,
    // in practice we will only access the components specified in the `FetchState`.
    // These accesses have been registered, which prevents runtime conflicts.
    pub(crate) sparse_sets: &'w SparseSets,
    // After `Fetch::set_archetype` or `set_table` has been called,
    // this will carry the component data and metadata for the first trait impl found in the archetype.
    pub(crate) storage: FetchStorage<'w, Trait>,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

impl<Trait: ?Sized> Clone for OneTraitFetch<'_, Trait> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Trait: ?Sized> Copy for OneTraitFetch<'_, Trait> {}

pub(crate) enum FetchStorage<'w, Trait: ?Sized> {
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
