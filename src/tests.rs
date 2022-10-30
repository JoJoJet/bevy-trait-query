use super::*;
use std::fmt::{Debug, Display};

pub use crate as bevy_trait_query;

#[derive(Resource, Default)]
pub struct Output(Vec<String>);

#[queryable]
pub trait Person {
    fn name(&self) -> &str;
    fn age(&self) -> u32;
    fn set_age(&mut self, age: u32);
}

#[derive(Component)]
struct Fem;

#[derive(Component)]
pub struct Human(String, u32);

impl Person for Human {
    fn name(&self) -> &str {
        &self.0
    }
    fn age(&self) -> u32 {
        self.1
    }
    fn set_age(&mut self, age: u32) {
        self.1 = age;
    }
}

#[derive(Component)]
pub struct Dolphin(u32);

impl Person for Dolphin {
    fn name(&self) -> &str {
        "Reginald"
    }
    fn age(&self) -> u32 {
        self.0
    }
    fn set_age(&mut self, age: u32) {
        self.0 = age;
    }
}

#[test]
fn one1() {
    let mut world = World::new();
    world.init_resource::<Output>();
    world
        .register_component_as::<dyn Person, Human>()
        .register_component_as::<dyn Person, Dolphin>();

    world.spawn(Human("Garbanzo".to_owned(), 7));
    world.spawn((Human("Garbanzo".to_owned(), 14), Fem));
    world.spawn(Dolphin(27));

    let mut stage = SystemStage::parallel();
    stage
        .add_system(print_info)
        .add_system(age_up.after(print_info))
        .add_system(change_name.after(print_info))
        .add_system(pluralize.after(print_info));

    stage.run(&mut world);
    stage.run(&mut world);

    assert_eq!(
        world.resource::<Output>().0,
        &[
            "All people:",
            "Garbanzo: 7",
            "Garbanzo: 14",
            "Reginald: 27",
            "",
            "All people:",
            "Garbanzos: 8",
            "Garbanza: 15",
            "Reginald: 28",
            "",
        ]
    );
}

fn print_info(people: Query<One<&dyn Person>>, mut output: ResMut<Output>) {
    output.0.push("All people:".to_string());
    for person in &people {
        output
            .0
            .push(format!("{}: {}", person.name(), person.age()));
    }
    output.0.push(default());
}

fn age_up(mut people: Query<One<&mut dyn Person>>) {
    for mut person in &mut people {
        let new_age = person.age() + 1;
        person.set_age(new_age);
    }
}

fn change_name(mut q: Query<&mut Human, With<Fem>>) {
    for mut bean in &mut q {
        bean.0 = "Garbanza".to_owned();
    }
}

fn pluralize(mut q: Query<&mut Human, Without<Fem>>) {
    for mut bean in &mut q {
        bean.0.push('s');
    }
}

#[test]
fn all1() {
    let mut world = World::new();
    world.init_resource::<Output>();
    world
        .register_component_as::<dyn Person, Human>()
        .register_component_as::<dyn Person, Dolphin>();

    world.spawn(Human("Henry".to_owned(), 22));
    world.spawn((Human("Eliza".to_owned(), 31), Fem, Dolphin(6)));
    world.spawn((Human("Garbanzo".to_owned(), 17), Fem, Dolphin(17)));
    world.spawn(Dolphin(27));

    let mut stage = SystemStage::parallel();
    stage
        .add_system(print_all_info)
        .add_system(age_up_fem.after(print_all_info))
        .add_system(age_up_not.after(print_all_info));

    stage.run(&mut world);
    stage.run(&mut world);

    assert_eq!(
        world.resource::<Output>().0,
        &[
            "All people:",
            "Henry: 22",
            "Eliza: 31",
            "Reginald: 6",
            "Garbanzo: 17",
            "Reginald: 17",
            "Reginald: 27",
            "",
            "All people:",
            "Henry: 23",
            "Eliza: 32",
            "Reginald: 7",
            "Garbanzo: 18",
            "Reginald: 18",
            "Reginald: 28",
            "",
        ]
    );
}

