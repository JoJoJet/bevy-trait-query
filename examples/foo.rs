use bevy::prelude::*;
use bevy_trait_query::*;

fn main() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .register_component_as::<dyn Foo, Bar>()
        .register_component_as::<dyn Foo, Gub>()
        .add_startup_system(setup)
        .add_system(print_info)
        .add_system(age_up.after(print_info))
        .add_system(change_name.after(age_up));
    app.update();
    app.update();

    fn setup(mut commands: Commands) {
        commands.spawn().insert(Bar("Garbanzo".to_owned(), 7));
        commands.spawn().insert(Bar("Garbanzo".to_owned(), 14));
        commands.spawn().insert(Gub(27));
    }

    fn print_info(q: Query<&dyn Foo>) {
        println!("All people:");
        for foo in &q {
            println!("{}: {}", foo.name(), foo.age());
        }
        println!();
    }

    fn age_up(mut q: Query<&mut dyn Foo>) {
        for foo in &mut q {
            let new_age = foo.age() + 1;
            foo.set_age(new_age);
        }
    }

    fn change_name(mut q: Query<&mut Bar>) {
        for mut bar in &mut q {
            if bar.1 > 14 {
                bar.0 = "Garbanza".to_owned();
            }
        }
    }
}

#[derive(Component)]
pub struct Bar(String, u32);

impl Foo for Bar {
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
pub struct Gub(u32);

impl Foo for Gub {
    fn name(&self) -> &str {
        "reginald"
    }
    fn age(&self) -> u32 {
        self.0
    }
    fn set_age(&mut self, age: u32) {
        self.0 = age;
    }
}
