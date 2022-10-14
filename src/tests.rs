use super::*;

#[derive(Default)]
pub struct Output(Vec<String>);

pub trait Person: 'static {
    fn name(&self) -> &str;
    fn age(&self) -> u32;
    fn set_age(&mut self, age: u32);
}

impl_dyn_query!(Person);

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
fn existential1() {
    let mut world = World::new();
    world.init_resource::<Output>();
    world
        .register_component_as::<dyn Person, Human>()
        .register_component_as::<dyn Person, Dolphin>();

    world.spawn().insert(Human("Garbanzo".to_owned(), 7));
    world
        .spawn()
        .insert_bundle((Human("Garbanzo".to_owned(), 14), Fem));
    world.spawn().insert(Dolphin(27));

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

// Prints the name and age of every `Person`.
fn print_info(people: Query<&dyn Person>, mut output: ResMut<Output>) {
    output.0.push("All people:".to_string());
    for person in &people {
        output
            .0
            .push(format!("{}: {}", person.name(), person.age()));
    }
    output.0.push(default());
}

fn age_up(mut people: Query<&mut dyn Person>) {
    for person in &mut people {
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
