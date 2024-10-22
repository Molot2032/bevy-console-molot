use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::thread;

use bevy::prelude::*;
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result};

use crate::ConsoleCommandEntered;
#[derive(Resource)]
pub struct ConsoleLineReceiver {
    rx: Mutex<Receiver<Result<String>>>,
}

/// The user inputted a console interrupt
#[derive(Event)]
pub struct ConsoleInterrupted;

fn str_to_command(str: &str) -> Option<ConsoleCommandEntered> {
    let mut iter = str.split_whitespace();
    let command_name = iter.next()?.to_owned();
    let args = iter.map(|s| s.to_owned()).collect();

    Some(ConsoleCommandEntered { command_name, args })
}

fn read_rustyline(
    clr: Res<ConsoleLineReceiver>,
    mut evw_consolecommand: EventWriter<ConsoleCommandEntered>,
    mut evw_interrupt: EventWriter<ConsoleInterrupted>,
) {
    if let Ok(r) = clr.rx.lock() {
        if let Ok(res) = r.try_recv() {
            match res {
                Ok(str) => {
                    evw_consolecommand.send_batch(str_to_command(&str));
                }
                Err(ReadlineError::Interrupted) => {
                    evw_interrupt.send(ConsoleInterrupted);
                }
                _ => (),
            }
        }
    }
}

pub(super) fn setup_rustyline(app: &mut App) {
    let (tx, rx): (Sender<Result<String>>, Receiver<Result<String>>) = mpsc::channel();

    thread::spawn(move || {
        let mut rl = match DefaultEditor::new() {
            Err(e) => {
                error!(
                    "Error: {e:?}. Failed to create rustyline editor. Reading input from attached console will not be available."
                );
                return;
            }
            Ok(rl) => rl,
        };

        loop {
            let input = rl.readline("");
            let _ = tx.send(input);
        }
    });

    app.insert_resource(ConsoleLineReceiver { rx: Mutex::new(rx) })
        .add_event::<ConsoleInterrupted>()
        .add_systems(Update, read_rustyline);
}
