use bevy::ecs::{
    component::Tick,
    system::{Resource, SystemMeta, SystemParam},
    world::unsafe_world_cell::UnsafeWorldCell,
};
use bevy::{input::keyboard::KeyboardInput, prelude::*};

#[cfg(feature = "ui")]
use bevy_egui::egui::{self, Align, ScrollArea, TextEdit};
#[cfg(feature = "ui")]
use bevy_egui::egui::{text::LayoutJob, text_selection::CCursorRange};
#[cfg(feature = "ui")]
use bevy_egui::egui::{Context, Id};
#[cfg(feature = "ui")]
use bevy_egui::{
    egui::{epaint::text::cursor::CCursor, Color32, FontId, TextFormat},
    EguiContexts,
};

use clap::{CommandFactory, FromArgMatches};
use shlex::Shlex;
use std::marker::PhantomData;
use std::mem;
use std::{
    collections::{BTreeMap, VecDeque},
    iter::once,
};

use crate::{
    color::{parse_ansi_styled_str, Colour, TextFormattingOverride},
    ConsoleSet,
};

type ConsoleCommandEnteredReaderSystemParam = EventReader<'static, 'static, ConsoleCommandEntered>;

type PrintConsoleLineWriterSystemParam = EventWriter<'static, PrintConsoleLine>;

/// A super-trait for command like structures
pub trait Command: NamedCommand + CommandFactory + FromArgMatches + Sized + Resource {}
impl<T: NamedCommand + CommandFactory + FromArgMatches + Sized + Resource> Command for T {}

/// Trait used to allow uniquely identifying commands at compile time
pub trait NamedCommand {
    /// Return the unique command identifier (same as the command "executable")
    fn name() -> &'static str;
}

/// Executed parsed console command.
///
/// Used to capture console commands which implement [`CommandName`], [`CommandArgs`] & [`CommandHelp`].
/// These can be easily implemented with the [`ConsoleCommand`](bevy_console_derive::ConsoleCommand) derive macro.
///
/// # Example
///
/// ```
/// # use bevy_console::ConsoleCommand;
/// # use clap::Parser;
/// /// Prints given arguments to the console.
/// #[derive(Parser, ConsoleCommand)]
/// #[command(name = "log")]
/// struct LogCommand {
///     /// Message to print
///     msg: String,
///     /// Number of times to print message
///     num: Option<i64>,
/// }
///
/// fn log_command(mut log: ConsoleCommand<LogCommand>) {
///     if let Some(Ok(LogCommand { msg, num })) = log.take() {
///         log.ok();
///     }
/// }
/// ```
pub struct ConsoleCommand<'w, T> {
    command: Option<Result<T, clap::Error>>,
    console_line: EventWriter<'w, PrintConsoleLine>,
}

impl<'w, T> ConsoleCommand<'w, T> {
    /// Returns Some(T) if the command was executed and arguments were valid.
    ///
    /// This method should only be called once.
    /// Consecutive calls will return None regardless if the command occurred.
    pub fn take(&mut self) -> Option<Result<T, clap::Error>> {
        mem::take(&mut self.command)
    }

    /// Print `[ok]` in the console.
    pub fn ok(&mut self) {
        self.console_line.send(PrintConsoleLine::new("[ok]".into()));
    }

    /// Print `[failed]` in the console.
    pub fn failed(&mut self) {
        self.console_line
            .send(PrintConsoleLine::new("[failed]".into()));
    }

    /// Print a reply in the console.
    ///
    /// See [`reply!`](crate::reply) for usage with the [`format!`] syntax.
    pub fn reply(&mut self, msg: impl Into<String>) {
        self.console_line.send(PrintConsoleLine::new(msg.into()));
    }

    /// Print a reply in the console followed by `[ok]`.
    ///
    /// See [`reply_ok!`](crate::reply_ok) for usage with the [`format!`] syntax.
    pub fn reply_ok(&mut self, msg: impl Into<String>) {
        self.console_line.send(PrintConsoleLine::new(msg.into()));
        self.ok();
    }

    /// Print a reply in the console followed by `[failed]`.
    ///
    /// See [`reply_failed!`](crate::reply_failed) for usage with the [`format!`] syntax.
    pub fn reply_failed(&mut self, msg: impl Into<String>) {
        self.console_line.send(PrintConsoleLine::new(msg.into()));
        self.failed();
    }
}

