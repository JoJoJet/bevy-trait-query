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
//! ```
//! # use bevy::prelude::*;
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
//! # bevy_ecs::system::assert_is_system(show_tooltips_system);
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
//! #     App::new().add_plugins((DefaultPlugins, TooltipPlugin)).update();
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
//! ```
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
//! # bevy_ecs::system::assert_is_system(show_tooltips);
//! ```
//!
//! Trait queries support basic change detection filtration.
//!
//! - queries requesting shared access yield [`ReadTraits`](`crate::all::ReadTraits`) which is
//!   similar to [`Ref`](https://docs.rs/bevy/latest/bevy/ecs/change_detection/struct.Ref.html)
//! - queries requesting exclusive access yield [`WriteTraits`](`crate::all::WriteTraits`) which is
//!   similar to [`Mut`](https://docs.rs/bevy/latest/bevy/ecs/change_detection/struct.Mut.html)
//!
//! To get all the components that implement the target trait, and have also changed in some way
//! since the last tick, you can:
//! ```no_run
//! # use bevy::prelude::*;
//! # use bevy_trait_query::*;
//! #
//! # #[bevy_trait_query::queryable]
//! # pub trait Tooltip {
//! #     fn tooltip(&self) -> &str;
//! # }
//! #
//! fn show_tooltips(
//!     tooltips_query: Query<All<&dyn Tooltip>>
//!     // tooltips_query: Query<&dyn Tooltip>  // <-- equivalent to line above
//!     // ...
//! ) {
//!     // Iterate over all entities with at least one component implementing `Tooltip`
//!     for entity_tooltips in &tooltips_query {
//!         // Iterate over each component for the current entity that changed since the last time the system was run.
//!         for tooltip in entity_tooltips.iter_changed() {
//!             println!("Changed Tooltip: {}", tooltip.tooltip());
//!         }
//!     }
//! }
//! ```
//!
//! Similar to [`iter_changed`](crate::all::ReadTraits), we have [`iter_added`](crate::all::ReadTraits)
//! to detect entities which have had a trait-implementing component added since the last tick.
//!
//! If you know you have only one component that implements the target trait,
//! you can use [`OneAdded`](crate::one::OneAdded) or [`OneChanged`](crate::one::OneChanged) which behave more like the typical
//! `bevy` [`Added`](https://docs.rs/bevy/latest/bevy/ecs/query/struct.Added.html)/[`Changed`](https://docs.rs/bevy/latest/bevy/ecs/query/struct.Changed.html) filters:
//! ```no_run
//! # use bevy::prelude::*;
//! # use bevy_trait_query::*;
//! #
//! # #[bevy_trait_query::queryable]
//! # pub trait Tooltip {
//! #     fn tooltip(&self) -> &str;
//! # }
//! #
//! fn show_tooltips(
//!     tooltips_query: Query<One<&dyn Tooltip>, OneChanged<dyn Tooltip>>
//!     // ...
//! ) {
//!     // Iterate over each entity that has one tooltip implementing component that has also changed
//!     for tooltip in &tooltips_query {
//!         println!("Changed Tooltip: {}", tooltip.tooltip());
//!     }
//! }
//! ```
//! Note in the above example how [`OneChanged`](crate::one::OneChanged) does *not* take a reference to the trait object!
//!
//! # Performance
//!
//! The performance of trait queries is quite competitive. Here are some benchmarks for simple cases:
//!
//! |                   | Concrete type  | [`One<dyn Trait>`](crate::one::One)    | [`All<dyn Trait>`](crate::all::All) |
//! |-------------------|----------------|---------------------|-------------------|
//! | 1 match           | 8.395 µs       | 28.174 µs           | 81.027 µs         |
//! | 2 matches         | 8.473 µs       | -                   | 106.47 µs         |
//! | 1-2 matches       | -              | 14.619 µs           | 92.876 µs         |
//!

mod internal;
#[cfg(test)]
mod tests;

pub mod all;
pub mod one;

pub use all::*;
pub use internal::*;
pub use one::*;

pub use bevy_trait_query_impl::queryable;

// used by proc macro crate, it's important to keep these things as they are. Only make changes if
// you know what you're doing
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
