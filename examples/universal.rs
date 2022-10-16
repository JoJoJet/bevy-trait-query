use std::fmt::Display;

use bevy::prelude::*;
use bevy_trait_query::*;

/// Define a trait for our components to implement.
pub trait Messages: 'static {
    fn messages(&self) -> &[String];
    fn send_message(&mut self, _: &dyn Display);
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

fn main() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        // Register the components with the trait.
        .register_component_as::<dyn Messages, RecA>()
        .register_component_as::<dyn Messages, RecB>()
        // Add systems.
        .add_startup_system(setup)
        .add_system(print_messages)
        .add_system(send_messages.after(print_messages));
    app.update();
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
    println!("New frame:");
    for receiver in &receivers {
        println!("Entity:");
        for m in receiver {
            println!("{:?}", m.messages());
        }
        println!();
    }
    println!();
}

fn send_messages(mut receivers: Query<All<&mut dyn Messages>>) {
    for (i, receiver) in receivers.iter_mut().enumerate() {
        for mut m in receiver {
            m.send_message(&i);
        }
    }
}

// Output:
//