pub struct ConsoleCommandState<T> {
    #[allow(clippy::type_complexity)]
    event_reader: <ConsoleCommandEnteredReaderSystemParam as SystemParam>::State,
    console_line: <PrintConsoleLineWriterSystemParam as SystemParam>::State,
    marker: PhantomData<T>,
}

unsafe impl<T: Command> SystemParam for ConsoleCommand<'_, T> {
    type State = ConsoleCommandState<T>;
    type Item<'w, 's> = ConsoleCommand<'w, T>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let event_reader = ConsoleCommandEnteredReaderSystemParam::init_state(world, system_meta);
        let console_line = PrintConsoleLineWriterSystemParam::init_state(world, system_meta);
        ConsoleCommandState {
            event_reader,
            console_line,
            marker: PhantomData,
        }
    }

    #[inline]
    unsafe fn get_param<'w, 's>(
        state: &'s mut Self::State,
        system_meta: &SystemMeta,
        world: UnsafeWorldCell<'w>,
        change_tick: Tick,
    ) -> Self::Item<'w, 's> {
        let mut event_reader = ConsoleCommandEnteredReaderSystemParam::get_param(
            &mut state.event_reader,
            system_meta,
            world,
            change_tick,
        );
        let mut console_line = PrintConsoleLineWriterSystemParam::get_param(
            &mut state.console_line,
            system_meta,
            world,
            change_tick,
        );

        let command = event_reader.read().find_map(|command| {
            if T::name() == command.command_name {
                let clap_command = T::command().no_binary_name(true);
                // .color(clap::ColorChoice::Always);
                let arg_matches = clap_command.try_get_matches_from(command.args.iter());

                debug!(
                    "Trying to parse as `{}`. Result: {arg_matches:?}",
                    command.command_name
                );

                match arg_matches {
                    Ok(matches) => {
                        return Some(T::from_arg_matches(&matches));
                    }
                    Err(err) => {
                        console_line.send(PrintConsoleLine::new(err.to_string()));
                        return Some(Err(err));
                    }
                }
            }
            None
        });

        ConsoleCommand {
            command,
            console_line,
        }
    }
}
/// Parsed raw console command into `command` and `args`.
#[derive(Clone, Debug, Event)]
pub struct ConsoleCommandEntered {
    /// the command definition
    pub command_name: String,
    /// Raw parsed arguments
    pub args: Vec<String>,
}

/// Events to print to the console.
#[derive(Clone, Debug, Eq, Event, PartialEq)]
pub struct PrintConsoleLine {
    /// Console line
    pub line: String,
}

impl PrintConsoleLine {
    /// Creates a new console line to print.
    pub const fn new(line: String) -> Self {
        Self { line }
    }
}

/// Console configuration
#[derive(Clone, Resource)]
pub struct ConsoleConfiguration {
    /// Registered keys for toggling the console
    pub keys: Vec<KeyCode>,
    /// Left position
    pub left_pos: f32,
    /// Top position
    pub top_pos: f32,
    /// Console height
    pub height: f32,
    /// Console width
    pub width: f32,
    /// Registered console commands
    pub commands: BTreeMap<&'static str, clap::Command>,
    /// Number of commands to store in history
    pub history_size: usize,
    /// Line prefix symbol
    pub symbol: String,
    /// allows window to be collpased
    pub collapsible: bool,
    /// Title name of console window
    pub title_name: String,
    /// allows window to be resizable
    pub resizable: bool,
    /// allows window to be movable
    pub moveable: bool,
    /// show the title bar or not
    pub show_title_bar: bool,
    /// Background color of console window  
    pub background_color: Colour,
    /// Foreground (text) color
    pub foreground_color: Colour,
    /// Number of suggested commands to show
    pub num_suggestions: usize,
}

impl Default for ConsoleConfiguration {
    fn default() -> Self {
        Self {
            keys: vec![KeyCode::Backquote],
            left_pos: 200.0,
            top_pos: 100.0,
            height: 400.0,
            width: 800.0,
            commands: BTreeMap::new(),
            history_size: 20,
            symbol: "$ ".to_owned(),
            collapsible: false,
            title_name: "Console".to_string(),
            resizable: true,
            moveable: true,
            show_title_bar: true,
            background_color: Colour::from_rgb(102, 102, 102),
            foreground_color: Colour::from_rgb(220, 220, 220),
            num_suggestions: 4,
        }
    }
}

