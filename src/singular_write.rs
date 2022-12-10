use crate::TraitImplMeta;
use bevy::ecs::component::ComponentTicks;
use bevy::ecs::storage::{ComponentSparseSet, SparseSets};
use bevy::ptr::{Ptr, ThinSlicePtr};
use std::cell::UnsafeCell;

#[doc(hidden)]
pub struct WriteTraitFetch<'w, Trait: ?Sized> {
    // While we have shared mutable access to all sparse set components,
    // in practice we will only modify the components specified in the `FetchState`.
    // These accesses have been registered, which prevents runtime conflicts.
    pub(crate) sparse_sets: &'w SparseSets,

    // After `Fetch::set_archetype` or `set_table` has been called,
    // this will carry the component data and metadata for the first trait impl found in the archetype.
    pub(crate) storage: WriteStorage<'w, Trait>,

    pub(crate) last_change_tick: u32,
    pub(crate) change_tick: u32,
}

pub(crate) enum WriteStorage<'w, Trait: ?Sized> {
    Uninit,
    Table {
        /// This is a shared mutable pointer to one of the component table columns,
        /// corresponding to one of the `ComponentId`s in the fetch state.
        /// The fetch impl registers write access for all of these components,
        /// so there will be no runtime conflicts.
        column: Ptr<'w>,
        table_ticks: ThinSlicePtr<'w, UnsafeCell<ComponentTicks>>,
        meta: TraitImplMeta<Trait>,
    },
    SparseSet {
        /// This gives us shared mutable access to one of the components implementing the trait.
        /// The fetch impl registers write access for all components implementing the trait, so there will be no runtime conflicts.
        components: &'w ComponentSparseSet,
        meta: TraitImplMeta<Trait>,
    },
}
