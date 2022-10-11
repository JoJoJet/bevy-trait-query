use bevy::prelude::*;
use bevy_trait_query::*;

/// Define a trait for our components to implement.
pub trait Person: 'static {
    fn name(&self) -> &str;
    fn age(&self) -> u32;
    fn set_age(&mut self, age: u32);
}

// Add `WorldQuery` impls for `dyn Person`
impl_dyn_query!(Person);

#[derive(Component)]
pub struct Beans(String, u32);

impl Person for Beans {
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
pub struct Reggie(u32);

impl Person for Reggie {
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

fn main() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        // Register the components with the trait.
        .register_component_as::<dyn Person, Beans>()
        .register_component_as::<dyn Person, Reggie>()
        // Add systems.
        .add_startup_system(setup)
        .add_system(print_info)
        .add_system(age_up.after(print_info))
        .add_system(change_name.after(age_up));
    app.update();
    app.update();
}

fn setup(mut commands: Commands) {
    commands.spawn().insert(Beans("Garbanzo".to_owned(), 7));
    commands.spawn().insert(Beans("Garbanzo".to_owned(), 14));
    commands.spawn().insert(Reggie(27));
}

// Prints the name and age of every `Person`.
fn print_info(people: Query<&dyn Person>) {
    println!("All people:");
    for person in &people {
        println!("{}: {}", person.name(), person.age());
    }
    println!();
}

fn age_up(mut people: Query<&mut dyn Person>) {
    for person in &mut people {
        let new_age = person.age() + 1;
        person.set_age(new_age);
    }
}

fn change_name(mut q: Query<&mut Beans>) {
    for mut bean in &mut q {
        if bean.name() == "Garbanzo" && bean.age() > 14 {
            bean.0 = "Garbanza".to_owned();
        }
    }
}

// Output:
//
// All people:
// Garbanzo: 7
// Garbanzo: 14
// Reginald: 27
//
// All people:
// Garbanzo: 8
// Garbanza: 15
// Reginald: 28
