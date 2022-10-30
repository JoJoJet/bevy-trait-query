#![allow(clippy::all)]

use bevy::prelude::*;
use bevy_trait_query::*;
use criterion::*;
use std::fmt::Display;

/// Define a trait for our components to implement.
#[queryable]
pub trait Messages {
    fn messages(&self) -> &[String];
    fn send_message(&mut self, _: &dyn Display);
}

// Add `WorldQuery` impls for `dyn Message`
impl_trait_query!(Messages);

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

pub struct Benchmark<'w>(World, QueryState<&'w RecA>, Vec<usize>);

impl<'w> Benchmark<'w> {
    // Each entity only has one component in practice.
    fn one() -> Self {
        let mut world = World::new();

        for _ in 0..10_000 {
            world.spawn((Name::new("Hello"), RecA { messages: vec![] }));
        }

        let query = world.query();
        Self(world, query, default())
    }
    fn multiple() -> Self {
        let mut world = World::new();

        for _ in 0..10_000 {
            world.spawn((
                Name::new("Hello"),
                RecB { messages: vec![] },
                RecA { messages: vec![] },
            ));
        }

        let query = world.query();
        Self(world, query, default())
    }

    pub fn run(&mut self) {
        let mut output = Vec::new();
        for x in self.1.iter(&mut self.0) {
            output.push(x.messages().len());
        }
        self.2 = output;
    }
}

pub fn one(c: &mut Criterion) {
    let mut benchmark = Benchmark::one();
    c.bench_function("concrete - 1 match", |b| b.iter(|| benchmark.run()));
    eprintln!("{}", benchmark.2.len());
}
pub fn multiple(c: &mut Criterion) {
    let mut benchmark = Benchmark::multiple();
    c.bench_function("concrete - 2 matches", |b| b.iter(|| benchmark.run()));
    eprintln!("{}", benchmark.2.len());
}

criterion_group!(concrete, one, multiple);
criterion_main!(concrete);
