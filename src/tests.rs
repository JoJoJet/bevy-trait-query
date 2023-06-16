use super::*;
use crate::query_filter::{OneAddedFilter, OneChangedFilter};
use std::fmt::{Debug, Display};

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

    let mut schedule = Schedule::new();
    schedule.add_systems((print_info, (age_up, change_name, pluralize)).chain());

    schedule.run(&mut world);
    schedule.run(&mut world);

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

    let mut schedule = Schedule::new();
    schedule.add_systems((print_all_info, (age_up_fem, age_up_not)).chain());

    schedule.run(&mut world);
    schedule.run(&mut world);

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

#[test]
fn added_all() {
    let mut world = World::new();
    world.init_resource::<Output>();
    world
        .register_component_as::<dyn Person, Human>()
        .register_component_as::<dyn Person, Dolphin>();

    world.spawn(Human("Henry".to_owned(), 22));

    let mut schedule = Schedule::new();
    schedule.add_systems((print_added_all_info, age_up_fem).chain());

    schedule.run(&mut world);

    world.spawn((Human("Garbanzo".to_owned(), 17), Fem, Dolphin(17)));

    schedule.run(&mut world);

    // only changes will occur now to the ages of Garbanzo/Reginald, so nothing should be printed

    schedule.run(&mut world);

    println!("{:?}", world.resource::<Output>().0);

    assert_eq!(
        world.resource::<Output>().0,
        &[
            "Added people:",
            "Henry: 22",
            "",
            "Added people:",
            "Garbanzo: 17",
            "Reginald: 17",
            "",
            "Added people:",
            ""
        ]
    );
}

// Prints the name and age of every newly added `Person`.
fn print_added_all_info(people: Query<AddedAll<&dyn Person>>, mut output: ResMut<Output>) {
    output.0.push("Added people:".to_string());
    for person in (&people).into_iter().flatten() {
        output
            .0
            .push(format!("{}: {}", person.name(), person.age()));
    }
    output.0.push(default());
}

#[test]
fn changed_all() {
    let mut world = World::new();
    world.init_resource::<Output>();
    world
        .register_component_as::<dyn Person, Human>()
        .register_component_as::<dyn Person, Dolphin>();

    let mut schedule = Schedule::new();
    schedule.add_systems((print_changed_all_info, age_up_fem).chain());

    // Henry is newly added, so we expect him to be printed
    world.spawn(Human("Henry".to_owned(), 22));

    schedule.run(&mut world);

    // Garbanzo and Dolphin (Reginald) are newly added, so we expect them to be printed
    world.spawn((Human("Garbanzo".to_owned(), 17), Fem, Dolphin(17)));

    schedule.run(&mut world);

    // Garbanzo and Dolphin (Reginald) will both be incremented in age by one by `age_up_fem`, so
    // they should be printed again

    schedule.run(&mut world);

    assert_eq!(
        world.resource::<Output>().0,
        &[
            "Changed people:",
            "Henry: 22",
            "",
            "Changed people:",
            "Garbanzo: 17",
            "Reginald: 17",
            "",
            "Changed people:",
            "Garbanzo: 18",
            "Reginald: 18",
            "",
        ]
    );
}

// Prints the name and age of every `Person` whose info has changed in some way
fn print_changed_all_info(people: Query<ChangedAll<&dyn Person>>, mut output: ResMut<Output>) {
    output.0.push("Changed people:".to_string());
    for person in (&people).into_iter().flatten() {
        output
            .0
            .push(format!("{}: {}", person.name(), person.age()));
    }
    output.0.push(default());
}

#[test]
fn added_one() {
    let mut world = World::new();
    world.init_resource::<Output>();
    world
        .register_component_as::<dyn Person, Human>()
        .register_component_as::<dyn Person, Dolphin>();

    world.spawn(Human("Henry".to_owned(), 22));

    let mut schedule = Schedule::new();
    schedule.add_systems((print_added_one_info, (age_up_fem, age_up_not)).chain());

    schedule.run(&mut world);

    world.spawn((Dolphin(27), Fem));

    schedule.run(&mut world);

    schedule.run(&mut world);

    assert_eq!(
        world.resource::<Output>().0,
        &[
            "Added people:",
            "Henry: 22",
            "",
            "Added people:",
            "Reginald: 27",
            "",
            "Added people:",
            "",
        ]
    );
}

// Prints the name and age of every newly added `Person`.
fn print_added_one_info(people: Query<AddedOne<&dyn Person>>, mut output: ResMut<Output>) {
    output.0.push("Added people:".to_string());
    for person in (&people).into_iter().flatten() {
        output
            .0
            .push(format!("{}: {}", person.name(), person.age()));
    }
    output.0.push(default());
}

