# bevy-trait-query

An implementation of trait queries for the bevy game engine.

Before using this crate, you should be familiar with bevy: https://bevyengine.org/.

| Bevy Version | [Crate Version](CHANGELOG.md) |
|--------------|---------------|
| 0.9          | 0.1           |
| 0.8          | 0.0.3         |
| Preview      | Main branch   |

## Note on reliability

While this crate has seen some use in the world with no issues yet,
it is still quite new and experimental. Use with caution (and miri!).

If you find a bug, please [open an issue](https://github.com/JoJoJet/bevy-trait-query/issues).

<!-- cargo-rdme start -->

## Overview

`bevy-trait-query` extends the capabilities of `bevy` by allowing you to query for components implementing a trait.

```rust
use bevy::prelude::*;

// Some trait that we wish to use in queries.

#[bevy_trait_query::queryable]
pub trait Tooltip {
    fn tooltip(&self) -> &str;
}

// Define some custom components which will implement the trait.

#[derive(Component)]
struct Person(String);

#[derive(Component)]
struct Monster;

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
        .add_system(show_tooltips)
        .add_system(show_tooltips_one)
}

fn setup(mut commands: Commands) {
    commands.spawn(Person("Fourier".to_owned()));
    commands.spawn(Monster);
}

fn show_tooltips(
    // Query for entities with components implementing the trait.
    query: Query<&dyn Tooltip>,
) {
    for entity_tooltips in &query {
        // It's possible for an entity to have more than one component implementing the trait,
        // so we must iterate over all possible components for each entity.
        for tooltip in entity_tooltips {
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
    for tooltip in &query {
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

<!-- cargo-rdme end -->

# License

[MIT](LICENSE-MIT) or [APACHE-2.0](LICENSE-APACHE)
