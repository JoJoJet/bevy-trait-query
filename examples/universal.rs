use bevy::prelude::*;
use bevy_trait_query::*;

/// Define a trait for our components to implement.
pub trait Messages: 'static {
    fn messages(&self) -> &[String];
}

// Add `WorldQuery` impls for `dyn Person`
impl_dyn_query!(Messages);

#[derive(Component)]
pub struct RecA {
    messages: Vec<String>,
}

impl Messages for RecA {
    fn messages(&self) -> &[String] {
        &self.messages
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
}

fn main() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        // Register the components with the trait.
        .register_component_as::<dyn Messages, RecA>()
        .register_component_as::<dyn Messages, RecB>()
        // Add systems.
        .add_startup_system(setup)
        .add_system(print_messages);
    app.update();
}

fn setup(mut commands: Commands) {
    commands.spawn().insert(RecA {
        messages: vec!["1".to_owned()],
    });
    commands.spawn().insert(RecB {
        messages: vec!["2".to_owned(), "3".to_owned()],
    });
    commands
        .spawn()
        .insert(RecA {
            messages: vec!["4".to_owned()],
        })
        .insert(RecB {
            messages: vec!["5".to_owned(), "6".to_owned(), "7".to_owned()],
        });
}

// Prints the messages in every receiver.
fn print_messages(receivers: Query<All<&dyn Messages>>) {
    for receiver in &receivers {
        println!("Entity:");
        for m in receiver {
            println!("{:?}", m.messages());
        }
        println!();
    }
}

// Output:
//