#[test]
fn changed_one() {
    let mut world = World::new();
    world.init_resource::<Output>();
    world
        .register_component_as::<dyn Person, Human>()
        .register_component_as::<dyn Person, Dolphin>();

    let mut schedule = Schedule::new();
    schedule.add_systems((print_changed_one_info, age_up_fem).chain());

    world.spawn(Human("Henry".to_owned(), 22));

    schedule.run(&mut world);

    world.spawn((Dolphin(27), Fem));

    schedule.run(&mut world);

    schedule.run(&mut world);

    assert_eq!(
        world.resource::<Output>().0,
        &[
            "Changed people:",
            "Henry: 22",
            "",
            "Changed people:",
            "Reginald: 27",
            "",
            "Changed people:",
            "Reginald: 28",
            ""
        ]
    );
}

// Prints the name and age of every `Person` whose info has changed in some way
fn print_changed_one_info(people: Query<ChangedOne<&dyn Person>>, mut output: ResMut<Output>) {
    output.0.push("Changed people:".to_string());
    for person in (&people).into_iter().flatten() {
        output
            .0
            .push(format!("{}: {}", person.name(), person.age()));
    }
    output.0.push(default());
}

#[test]
fn one_added_filter() {
    let mut world = World::new();
    world.init_resource::<Output>();
    world
        .register_component_as::<dyn Person, Human>()
        .register_component_as::<dyn Person, Dolphin>();

    world.spawn(Human("Henry".to_owned(), 22));

    let mut schedule = Schedule::new();
    schedule.add_systems((print_one_added_filter_info, (age_up_fem, age_up_not)).chain());

    schedule.run(&mut world);

    world.spawn((Dolphin(27), Fem));

    schedule.run(&mut world);

    schedule.run(&mut world);

    assert_eq!(
        world.resource::<Output>().0,
        &[
            "Added people:",
            "Henry: 22",
            "",
            "Added people:",
            "Reginald: 27",
            "",
            "Added people:",
            "",
        ]
    );
}

// Prints the name and age of every newly added `Person`.
fn print_one_added_filter_info(
    people: Query<One<&dyn Person>, OneAddedFilter<dyn Person>>,
    mut output: ResMut<Output>,
) {
    output.0.push("Added people:".to_string());
    for person in (&people).into_iter() {
        output
            .0
            .push(format!("{}: {}", person.name(), person.age()));
    }
    output.0.push(default());
}

#[test]
fn one_changed_filter() {
    let mut world = World::new();
    world.init_resource::<Output>();
    world
        .register_component_as::<dyn Person, Human>()
        .register_component_as::<dyn Person, Dolphin>();

    world.spawn(Human("Henry".to_owned(), 22));

    let mut schedule = Schedule::new();
    schedule.add_systems((print_one_changed_filter_info, age_up_fem).chain());

    schedule.run(&mut world);

    world.spawn((Dolphin(27), Fem));

    schedule.run(&mut world);

    schedule.run(&mut world);

    assert_eq!(
        world.resource::<Output>().0,
        &[
            "Changed people:",
            "Henry: 22",
            "",
            "Changed people:",
            "Reginald: 27",
            "",
            "Changed people:",
            "Reginald: 28",
            ""
        ]
    );
}

// Prints the name and age of every `Person` whose info has changed in some way
fn print_one_changed_filter_info(
    people: Query<One<&dyn Person>, OneChangedFilter<dyn Person>>,
    mut output: ResMut<Output>,
) {
    output.0.push("Changed people:".to_string());
    for person in (&people).into_iter() {
        output
            .0
            .push(format!("{}: {}", person.name(), person.age()));
    }
    output.0.push(default());
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

    let mut schedule = Schedule::new();
    schedule.add_systems((print_messages, spawn_sparse).chain());

    schedule.run(&mut world);
    schedule.run(&mut world);

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

    let mut schedule = Schedule::new();
    schedule.add_systems(count_impls);

    fn count_impls(q: Query<&dyn Messages>, mut output: ResMut<Output>) {
        for traits in &q {
            // Make sure each impl gets yielded the correct number of times.
            // We don't want any of them to get double-counted.
            output.0.push(format!("{} Traits", traits.iter().count()));
        }
    }

    schedule.run(&mut world);

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

#[allow(dead_code)]
fn generic_system<T: Debug + 'static>(_q: Query<&dyn GenericTrait<T>>) {
    // Assert that this current function is a system.
    let _x = IntoSystem::into_system(generic_system::<T>);
}

#[queryable]
pub trait AssociatedTrait {
    type T: Display;
}

#[allow(dead_code)]
fn associated_type_system<T: Display + 'static>(_q: Query<&dyn AssociatedTrait<T = T>>) {
    // Assert that this current function is a system.
    let _x = IntoSystem::into_system(associated_type_system::<T>);
}
