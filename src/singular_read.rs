use crate::TraitImplMeta;
use bevy::ecs::component::ComponentTicks;
use bevy::ecs::storage::{ComponentSparseSet, SparseSets};
use bevy::ptr::{Ptr, ThinSlicePtr};
use std::cell::UnsafeCell;

pub struct ReadTraitFetch<'w, Trait: ?Sized> {
    // While we have shared access to all sparse set components,
    // in practice we will only read the components specified in the `FetchState`.
    // These accesses have been registered, which prevents runtime conflicts.
    pub(crate) sparse_sets: &'w SparseSets,
    // After `Fetch::set_archetype` or `set_table` has been called,
    // this will carry the component data and metadata for the first trait impl found in the archetype.
    pub(crate) storage: ReadStorage<'w, Trait>,
}

pub(crate) enum ReadStorage<'w, Trait: ?Sized> {
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
