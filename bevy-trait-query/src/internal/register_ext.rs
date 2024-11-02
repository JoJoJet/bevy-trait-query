use crate::{
    dyn_constructor::DynCtor, TraitImplMeta, TraitImplRegistry, TraitQuery, TraitQueryMarker,
};
use bevy_ecs::prelude::{Component, World};

/// Extension methods for registering components with trait queries.
pub trait RegisterExt {
    /// Allows a component to be used in trait queries.
    /// Calling this multiple times with the same arguments will do nothing on subsequent calls.
    ///
    /// # Panics
    /// If this function is called after the simulation starts for a given [`World`].
    /// Due to engine limitations, registering new trait impls after the game starts cannot be supported.
    fn register_component_as<Trait: ?Sized + TraitQuery, C: Component>(&mut self) -> &mut Self
    where
        (C,): TraitQueryMarker<Trait, Covered = C>;
}

impl RegisterExt for World {
    fn register_component_as<Trait: ?Sized + TraitQuery, C: Component>(&mut self) -> &mut Self
    where
        (C,): TraitQueryMarker<Trait, Covered = C>,
    {
        let component_id = self.init_component::<C>();
        let registry = self
            .get_resource_or_insert_with::<TraitImplRegistry<Trait>>(Default::default)
            .into_inner();
        let meta = TraitImplMeta {
            size_bytes: std::mem::size_of::<C>(),
            dyn_ctor: DynCtor { cast: <(C,)>::cast },
        };
        registry.register::<C>(component_id, meta);
        self
    }
}

#[cfg(feature = "bevy_app")]
impl RegisterExt for bevy_app::App {
    fn register_component_as<Trait: ?Sized + TraitQuery, C: Component>(&mut self) -> &mut Self
    where
        (C,): TraitQueryMarker<Trait, Covered = C>,
    {
        self.world_mut().register_component_as::<Trait, C>();
        self
    }
}
