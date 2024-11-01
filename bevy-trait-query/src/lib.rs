//! <style>
//! .rustdoc-hidden { display: none; }
//! </style>
#![doc = include_str!("../../README.md")]

use bevy_ecs::{
    component::{ComponentId, StorageType},
    prelude::{Component, Resource, World},
    ptr::{Ptr, PtrMut},
};

#[cfg(test)]
mod tests;

pub mod all;
pub mod one;

pub use all::*;
pub use one::*;

/// Marker for traits that can be used in queries.
pub trait TraitQuery: 'static {}

pub use bevy_trait_query_impl::queryable;

#[doc(hidden)]
pub trait TraitQueryMarker<Trait: ?Sized + TraitQuery> {
    type Covered: Component;
    /// Casts an untyped pointer to a trait object pointer,
    /// with a vtable corresponding to `Self::Covered`.
    fn cast(_: *mut u8) -> *mut Trait;
}

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

#[derive(Resource)]
struct TraitImplRegistry<Trait: ?Sized> {
    // Component IDs are stored contiguously so that we can search them quickly.
    components: Vec<ComponentId>,
    meta: Vec<TraitImplMeta<Trait>>,

    table_components: Vec<ComponentId>,
    table_meta: Vec<TraitImplMeta<Trait>>,

    sparse_components: Vec<ComponentId>,
    sparse_meta: Vec<TraitImplMeta<Trait>>,

    sealed: bool,
}

impl<T: ?Sized> Default for TraitImplRegistry<T> {
    #[inline]
    fn default() -> Self {
        Self {
            components: vec![],
            meta: vec![],
            table_components: vec![],
            table_meta: vec![],
            sparse_components: vec![],
            sparse_meta: vec![],
            sealed: false,
        }
    }
}

impl<Trait: ?Sized + TraitQuery> TraitImplRegistry<Trait> {
    fn register<C: Component>(&mut self, component: ComponentId, meta: TraitImplMeta<Trait>) {
        // Don't register the same component multiple times.
        if self.components.contains(&component) {
            return;
        }

        if self.sealed {
            // It is not possible to update the `FetchState` for a given system after the game has started,
            // so for explicitness, let's panic instead of having a trait impl silently get forgotten.
            panic!("Cannot register new trait impls after the game has started");
        }

        self.components.push(component);
        self.meta.push(meta);

        match <C as Component>::STORAGE_TYPE {
            StorageType::Table => {
                self.table_components.push(component);
                self.table_meta.push(meta);
            }
            StorageType::SparseSet => {
                self.sparse_components.push(component);
                self.sparse_meta.push(meta);
            }
        }
    }

    fn seal(&mut self) {
        self.sealed = true;
    }
}

/// Stores data about an impl of a trait
struct TraitImplMeta<Trait: ?Sized> {
    size_bytes: usize,
    dyn_ctor: DynCtor<Trait>,
}

impl<T: ?Sized> Copy for TraitImplMeta<T> {}
impl<T: ?Sized> Clone for TraitImplMeta<T> {
    fn clone(&self) -> Self {
        *self
    }
}

#[doc(hidden)]
pub mod imports {
    pub use bevy_ecs::{
        archetype::{Archetype, ArchetypeComponentId},
        component::Tick,
        component::{Component, ComponentId, Components},
        entity::Entity,
        query::{
            Access, Added, Changed, FilteredAccess, QueryData, QueryFilter, QueryItem,
            ReadOnlyQueryData, WorldQuery,
        },
        storage::{Table, TableRow},
        world::{unsafe_world_cell::UnsafeWorldCell, World},
    };
}

#[doc(hidden)]
pub struct TraitQueryState<Trait: ?Sized> {
    components: Box<[ComponentId]>,
    meta: Box<[TraitImplMeta<Trait>]>,
}

impl<Trait: ?Sized + TraitQuery> TraitQueryState<Trait> {
    fn init(world: &mut World) -> Self {
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

    // // REVIEW: inline?
    // // REVIEW: does it make sense to use the optional return type here? The call sites would be
    // // happy with this so I just made it use `Option`
    // fn get(world: &World) -> Option<Self> {
    //     // REVIEW: is it ok to use the optional version here?
    //     let registry = world.get_resource::<TraitImplRegistry<Trait>>()?;
    //     // REVIEW: do we really need to clone here on get calls?
    //     Some(Self {
    //         components: registry.components.clone().into_boxed_slice(),
    //         meta: registry.meta.clone().into_boxed_slice(),
    //     })
    // }

    #[inline]
    fn matches_component_set_any(&self, set_contains_id: &impl Fn(ComponentId) -> bool) -> bool {
        self.components.iter().copied().any(set_contains_id)
    }

    #[inline]
    fn matches_component_set_one(&self, set_contains_id: &impl Fn(ComponentId) -> bool) -> bool {
        let match_count = self
            .components
            .iter()
            .filter(|&&c| set_contains_id(c))
            .count();
        match_count == 1
    }
}

/// Turns an untyped pointer into a trait object pointer,
/// for a specific erased concrete type.
struct DynCtor<Trait: ?Sized> {
    cast: unsafe fn(*mut u8) -> *mut Trait,
}

impl<T: ?Sized> Copy for DynCtor<T> {}
impl<T: ?Sized> Clone for DynCtor<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Trait: ?Sized> DynCtor<Trait> {
    #[inline]
    unsafe fn cast(self, ptr: Ptr) -> &Trait {
        &*(self.cast)(ptr.as_ptr())
    }
    #[inline]
    unsafe fn cast_mut(self, ptr: PtrMut) -> &mut Trait {
        &mut *(self.cast)(ptr.as_ptr())
    }
}

struct ZipExact<A, B> {
    a: A,
    b: B,
}

impl<A: Iterator, B: Iterator> Iterator for ZipExact<A, B> {
    type Item = (A::Item, B::Item);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let a = self.a.next()?;
        let b = self
            .b
            .next()
            // SAFETY: `a` returned a valid value, and the caller of `zip_exact`
            // guaranteed that `b` will return a value as long as `a` does.
            .unwrap_or_else(|| unsafe { debug_unreachable() });
        Some((a, b))
    }
}

/// SAFETY: `b` must yield at least as many items as `a`.
#[inline]
unsafe fn zip_exact<A: IntoIterator, B: IntoIterator>(
    a: A,
    b: B,
) -> ZipExact<A::IntoIter, B::IntoIter>
where
    A::IntoIter: ExactSizeIterator,
    B::IntoIter: ExactSizeIterator,
{
    let a = a.into_iter();
    let b = b.into_iter();
    debug_assert_eq!(a.len(), b.len());
    ZipExact { a, b }
}

#[track_caller]
#[inline(always)]
unsafe fn debug_unreachable() -> ! {
    #[cfg(debug_assertions)]
    unreachable!();

    #[cfg(not(debug_assertions))]
    std::hint::unreachable_unchecked();
}

#[inline(never)]
#[cold]
fn trait_registry_error() -> ! {
    panic!("The trait query registry has not been initialized; did you forget to register your traits with the world?")
}