/// Add a console commands to Bevy app.
pub trait AddConsoleCommand {
    /// Add a console command with a given system.
    ///
    /// This registers the console command so it will print with the built-in `help` console command.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_console::{AddConsoleCommand, ConsoleCommand};
    /// # use clap::Parser;
    /// App::new()
    ///     .add_console_command::<LogCommand, _>(log_command);
    /// #
    /// # /// Prints given arguments to the console.
    /// # #[derive(Parser, ConsoleCommand)]
    /// # #[command(name = "log")]
    /// # struct LogCommand;
    /// #
    /// # fn log_command(mut log: ConsoleCommand<LogCommand>) {}
    /// ```
    fn add_console_command<T: Command, Params>(
        &mut self,
        system: impl IntoSystemConfigs<Params>,
    ) -> &mut Self;
}

impl AddConsoleCommand for App {
    fn add_console_command<T: Command, Params>(
        &mut self,
        system: impl IntoSystemConfigs<Params>,
    ) -> &mut Self {
        let sys = move |mut config: ResMut<ConsoleConfiguration>| {
            let command = T::command().no_binary_name(true);
            // .color(clap::ColorChoice::Always);
            let name = T::name();
            if config.commands.contains_key(name) {
                warn!(
                    "console command '{}' already registered and was overwritten",
                    name
                );
            }
            config.commands.insert(name, command);
        };

        self.add_systems(Startup, sys)
            .add_systems(Update, system.in_set(ConsoleSet::Commands))
    }
}

/// Console open state
#[derive(Default, Resource)]
pub struct ConsoleOpen {
    /// Console open
    pub open: bool,
}

#[derive(Resource)]
pub(crate) struct ConsoleState {
    pub(crate) buf: String,
    pub(crate) scrollback: Vec<String>,
    pub(crate) history: VecDeque<String>,
    pub(crate) history_index: usize,
}

impl Default for ConsoleState {
    fn default() -> Self {
        ConsoleState {
            buf: String::default(),
            scrollback: Vec::new(),
            history: VecDeque::from([String::new()]),
            history_index: 0,
        }
    }
}

#[cfg(feature = "ui")]
fn default_style(config: &ConsoleConfiguration) -> TextFormat {
    TextFormat::simple(FontId::monospace(14f32), config.foreground_color.into())
}

#[cfg(feature = "ui")]
fn style_ansi_text(str: &str, config: &ConsoleConfiguration) -> LayoutJob {
    let mut layout_job = LayoutJob::default();
    let mut current_style = default_style(config);
    let mut last_offset = 0;
    let str_without_ansi = strip_ansi_escapes::strip_str(str);
    for (offset, overrides) in parse_ansi_styled_str(str)
        .into_iter()
        .chain(once((str_without_ansi.len(), Default::default())))
    {
        // 1<red>2345</red>
        // 01234
        let text = &str_without_ansi[(last_offset)..offset];
        if !text.is_empty() {
            layout_job.append(text, 0f32, current_style.clone());
        }

        if overrides.contains(&TextFormattingOverride::Reset) {
            current_style = default_style(config);
        }

        for o in overrides {
            match o {
                TextFormattingOverride::Bold => current_style.font_id.size = 16f32, // no support for bold font families in egui TODO: when egui supports bold font families, use them here
                TextFormattingOverride::Dim => current_style.font_id.size = 12f32, // no support for dim font families in egui TODO: when egui supports dim font families, use them here
                TextFormattingOverride::Italic => current_style.italics = true,
                TextFormattingOverride::Underline => {
                    current_style.underline = egui::Stroke::new(1., config.foreground_color)
                }
                TextFormattingOverride::Strikethrough => {
                    current_style.strikethrough = egui::Stroke::new(1., config.foreground_color)
                }
                TextFormattingOverride::Foreground(c) => current_style.color = c.into(),
                TextFormattingOverride::Background(c) => current_style.background = c.into(),
                _ => {}
            }
        }

        last_offset = offset;
    }
    layout_job
}

