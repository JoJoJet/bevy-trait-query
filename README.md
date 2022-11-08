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

## Overview

<!-- cargo-rdme start -->

Lets say you have a trait that you wanna implement for some of your components.

```rust
/// Components that display a message when hovered.
pub trait Tooltip {
    /// Text displayed when hovering over an entity with this trait.
    fn tooltip(&self) -> &str;
}

// ahhh scary
#[derive(Component)]
struct Monster;

impl Tooltip for Monster {
    fn tooltip(&self) -> &str {
        "Run!"
    }
}
```

In order to to be useful within bevy, you'll want to be able to use it in queries.

```rust

// Just add this attribute...
#[bevy_trait_query::queryable]
pub trait Tooltip {
    fn tooltip(&self) -> &str;
}

// ...and now you can use your trait in queries.
fn show_tooltips_system(
    tooltips: Query<&dyn Tooltip>,
    // ...
) {
    // ...
}

bevy::ecs::system::assert_is_system(show_tooltips_system);
```

Since Rust unfortunately lacks any kind of reflection, it is necessary to register each
component with the trait when the app gets built.

```rust
#[derive(Component)]
struct Player(String);

#[derive(Component)]
enum Villager {
    Farmer,
    Blacksmith,
    // ...
}

#[derive(Component)]
struct Monster;

// Trait implementations omitted for brevity...

// Adds the logic for this game.
struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        // We must import this trait in order to register our components.
        // If we don't register them, they will be invisible to the game engine.
        use bevy_trait_query::RegisterExt;

        app
            .register_component_as::<dyn Tooltip, Player>()
            .register_component_as::<dyn Tooltip, Villager>()
            .register_component_as::<dyn Tooltip, Monster>()
            // Add systems...
    }
}
```

Unlike queries for concrete types, it's possible for an entity to have multiple components
that match a trait query.

```rust

fn show_tooltips(
    tooltips: Query<&dyn Tooltip>,
    // ...
) {
    // Iterate over each entity that has tooltips.
    for entity_tooltips in &tooltips {
        // Iterate over each component implementing `Tooltip` for the current entity.
        for tooltip in entity_tooltips {
            println!("Tooltip: {}", tooltip.tooltip());
        }
    }

    // If you instead just want to iterate over all tooltips, you can just do:
    for tooltip in tooltips.iter().flatten() {
        println!("Tooltip: {}", tooltip.tooltip());
    }
}
```

Alternatively, if you expect to only have component implementing the trait for each entity,
you can use the filter [`One`](https://docs.rs/bevy-trait-query/latest/bevy_trait_query/struct.One.html). This has significantly better performance than iterating
over all trait impls.

```rust
use bevy_trait_query::One;

fn show_tooltips(
    tooltips: Query<One<&dyn Tooltip>>,
    // ...
) {
    for tooltip in &tooltips {
        println!("Tooltip: {}", tooltip.tooltip());
    }
}
```

### Performance

The performance of trait queries is quite competitive. Here are some benchmarks for simple cases:

|                   | Concrete type | One<dyn Trait> | All<dyn Trait> |
|-------------------|----------------|-------------------|-----------------|
| 1 match           | 16.135 µs      | 31.441 µs         | 63.273 µs       |
| 2 matches         | 17.501 µs      | -                 | 102.83 µs       |
| 1-2 matches       | -              | 16.959 µs         | 82.179 µs       |

<!-- cargo-rdme end -->

# License

[MIT](LICENSE-MIT) or [APACHE-2.0](LICENSE-APACHE)
