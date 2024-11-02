use bevy_ecs::{
    component::Tick,
    storage::{SparseSets, Table},
};

use crate::TraitImplRegistry;

#[doc(hidden)]
pub struct AllTraitsFetch<'w, Trait: ?Sized> {
    pub(crate) registry: &'w TraitImplRegistry<Trait>,
    pub(crate) table: Option<&'w Table>,
    pub(crate) sparse_sets: &'w SparseSets,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

impl<Trait: ?Sized> Clone for AllTraitsFetch<'_, Trait> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<Trait: ?Sized> Copy for AllTraitsFetch<'_, Trait> {}
