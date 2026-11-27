//! A client driving a [`Connection`] without networking: sends messages and
//! collects the replies plus the match changes it would receive.

#![allow(dead_code)]

use std::sync::Arc;

use libretable_game_core::Choice;
use libretable_server::protocol::{ClientMessage, Driver, MatchState, ServerMessage, WireObserver};
use libretable_server::{Change, Connection, Server};
use tokio::sync::broadcast;

pub struct Client {
    connection: Connection,
    changes: Option<broadcast::Receiver<Change>>,
    subscribed: Option<u64>,
}

impl Client {
    pub fn new(server: &Arc<Server>) -> Self {
        Self {
            connection: Connection::new(Arc::clone(server)),
            changes: None,
            subscribed: None,
        }
    }

    /// Sends `message`; returns the replies followed by what this client
    /// receives for the changes it caused.
    pub fn send(&mut self, message: ClientMessage) -> Vec<ServerMessage> {
        let mut out = self.connection.handle(message);
        if self.connection.match_id() != self.subscribed {
            self.subscribed = self.connection.match_id();
            self.changes = self.connection.changes();
        }
        out.extend(self.receive());
        out
    }

    /// Sends a raw text frame.
    pub fn send_text(&mut self, text: &str) -> Vec<ServerMessage> {
        self.connection.receive(text)
    }

    /// What arrived for changes made by any connection.
    pub fn receive(&mut self) -> Vec<ServerMessage> {
        let mut out = Vec::new();
        if let Some(changes) = &mut self.changes {
            while let Ok(change) = changes.try_recv() {
                out.extend(self.connection.on_change(&change));
            }
        }
        out
    }
}

pub fn create(seats: u8, drivers: Option<Vec<Driver>>) -> ClientMessage {
    ClientMessage::Create {
        game: "rock-paper-scissors".to_owned(),
        seats,
        seed: Some(7),
        variant: None,
        drivers,
        observers: None,
    }
}

pub fn create_observing(observers: Vec<WireObserver>) -> ClientMessage {
    ClientMessage::Create {
        game: "rock-paper-scissors".to_owned(),
        seats: 2,
        seed: Some(7),
        variant: None,
        drivers: None,
        observers: Some(observers),
    }
}

/// The last `state` among `messages`.
pub fn last_state(messages: &[ServerMessage]) -> MatchState {
    messages
        .iter()
        .rev()
        .find_map(|message| match message {
            ServerMessage::State(state) => Some(state.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no state in {messages:?}"))
}

/// The codes of every `error` among `messages`.
pub fn errors(messages: &[ServerMessage]) -> Vec<String> {
    messages
        .iter()
        .filter_map(|message| match message {
            ServerMessage::Error(error) => Some(error.code.clone()),
            _ => None,
        })
        .collect()
}

/// Plays the first selectable choice of the first prompt until the match
/// ends; returns the final state.
pub fn play_out(client: &mut Client, mut state: MatchState) -> MatchState {
    for _ in 0..1000 {
        if state.outcome.is_some() {
            return state;
        }
        let prompt = state.prompts.first().expect("a human seat to act").clone();
        let replies = if prompt.complete {
            client.send(ClientMessage::Submit { seat: prompt.seat })
        } else {
            client.send(ClientMessage::Choose {
                seat: prompt.seat,
                choice: prompt.selectable[0].clone(),
            })
        };
        assert_eq!(errors(&replies), Vec::<String>::new());
        state = last_state(&replies);
    }
    panic!("the match did not end");
}

pub fn rock() -> Choice {
    Choice::Option("rps-rock".to_owned())
}
