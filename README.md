# bevy-trait-query

<!-- cargo-rdme start -->

An implementation of trait queries for the bevy game engine.

Before using this crate, you should be familiar with bevy: https://bevyengine.org/.

This crate is implementation of the following RFC: https://github.com/bevyengine/rfcs/pull/39.

## Note on reliability

This crate is highly experimental (read: not battle tested). It seems to work in my testing,
but it very well could invoke undefined behavior when run. Use with caution (and miri!).

If you find a bug, please [open an issue](https://github.com/JoJoJet/bevy-trait-query/issues).

## Overview

`bevy-trait-query` extends the capabilities of `bevy` by allowing you to query for components implementing a trait.

```rust
use bevy::prelude::*;
use bevy_trait_query::{impl_trait_query, RegisterExt};

// Trait for entities that should show text when the mouse hovers over them.
pub trait Tooltip: 'static {
    fn tooltip(&self) -> &str;
}
impl_trait_query!(Tooltip);

#[derive(Component)]
struct Person(String);

impl Tooltip for Person {
    fn tooltip(&self) -> &str {
        &self.0
    }
}

#[derive(Component)]
struct Monster;

impl Tooltip for Monster {
    fn tooltip(&self) -> &str {
        "Run!"
    }
}

fn main() {
    App::new()
        // We must register each trait impl, otherwise they are invisible to the game engine.
        .register_component_as::<dyn Tooltip, Person>()
        .register_component_as::<dyn Tooltip, Monster>()
        .add_startup_system(setup)
        .add_system(show_tooltip)
}

fn setup(mut commands: Commands) {
    commands.spawn().insert(Person("Fourier".to_owned()));
    commands.spawn().insert(Monster);
}

fn show_tooltip(
    query: Query<&dyn Tooltip>,
    // ...
) {
    for tt in &query {
        let mouse_hovered = {
            // ...
        };
        if mouse_hovered {
            println!("{}", tt.tooltip())
        }
    }
    // Prints 'Fourier', 'Run!'.
}
```

Note that `&dyn Trait` and `&mut dyn Trait` are referred to as "existential" queries,
which means that they will only return one implementation of the trait for a given entity.

If you expect to have multiple components implementing the trait for a given entity,
you should instead use "universal" queries: `All<&dyn Trait>`, `All<&mut dyn Trait>`.
These queries will return every component implementing `Trait` for each entity.

## Performance

The performance of trait queries is quite competitive. Here are some benchmarks for simple cases:

|                   | Concrete type | Trait-existential | Trait-universal |
|-------------------|----------------|-------------------|-----------------|
| 1 match           | 16.931 µs      | 29.692 µs         | 63.095 µs       |
| 2 matches         | 17.508 µs      | 30.859 µs         | 101.88 µs       |
| 1-2 matches       | -              | 28.840 µs         | 83.035 µs       |

On the nightly branch, performance is comparable to concrete queries:

|                   | Concrete type | Trait-existential | Trait-universal |
|-------------------|----------------|-------------------|-----------------|
| 1 match           | 17.017 µs      | 20.432 µs         | 61.896 µs       |
| 2 matches         | 17.560 µs      | 21.556 µs         | 90.160 µs       |
| 1-2 matches       | -              | 22.247 µs         | 75.418 µs       |

<!-- cargo-rdme end -->

# License

MIT or APACHE-2.0
