use bevy_ecs::component::ComponentId;
use bevy_ecs::prelude::World;

use crate::{
    trait_registry::{TraitImplMeta, TraitImplRegistry},
    TraitQuery,
};

#[doc(hidden)]
pub struct TraitQueryState<Trait: ?Sized> {
    pub(crate) components: Box<[ComponentId]>,
    pub(crate) meta: Box<[TraitImplMeta<Trait>]>,
}

impl<Trait: ?Sized + TraitQuery> TraitQueryState<Trait> {
    pub(crate) fn init(world: &mut World) -> Self {
        #[cold]
        fn missing_registry<T: ?Sized + 'static>() -> TraitImplRegistry<T> {
            tracing::warn!(
                "no components found matching `{}`, did you forget to register them?",
                std::any::type_name::<T>()
            );
            TraitImplRegistry::<T>::default()
        }

        let mut registry = world.get_resource_or_insert_with(missing_registry);
        registry.seal();
        Self {
            components: registry.components.clone().into_boxed_slice(),
            meta: registry.meta.clone().into_boxed_slice(),
        }
    }

    #[inline]
    pub(crate) fn matches_component_set_any(
        &self,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        self.components.iter().copied().any(set_contains_id)
    }

    #[inline]
    pub(crate) fn matches_component_set_one(
        &self,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        let match_count = self
            .components
            .iter()
            .filter(|&&c| set_contains_id(c))
            .count();
        match_count == 1
    }
}