#[cfg(feature = "ui")]
pub(crate) fn console_ui(
    mut egui_context: EguiContexts,
    config: Res<ConsoleConfiguration>,
    mut keyboard_input_events: EventReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<ConsoleState>,
    mut command_entered: EventWriter<ConsoleCommandEntered>,
    mut console_open: ResMut<ConsoleOpen>,
) {
    let keyboard_input_events = keyboard_input_events.read().collect::<Vec<_>>();

    // If there is no egui context, return, this can happen when exiting the app
    let ctx = if let Some(ctxt) = egui_context.try_ctx_mut() {
        ctxt
    } else {
        return;
    };

    let pressed = keyboard_input_events
        .iter()
        .any(|code| console_key_pressed(code, &config.keys));

    // always close if console open
    // avoid opening console if typing in another text input
    if pressed && (console_open.open || !ctx.wants_keyboard_input()) {
        console_open.open = !console_open.open;
    }

    if console_open.open {
        egui::Window::new(&config.title_name)
            .collapsible(config.collapsible)
            .default_pos([config.left_pos, config.top_pos])
            .default_size([config.width, config.height])
            .resizable(config.resizable)
            .movable(config.moveable)
            .title_bar(config.show_title_bar)
            .frame(egui::Frame {
                fill: config.background_color.into(),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.style_mut().visuals.extreme_bg_color = config.background_color.into();
                ui.style_mut().visuals.override_text_color = Some(config.foreground_color.into());

                ui.vertical(|ui| {
                    let scroll_height = ui.available_height() - 30.0;

                    // Scroll area
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .max_height(scroll_height)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                for line in &state.scrollback {
                                    ui.label(style_ansi_text(line, &config));
                                }
                            });

                            // Scroll to bottom if console just opened
                            if console_open.is_changed() {
                                ui.scroll_to_cursor(Some(Align::BOTTOM));
                            }
                        });

                    // Separator
                    ui.separator();

                    // Input
                    let text_edit = TextEdit::singleline(&mut state.buf)
                        .desired_width(f32::INFINITY)
                        .lock_focus(true)
                        .font(egui::TextStyle::Monospace);

                    let text_edit_response = ui.add(text_edit);

                    // show a few suggestions
                    if text_edit_response.has_focus() && !state.buf.is_empty() {
                        // create the area to show suggestions
                        let suggestions_area = egui::Area::new(ui.auto_id_with("suggestions"))
                            .fixed_pos(ui.next_widget_position())
                            .movable(false);
                        suggestions_area.show(ui.ctx(), |ui| {
                            // collect the given number of commands starting
                            // with the given text
                            let command_names = &config
                                .commands
                                .iter()
                                .map(|c| *c.0)
                                .filter(|c| c.starts_with(&state.buf))
                                .collect::<Vec<_>>();

                            // show each command in the list
                            for command in command_names.iter().take(config.num_suggestions) {
                                let mut layout_job = egui::text::LayoutJob::default();
                                layout_job.append(
                                    state.buf.as_str(),
                                    0.0,
                                    TextFormat {
                                        font_id: FontId::new(14.0, egui::FontFamily::Monospace),
                                        underline: egui::Stroke::new(1., Color32::WHITE),
                                        color: Color32::WHITE,
                                        ..default()
                                    },
                                );
                                layout_job.append(
                                    &command[state.buf.len()..],
                                    0.0,
                                    TextFormat {
                                        font_id: FontId::new(14.0, egui::FontFamily::Monospace),
                                        color: Color32::LIGHT_GRAY,
                                        ..default()
                                    },
                                );
                                ui.label(layout_job);
                            }
                        });
                    }

                    // Handle enter
                    if text_edit_response.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        if state.buf.trim().is_empty() {
                            state.scrollback.push(String::new());
                        } else {
                            let msg = format!("{}{}", config.symbol, state.buf);
                            state.scrollback.push(msg);
                            let cmd_string = state.buf.clone();
                            state.history.insert(1, cmd_string);
                            if state.history.len() > config.history_size + 1 {
                                state.history.pop_back();
                            }

                            let mut args = Shlex::new(&state.buf).collect::<Vec<_>>();

                            if !args.is_empty() {
                                let command_name = args.remove(0);
                                debug!("Command entered: `{command_name}`, with args: `{args:?}`");

                                let command = config.commands.get(command_name.as_str());

                                if command.is_some() {
                                    command_entered
                                        .send(ConsoleCommandEntered { command_name, args });
                                } else {
                                    debug!(
                                        "Command not recognized, recognized commands: `{:?}`",
                                        config.commands.keys().collect::<Vec<_>>()
                                    );

                                    state.scrollback.push("error: Invalid command".into());
                                }
                            }

                            state.buf.clear();
                        }
                    }

                    // Clear on ctrl+l
                    if keyboard_input_events
                        .iter()
                        .any(|&k| k.state.is_pressed() && k.key_code == KeyCode::KeyL)
                        && (keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]))
                    {
                        state.scrollback.clear();
                    }

                    // Handle up and down through history
                    if text_edit_response.has_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::ArrowUp))
                        && state.history.len() > 1
                        && state.history_index < state.history.len() - 1
                    {
                        if state.history_index == 0 && !state.buf.trim().is_empty() {
                            *state.history.get_mut(0).unwrap() = state.buf.clone();
                        }

                        state.history_index += 1;
                        let previous_item = state.history.get(state.history_index).unwrap().clone();
                        state.buf = previous_item.to_string();

                        set_cursor_pos(ui.ctx(), text_edit_response.id, state.buf.len());
                    } else if text_edit_response.has_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::ArrowDown))
                        && state.history_index > 0
                    {
                        state.history_index -= 1;
                        let next_item = state.history.get(state.history_index).unwrap().clone();
                        state.buf = next_item.to_string();

                        set_cursor_pos(ui.ctx(), text_edit_response.id, state.buf.len());
                    }

                    // Focus on input
                    ui.memory_mut(|m| m.request_focus(text_edit_response.id));
                });
            });
    }
}

