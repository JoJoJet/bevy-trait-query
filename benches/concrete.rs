#![allow(clippy::all)]

use bevy::prelude::*;
use bevy_trait_query::*;
use criterion::*;
use std::fmt::Display;

/// Define a trait for our components to implement.
pub trait Messages: 'static {
    fn messages(&self) -> &[String];
    fn send_message(&mut self, _: &dyn Display);
}

// Add `WorldQuery` impls for `dyn Message`
impl_dyn_query!(Messages);

#[derive(Component)]
pub struct RecA {
    messages: Vec<String>,
}

impl Messages for RecA {
    fn messages(&self) -> &[String] {
        &self.messages
    }
    fn send_message(&mut self, msg: &dyn Display) {
        self.messages.push(msg.to_string());
    }
}

#[derive(Component)]
pub struct RecB {
    messages: Vec<String>,
}

impl Messages for RecB {
    fn messages(&self) -> &[String] {
        &self.messages
    }
    fn send_message(&mut self, msg: &dyn Display) {
        self.messages.push(msg.to_string());
    }
}

pub struct Benchmark<'w>(World, QueryState<&'w RecA>);

impl<'w> Benchmark<'w> {
    // Each entity only has one component in practice.
    fn one() -> Self {
        let mut world = World::new();

        for _ in 0..10_000 {
            world
                .spawn()
                .insert_bundle((Name::new("Hello"), RecA { messages: vec![] }));
        }

        let query = world.query::<&RecA>();
        Self(world, query)
    }
    fn multiple() -> Self {
        let mut world = World::new();

        for _ in 0..10_000 {
            world.spawn().insert_bundle((
                Name::new("Hello"),
                RecB { messages: vec![] },
                RecA { messages: vec![] },
            ));
        }

        let query = world.query::<&RecA>();
        Self(world, query)
    }
    // Queries with only one, and queries with mutliple.
    pub fn distributed() -> Self {
        let mut world = World::new();

        for _ in 0..2_500 {
            world
                .spawn()
                .insert_bundle((Name::new("Hello"), RecA { messages: vec![] }));
        }
        for _ in 0..2_500 {
            world
                .spawn()
                .insert_bundle((Name::new("Hello"), RecB { messages: vec![] }));
        }
        for _ in 0..5_000 {
            world.spawn().insert_bundle((
                Name::new("Hello"),
                RecA { messages: vec![] },
                RecB { messages: vec![] },
            ));
        }

        let query = world.query::<&RecA>();
        Self(world, query)
    }

    pub fn run(&mut self) {
        for x in self.1.iter_mut(&mut self.0) {
            criterion::black_box(x);
        }
    }
}

pub fn one(c: &mut Criterion) {
    let mut benchmark = Benchmark::one();
    c.bench_function("concrete-one", |b| b.iter(|| benchmark.run()));
}
pub fn multiple(c: &mut Criterion) {
    let mut benchmark = Benchmark::multiple();
    c.bench_function("concrete-multiple", |b| b.iter(|| benchmark.run()));
}

criterion_group!(concrete, one, multiple);
criterion_main!(concrete);
