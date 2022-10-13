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

macro_rules! create_entities {
    ($world:ident; $( $variants:ident ),*) => {
        $(
            #[derive(Component)]
            struct $variants(f32);
            for _ in 0..20 {
                $world.spawn().insert_bundle(($variants(0.0), RecA { messages: vec![] }, RecB { messages: vec![] }));
            }
        )*
    };
}

pub struct Benchmark(World);

impl Benchmark {
    fn new() -> Self {
        let mut world = World::new();

        world.register_component_as::<dyn Messages, RecA>();
        world.register_component_as::<dyn Messages, RecB>();

        create_entities!(
            world; A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z
        );

        Self(world)
    }
}

pub fn concrete(c: &mut Criterion) {
    let mut benchmark = Benchmark::new();
    let mut query = benchmark.0.query::<&mut RecA>();
    let mut output = Vec::new();
    c.bench_function("concrete-fragmented", |b| {
        b.iter(|| {
            for x in query.iter_mut(&mut benchmark.0) {
                output.push(x.messages().len());
            }
        });
    });
    eprintln!("{}", output.len());
}
pub fn existential(c: &mut Criterion) {
    let mut benchmark = Benchmark::new();
    let mut query = benchmark.0.query::<&mut dyn Messages>();
    let mut output = Vec::new();
    c.bench_function("exisential-fragmented", |b| {
        b.iter(|| {
            for x in query.iter_mut(&mut benchmark.0) {
                output.push(x.messages().len());
            }
        });
    });
    eprintln!("{}", output.len());
}
pub fn universal(c: &mut Criterion) {
    let mut benchmark = Benchmark::new();
    let mut query = benchmark.0.query::<All<&mut dyn Messages>>();
    let mut output = Vec::new();
    c.bench_function("universal-fragmented", |b| {
        b.iter(|| {
            for all in query.iter_mut(&mut benchmark.0) {
                for x in all {
                    output.push(x.messages().len());
                }
            }
        });
    });
    eprintln!("{}", output.len());
}

criterion_group!(fragmented, concrete, existential, universal);
criterion_main!(fragmented);
