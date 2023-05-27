//! Let's say you have a trait that you want to implement for some of your components.
//!
//! ```
//! # use bevy::prelude::*;
//! #
//! /// Components that display a message when hovered.
//! pub trait Tooltip {
//!     /// Text displayed when hovering over an entity with this trait.
//!     fn tooltip(&self) -> &str;
//! }
//! ```
//!
//! In order to be useful within bevy, you'll want to be able to query for this trait.
//!
//! ```ignore
//! # use bevy::prelude::*;
//! # // Required to make the macro work, because cargo thinks
//! # // we are in `bevy_trait_query` when compiling this example.
//! # use bevy_trait_query::*;
//!
//! // Just add this attribute...
//! #[bevy_trait_query::queryable]
//! pub trait Tooltip {
//!     fn tooltip(&self) -> &str;
//! }
//!
//! // ...and now you can use your trait in queries.
//! fn show_tooltips_system(
//!     tooltips: Query<&dyn Tooltip>,
//!     // ...
//! ) {
//!     // ...
//! }
//! # bevy::ecs::system::assert_is_system(show_tooltips_system);
//! ```
//!
//! Since Rust unfortunately lacks any kind of reflection, it is necessary to register each
//! component with the trait when the app gets built.
//!
//! ```
//! # use bevy::prelude::*;
//! # use bevy_trait_query::*;
//! #
//! # #[bevy_trait_query::queryable]
//! # pub trait Tooltip {
//! #     fn tooltip(&self) -> &str;
//! # }
//! #
//! #[derive(Component)]
//! struct Player(String);
//!
//! #[derive(Component)]
//! enum Villager {
//!     Farmer,
//!     // ...
//! }
//!
//! #[derive(Component)]
//! struct Monster;
//!
//! /* ...trait implementations omitted for brevity... */
//!
//! # impl Tooltip for Player {
//! #     fn tooltip(&self) -> &str {
//! #         &self.0
//! #     }
//! # }
//! #
//! # impl Tooltip for Villager {
//! #     fn tooltip(&self) -> &str {
//! #         "Villager"
//! #     }
//! # }
//! #
//! # impl Tooltip for Monster {
//! #     fn tooltip(&self) -> &str {
//! #         "Run!"
//! #     }
//! # }
//! #
//! struct TooltipPlugin;
//!
//! impl Plugin for TooltipPlugin {
//!     fn build(&self, app: &mut App) {
//!         // We must import this trait in order to register our components.
//!         // If we don't register them, they will be invisible to the game engine.
//!         use bevy_trait_query::RegisterExt;
//!
//!         app
//!             .register_component_as::<dyn Tooltip, Player>()
//!             .register_component_as::<dyn Tooltip, Villager>()
//!             .register_component_as::<dyn Tooltip, Monster>()
//!             .add_systems(Update, show_tooltips);
//!     }
//! }
//! # fn show_tooltips() {}
//! #
//! # fn main() {
//! #     App::new().add_plugins(DefaultPlugins).add_plugin(TooltipPlugin).update();
//! # }
//! ```
//!
//! Unlike queries for concrete types, it's possible for an entity to have multiple components
//! that match a trait query.
//!
//! ```
//! # use bevy::prelude::*;
//! # use bevy_trait_query::*;
//! #
//! # #[bevy_trait_query::queryable]
//! # pub trait Tooltip {
//! #     fn tooltip(&self) -> &str;
//! # }
//! #
//! # #[derive(Component)]
//! # struct Player(String);
//! #
//! # #[derive(Component)]
//! # struct Monster;
//! #
//! # impl Tooltip for Player {
//! #     fn tooltip(&self) -> &str {
//! #         &self.0
//! #     }
//! # }
//! #
//! # impl Tooltip for Monster {
//! #     fn tooltip(&self) -> &str {
//! #         "Run!"
//! #     }
//! # }
//! #
//! # fn main() {
//! #     App::new()
//! #         .add_plugins(DefaultPlugins)
//! #         .register_component_as::<dyn Tooltip, Player>()
//! #         .register_component_as::<dyn Tooltip, Monster>()
//! #         .add_systems(Startup, setup)
//! #         .update();
//! # }
//! #
//! # fn setup(mut commands: Commands) {
//! #     commands.spawn(Player("Fourier".to_owned()));
//! #     commands.spawn(Monster);
//! # }
//!
//! fn show_tooltips(
//!     tooltips: Query<&dyn Tooltip>,
//!     // ...
//! ) {
//!     // Iterate over each entity that has tooltips.
//!     for entity_tooltips in &tooltips {
//!         // Iterate over each component implementing `Tooltip` for the current entity.
//!         for tooltip in entity_tooltips {
//!             println!("Tooltip: {}", tooltip.tooltip());
//!         }
//!     }
//!
//!     // If you instead just want to iterate over all tooltips, you can do:
//!     for tooltip in tooltips.iter().flatten() {
//!         println!("Tooltip: {}", tooltip.tooltip());
//!     }
//! }
//! ```
//!
//! Alternatively, if you expect to only have component implementing the trait for each entity,
//! you can use the filter [`One`](crate::one::One). This has significantly better performance than iterating
//! over all trait impls.
//!
//! ```ignore
//! # use bevy::prelude::*;
//! # use bevy_trait_query::*;
//! #
//! # #[bevy_trait_query::queryable]
//! # pub trait Tooltip {
//! #     fn tooltip(&self) -> &str;
//! # }
//! #
//! use bevy_trait_query::One;
//!
//! fn show_tooltips(
//!     tooltips: Query<One<&dyn Tooltip>>,
//!     // ...
//! ) {
//!     for tooltip in &tooltips {
//!         println!("Tooltip: {}", tooltip.tooltip());
//!     }
//! }
//! # bevy::ecs::system::assert_is_system(show_tooltips);
//! ```
//!
//! Trait queries support basic change detection filtration. So to get all the components that
//! implement the target trait, and have also changed in some way since the last tick, you can:
//! ```ignore
//! fn show_tooltips(
//!     tooltips_query: Query<ChangedAll<&dyn Tooltip>>
//!     // ...
//! ) {
//!     // Iterate over each entity that has tooltips, which have *changed* since the last tick
//!     for entity_tooltips in &tooltips_query {
//!         // Iterate over each component implementing `Tooltip` for the current entity.
//!         for tooltip in entity_tooltips {
//!             println!("Changed Tooltip: {}", tooltip.tooltip());
//!         }
//!     }
//! }
//! ```
//!
//! Similar to `ChangedAll`, we have `AddedAll`.
//!
//! If you know you have only one component that implements the target trait, you can use
//! `ChangedOne` (or `AddedOne`) filters, which returns an `Option`al entity if change was detected:
//! ```ignore
//! fn show_tooltips(
//!     tooltips_query: Query<ChangedOne<&dyn Tooltip>>
//!     // ...
//! ) {
//!     // Iterate over each entity that has one tooltip implementing component
//!     for maybe_changed_tooltip in &tooltips_query {
//!         if let Some(changed_tooltip) = maybe_changed_tooltip {
//!             println!("Changed Tooltip: {}", tooltip.tooltip());
//!         }
//!     }
//! }
//! ```
//!
//! or you can use `OneAddedFilter` or `OneChangedFilter` which behave more like the typical
//! `bevy` `Added/Changed` filters:
//! ```ignore
//! fn show_tooltips(
//!     tooltips_query: Query<One<&dyn Tooltip>, OneChangedFilter<dyn Tooltip>>
//!     // ...
//! ) {
//!     // Iterate over each entity that has one tooltip implementing component that has also changed
//!     for changed_tooltip in &tooltips_query {
//!         println!("Changed Tooltip: {}", tooltip.tooltip());
//!     }
//! }
//! ```
//! Note in the above example how `OneChangedFilter` does *not* take a reference to the trait object!
//!
//! # Performance
//!
//! The performance of trait queries is quite competitive. Here are some benchmarks for simple cases:
//!
//! |                   | Concrete type | One<dyn Trait> | All<dyn Trait> |
//! |-------------------|----------------|-------------------|-----------------|
//! | 1 match           | 16.135 µs      | 31.441 µs         | 63.273 µs       |
//! | 2 matches         | 17.501 µs      | -                 | 102.83 µs       |
//! | 1-2 matches       | -              | 16.959 µs         | 82.179 µs       |
//!
use bevy::{
    ecs::{
        component::{ComponentId, StorageType},
        world::World,
    },
    prelude::*,
    ptr::{Ptr, PtrMut},
};

#[cfg(test)]
mod tests;

pub mod all;
pub mod one;
mod query_filter;

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
            .get_resource_or_insert_with::<TraitImplRegistry<Trait>>(default)
            .into_inner();
        let meta = TraitImplMeta {
            size_bytes: std::mem::size_of::<C>(),
            dyn_ctor: DynCtor { cast: <(C,)>::cast },
        };
        registry.register::<C>(component_id, meta);
        self
    }
}

impl RegisterExt for App {
    fn register_component_as<Trait: ?Sized + TraitQuery, C: Component>(&mut self) -> &mut Self
    where
        (C,): TraitQueryMarker<Trait, Covered = C>,
    {
        self.world.register_component_as::<Trait, C>();
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

        use bevy::ecs::component::ComponentStorage;
        match <C as Component>::Storage::STORAGE_TYPE {
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
    pub use bevy::ecs::{
        archetype::{Archetype, ArchetypeComponentId},
        component::Tick,
        component::{Component, ComponentId},
        entity::Entity,
        query::{
            Access, Added, Changed, FilteredAccess, QueryItem, ReadOnlyWorldQuery, WorldQuery,
        },
        storage::{Table, TableRow},
        world::World,
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
            warn!(
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
