//! The `dev` feature gate: only a dev build serves the omniscient view, the
//! record with its seed and game events. CI runs this file with and without
//! `--features dev`.

mod common;

use common::{Client, create, last_state, rock};
use libretable_server::protocol::{ClientMessage, Driver, ServerMessage, WireObserver, encode};
use libretable_server::{DEV, Server};

/// Everything a connection receives while bots play a whole match, and while
/// a human seat moves, as the text frames that would go over the wire.
fn frames_of_a_match() -> Vec<String> {
    let server = Server::new("test");
    let mut client = Client::new(&server);
    let mut messages = client.send(ClientMessage::Hello { locale: None });
    messages.extend(client.send(create(2, None)));
    messages.extend(client.send(ClientMessage::Choose {
        seat: 0,
        choice: rock(),
    }));
    messages.extend(client.send(ClientMessage::Submit { seat: 0 }));
    messages.extend(client.send(ClientMessage::SetDriver {
        seat: 1,
        driver: Driver::Bot,
    }));
    messages.extend(client.send(ClientMessage::SetDriver {
        seat: 0,
        driver: Driver::Bot,
    }));
    messages.extend(client.send(ClientMessage::Seek { position: Some(1) }));
    assert!(last_state(&messages).length == 6);
    messages.iter().map(encode).collect()
}

#[cfg(not(feature = "dev"))]
mod without_dev {
    use super::*;
    use common::{create_observing, errors};

    // The crate's own flag agrees with the feature, at compile time.
    const _: () = assert!(!DEV);

    #[test]
    fn omniscient_is_refused() {
        let server = Server::new("test");
        let mut client = Client::new(&server);
        assert_eq!(
            errors(&client.send(create_observing(vec![
                WireObserver::Seat(0),
                WireObserver::Omniscient,
            ]))),
            ["server-error-omniscient-unavailable"]
        );

        let replies = client.send(create(2, None));
        let ServerMessage::Joined {
            match_id,
            observers,
            seed,
            ..
        } = &replies[0]
        else {
            panic!("{replies:?}");
        };
        assert_eq!(
            observers,
            &[
                WireObserver::Seat(0),
                WireObserver::Seat(1),
                WireObserver::Spectator
            ]
        );
        assert_eq!(*seed, None);
        assert_eq!(
            errors(&client.send(ClientMessage::Join {
                match_id: *match_id,
                observers: Some(vec![WireObserver::Omniscient]),
            })),
            ["server-error-omniscient-unavailable"]
        );
    }

    #[test]
    fn no_dev_data_goes_over_the_wire() {
        for frame in frames_of_a_match() {
            // As a JSON string: the observer, not the page's messages.
            assert!(!frame.contains(r#""omniscient""#), "{frame}");
            assert!(!frame.contains(r#""type":"events""#), "{frame}");
            assert!(!frame.contains(r#""record""#), "{frame}");
            assert!(!frame.contains(r#""seed""#), "{frame}");
        }
    }
}

#[cfg(feature = "dev")]
mod with_dev {
    use super::*;
    use libretable_game_core::Face;

    // The crate's own flag agrees with the feature, at compile time.
    const _: () = assert!(DEV);

    #[test]
    fn omniscient_shows_what_seats_cannot_see() {
        let server = Server::new("test");
        let mut client = Client::new(&server);
        let replies = client.send(create(2, None));
        let ServerMessage::Joined {
            observers, seed, ..
        } = &replies[0]
        else {
            panic!("{replies:?}");
        };
        assert_eq!(observers.last(), Some(&WireObserver::Omniscient));
        assert_eq!(*seed, Some(7));

        client.send(ClientMessage::Choose {
            seat: 0,
            choice: rock(),
        });
        let replies = client.send(ClientMessage::Submit { seat: 0 });
        assert!(replies.iter().any(|message| matches!(
            message,
            ServerMessage::Events {
                position: 1,
                seat: 0,
                ..
            }
        )));
        let state = last_state(&replies);
        let throw_of_seat_0 = |observer: WireObserver| {
            let view = state
                .views
                .iter()
                .find(|view| view.observer == observer)
                .expect("observed");
            view.table.zones[0].components[0].face.clone()
        };
        assert_eq!(throw_of_seat_0(WireObserver::Seat(1)), Face::Hidden);
        assert_eq!(throw_of_seat_0(WireObserver::Spectator), Face::Hidden);
        assert_ne!(throw_of_seat_0(WireObserver::Omniscient), Face::Hidden);
        assert_ne!(throw_of_seat_0(WireObserver::Seat(0)), Face::Hidden);

        let record = state.record.expect("dev builds send the record");
        assert_eq!(record["moves"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn dev_data_goes_over_the_wire() {
        let frames = frames_of_a_match();
        assert!(frames.iter().any(|frame| frame.contains(r#""omniscient""#)));
        assert!(
            frames
                .iter()
                .any(|frame| frame.contains(r#""type":"events""#))
        );
        assert!(frames.iter().any(|frame| frame.contains(r#""record""#)));
    }
}