pub(crate) fn receive_console_line(
    mut console_state: ResMut<ConsoleState>,
    mut events: EventReader<PrintConsoleLine>,
) {
    for event in events.read() {
        let event: &PrintConsoleLine = event;
        console_state.scrollback.push(event.line.clone());
    }
}

fn console_key_pressed(keyboard_input: &KeyboardInput, configured_keys: &[KeyCode]) -> bool {
    if !keyboard_input.state.is_pressed() {
        return false;
    }

    for configured_key in configured_keys {
        if configured_key == &keyboard_input.key_code {
            return true;
        }
    }

    false
}

#[cfg(feature = "ui")]
fn set_cursor_pos(ctx: &Context, id: Id, pos: usize) {
    if let Some(mut state) = TextEdit::load_state(ctx, id) {
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(pos))));
        state.store(ctx, id);
    }
}

#[cfg(test)]
mod tests {
    use bevy::input::keyboard::{Key, NativeKey, NativeKeyCode};
    use bevy::input::ButtonState;

    use super::*;

    #[test]
    fn test_console_key_pressed_scan_code() {
        let input = KeyboardInput {
            key_code: KeyCode::Unidentified(NativeKeyCode::Xkb(41)),
            logical_key: Key::Unidentified(NativeKey::Xkb(41)),
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        };

        let config = vec![KeyCode::Unidentified(NativeKeyCode::Xkb(41))];

        let result = console_key_pressed(&input, &config);
        assert!(result);
    }

    #[test]
    fn test_console_wrong_key_pressed_scan_code() {
        let input = KeyboardInput {
            key_code: KeyCode::Unidentified(NativeKeyCode::Xkb(42)),
            logical_key: Key::Unidentified(NativeKey::Xkb(42)),
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        };

        let config = vec![KeyCode::Unidentified(NativeKeyCode::Xkb(41))];

        let result = console_key_pressed(&input, &config);
        assert!(!result);
    }

    #[test]
    fn test_console_key_pressed_key_code() {
        let input = KeyboardInput {
            key_code: KeyCode::Backquote,
            logical_key: Key::Character("`".into()),
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        };

        let config = vec![KeyCode::Backquote];

        let result = console_key_pressed(&input, &config);
        assert!(result);
    }

    #[test]
    fn test_console_wrong_key_pressed_key_code() {
        let input = KeyboardInput {
            key_code: KeyCode::KeyA,
            logical_key: Key::Character("A".into()),
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        };

        let config = vec![KeyCode::Backquote];

        let result = console_key_pressed(&input, &config);
        assert!(!result);
    }

    #[test]
    fn test_console_key_right_key_but_not_pressed() {
        let input = KeyboardInput {
            key_code: KeyCode::Backquote,
            logical_key: Key::Character("`".into()),
            state: ButtonState::Released,
            window: Entity::PLACEHOLDER,
        };

        let config = vec![KeyCode::Backquote];

        let result = console_key_pressed(&input, &config);
        assert!(!result);
    }
}