// Prints the name and age of every `Person`.
fn print_all_info(people: Query<&dyn Person>, mut output: ResMut<Output>) {
    output.0.push("All people:".to_string());
    for all in &people {
        for person in all {
            output
                .0
                .push(format!("{}: {}", person.name(), person.age()));
        }
    }
    output.0.push(default());
}

fn age_up_fem(mut q: Query<&mut dyn Person, With<Fem>>) {
    for all in &mut q {
        for mut p in all {
            let age = p.age();
            p.set_age(age + 1);
        }
    }
}

fn age_up_not(mut q: Query<&mut dyn Person, Without<Fem>>) {
    for all in &mut q {
        for mut p in all {
            let age = p.age();
            p.set_age(age + 1);
        }
    }
}

#[queryable]
pub trait Messages {
    fn send(&mut self, _: &dyn Display);
    fn read(&self) -> &[String];
}

#[derive(Component)]
pub struct RecA(Vec<String>);

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct RecB(Vec<String>);

impl Messages for RecA {
    fn send(&mut self, m: &dyn Display) {
        self.0.push(format!("RecA: {m}"));
    }
    fn read(&self) -> &[String] {
        &self.0
    }
}

impl Messages for RecB {
    fn send(&mut self, m: &dyn Display) {
        self.0.push(format!("RecB: {m}"));
    }
    fn read(&self) -> &[String] {
        &self.0
    }
}

#[test]
fn sparse1() {
    let mut world = World::new();
    world.init_resource::<Output>();
    world
        .register_component_as::<dyn Messages, RecA>()
        .register_component_as::<dyn Messages, RecB>();

    world.spawn(RecA(vec![]));
    world.spawn((RecA(vec![]), RecB(vec!["Mama mia".to_owned()])));

    let mut stage = SystemStage::parallel();
    stage
        .add_system(print_messages)
        .add_system(spawn_sparse.after(print_messages));

    stage.run(&mut world);
    stage.run(&mut world);

    assert_eq!(
        world.resource::<Output>().0,
        &[
            "New frame:",
            "0: []",
            "1: []",
            r#"1: ["Mama mia"]"#,
            "New frame:",
            "0: []",
            "1: []",
            r#"1: ["Mama mia"]"#,
            r#"2: ["Sparse #0"]"#,
            r#"3: ["Sparse #1"]"#,
            r#"4: ["Sparse #2"]"#,
        ]
    );
}

fn print_messages(q: Query<&dyn Messages>, mut output: ResMut<Output>) {
    output.0.push("New frame:".to_owned());
    for (i, all) in q.iter().enumerate() {
        for msgs in all {
            output.0.push(format!("{i}: {:?}", msgs.read()));
        }
    }
}

fn spawn_sparse(mut commands: Commands) {
    for i in 0..3 {
        commands.spawn(RecB(vec![format!("Sparse #{i}")]));
    }
}

// Make sure it works correctly when components are registered multiple times.
#[test]
fn multi_register() {
    let mut world = World::new();
    world.init_resource::<Output>();
    // Register each trait impl multiple times. Nothing should happen for the extra registrations.
    world
        .register_component_as::<dyn Messages, RecA>()
        .register_component_as::<dyn Messages, RecA>()
        .register_component_as::<dyn Messages, RecA>()
        .register_component_as::<dyn Messages, RecB>()
        .register_component_as::<dyn Messages, RecB>();

    world.spawn(RecA(vec![]));
    world.spawn((RecA(vec![]), RecB(vec![])));
    world.spawn(RecB(vec![]));

    let mut stage = SystemStage::parallel();
    stage.add_system(count_impls);

    fn count_impls(q: Query<&dyn Messages>, mut output: ResMut<Output>) {
        for traits in &q {
            // Make sure each impl gets yielded the correct number of times.
            // We don't want any of them to get double-counted.
            output
                .0
                .push(format!("{} Traits", traits.into_iter().count()));
        }
    }

    stage.run(&mut world);

    assert_eq!(
        world.resource::<Output>().0,
        &["1 Traits", "2 Traits", "1 Traits"]
    );
}

#[queryable]
pub trait GenericTrait<T: Debug> {
    fn get(&self) -> T;
    fn get_double(&self) -> T
    where
        T: std::ops::Add<Output = T> + Clone,
    {
        let val = self.get();
        val.clone() + val
    }
}
