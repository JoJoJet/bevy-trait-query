# bevy-trait-query

<!-- cargo-rdme start -->

An implementation of trait queries for the bevy game engine.

Before using this crate, you should be familiar with bevy: https://bevyengine.org/.
The current published version depends on bevy 0.8, although there is a branch on github
that supports the upcoming version.

## Note on reliability

This crate is experimental, and not battle-tested. It seems to work in my personal testing,
but it very well could contain undefined behavior. Use with caution (and miri!).

If you find a bug, please [open an issue](https://github.com/JoJoJet/bevy-trait-query/issues).

## Overview

`bevy-trait-query` extends the capabilities of `bevy` by allowing you to query for components implementing a trait.

```rust
use bevy::prelude::*;

// Some trait that we wish to use in queries.
pub trait Tooltip: 'static {
    fn tooltip(&self) -> &str;
}

// Add the necessary impls for querying.
bevy_trait_query::impl_trait_query!(Tooltip);

// Define some custom components.

#[derive(Component)]
struct Person(String);

#[derive(Component)]
struct Monster;

// Implement the trait for these components.

impl Tooltip for Person {
    fn tooltip(&self) -> &str {
        &self.0
    }
}

impl Tooltip for Monster {
    fn tooltip(&self) -> &str {
        "Run!"
    }
}

fn main() {
    // We must import this trait in order to register our trait impls.
    // If we don't register them, they will be invisible to the game engine.
    use bevy_trait_query::RegisterExt;

    App::new()
        // Register our components.
        .register_component_as::<dyn Tooltip, Person>()
        .register_component_as::<dyn Tooltip, Monster>()
        .add_startup_system(setup)
        .add_system(show_tooltip)
        .add_system(show_all_tooltips)
}

fn setup(mut commands: Commands) {
    commands.spawn().insert(Person("Fourier".to_owned()));
    commands.spawn().insert(Monster);
}

fn show_tooltips(
    // Query for entities with components implementing the trait.
    query: Query<&dyn Tooltip>,
) {
    for entity_tooltips in &query {
        // It's possible for an entity to have more than one component implementing the trait,
        // so we must iterate over all possible components for each entity.
        for tooltip: &dyn Tooltip in entity_tooltips {
            println!("Hovering: {}", tooltip.tooltip());
        }
    }
}


use bevy_trait_query::One;
fn show_tooltips_one(
    // If you expect to only have one trait impl per entity, you should use the `One` filter.
    // This is significantly more efficient than iterating over all trait impls.
    query: Query<One<&dyn Tooltip>>,
) {
    for tooltip: &dyn Tooltip in &query {
        println!("Hovering: {}", tooltip.tooltip());
    }
}
```

## Performance

The performance of trait queries is quite competitive. Here are some benchmarks for simple cases:

|                   | Concrete type | One<dyn Trait> | All<dyn Trait> |
|-------------------|----------------|-------------------|-----------------|
| 1 match           | 16.135 µs      | 31.441 µs         | 63.273 µs       |
| 2 matches         | 17.501 µs      | -                 | 102.83 µs       |
| 1-2 matches       | -              | 16.959 µs         | 82.179 µs       |

## Poor use cases

You should avoid using trait queries for very simple cases that can be solved with more direct solutions.

One naive use would be querying for a trait that looks something like:

```rust
trait Person {
    fn name(&self) -> &str;
}
```

A far better way of expressing this would be to store the name in a separate component
and query for that directly, making `Person` a simple marker component.

Trait queries are often the most *obvious* solution to a problem, but not always the best one.
For examples of strong real-world use-cases, check out the RFC for trait queries in `bevy`:
https://github.com/bevyengine/rfcs/pull/39.

<!-- cargo-rdme end -->

# License

[MIT](LICENSE-MIT) or [APACHE-2.0](LICENSE-APACHE)
