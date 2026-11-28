//! End-to-end tests of the `table` binary: simulate, play, replay and errors.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const RPS: &str = "rock-paper-scissors";

fn table(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_table"))
        .args(args)
        // Tests expect English whatever the machine's locale.
        .env("LC_ALL", "en_US.UTF-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary starts");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn temp(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}

#[test]
fn lists_games() {
    assert_eq!(
        stdout(&table(&["games"], "")),
        "Games:\n  mau-mau: Mau-Mau\n  rock-paper-scissors: Rock, Paper, Scissors\n"
    );
}

#[test]
fn simulate_is_deterministic_and_summarizes() {
    let args = ["simulate", RPS, "-n", "200", "--seats", "3", "--seed", "4"];
    let first = stdout(&table(&args, ""));
    assert_eq!(first, stdout(&table(&args, "")));
    let lines: Vec<&str> = first.lines().collect();
    assert_eq!(
        lines[0],
        "200 matches of Rock, Paper, Scissors with 3 seats, 1800 moves in total."
    );
    assert_eq!(lines.len(), 4);
    for (seat, line) in lines[1..].iter().enumerate() {
        assert!(line.starts_with(&format!("  Seat {seat}: ")), "{line}");
    }
    assert_ne!(
        first,
        stdout(&table(
            &["simulate", RPS, "-n", "200", "--seats", "3", "--seed", "5"],
            ""
        ))
    );
}

#[test]
fn bot_match_records_and_replays() {
    let path = temp("bot-match.json");
    let path_text = path.to_str().unwrap();
    let play = stdout(&table(
        &[
            "play", RPS, "--seats", "bot,bot", "--seed", "7", "--record", path_text,
        ],
        "",
    ));
    assert!(play.starts_with(
        "Playing Rock, Paper, Scissors with 2 seats, seed 7.\nSeat 0: bot\nSeat 1: bot\n"
    ));
    assert!(play.contains("Result:\n"));
    assert!(play.ends_with(&format!("Record written to {path_text}.\n")));

    let record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(record["game"], RPS);
    assert_eq!(record["moves"].as_array().unwrap().len(), 6);

    let replay = stdout(&table(&["replay", path_text, "--seat", "1"], ""));
    let lines: Vec<&str> = replay.lines().collect();
    assert_eq!(
        lines[0],
        "Replaying Rock, Paper, Scissors with 2 seats, 6 moves."
    );
    for (number, line) in (1..=6).zip(&lines[1..7]) {
        let seat = (number + 1) % 2;
        assert!(
            line.starts_with(&format!("Move {number}: seat {seat}: ")),
            "{line}"
        );
    }
    assert_eq!(lines[7], "--- Table: seat 1 ---");
    // The outcome after replay is the outcome the match ended with.
    let result = |text: &str| {
        text[text.find("Result:").unwrap()..]
            .lines()
            .take(3)
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(result(&replay), result(&play));

    let each = stdout(&table(&["replay", path_text, "--each"], ""));
    assert_eq!(each.matches("--- Table: spectator ---").count(), 6);
}

#[test]
fn human_input_goes_through_the_selection_protocol() {
    // Invalid input is refused, then rock, then paper; one round.
    let out = stdout(&table(
        &[
            "play",
            RPS,
            "--seats",
            "human,bot",
            "--seed",
            "3",
            "--variant",
            r#"{"rounds":1}"#,
        ],
        "x\n0\n4\n1\n",
    ));
    assert!(out.contains(
        "Seat 0, choose:\n  1) Rock\n  2) Paper\n  3) Scissors\nEnter a number, u to undo, q to quit:\n"
    ));
    assert_eq!(
        out.matches("Please enter a number from 1 to 3, u or q.")
            .count(),
        3
    );
    assert!(out.contains("Seat 1 (bot) has decided.\n"));
    assert!(out.contains("    #64 token: Rock (shape: rock)\n"));
    assert!(out.contains("Result:\n"));
}

#[test]
fn abandoned_match_writes_a_partial_record() {
    let path = temp("abandoned.json");
    let path_text = path.to_str().unwrap();
    let out = stdout(&table(
        &[
            "play",
            RPS,
            "--seats",
            "human,human",
            "--seed",
            "1",
            "--record",
            path_text,
        ],
        "\n2\n\nq\n",
    ));
    assert!(out.contains("Pass the device to seat 0 and press Enter.\n"));
    assert!(out.contains("Pass the device to seat 1 and press Enter.\n"));
    // Seat 1 sees that seat 0 has thrown, not what.
    assert!(out.contains("--- Table: seat 1 ---"));
    assert!(out.contains("    #0 token: face down\n"));
    assert!(out.contains("Match abandoned.\n"));

    let replay = stdout(&table(&["replay", path_text], ""));
    assert!(replay.contains("Move 1: seat 0: Paper\n"));
    assert!(replay.ends_with("The match is not finished.\n"));
}

#[test]
fn errors_exit_with_a_message() {
    let unknown = table(&["play", "chess"], "");
    assert_eq!(unknown.status.code(), Some(2));
    let stderr = String::from_utf8(unknown.stderr).unwrap();
    assert!(stderr.starts_with("Error: Unknown game \"chess\"."));

    let variant = table(&["simulate", RPS, "--variant", r#"{"rounds":0}"#], "");
    assert_eq!(variant.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(variant.stderr).unwrap(),
        "Error: Invalid variant: Play at least one round.\n"
    );

    let path = temp("broken.json");
    let mut record: serde_json::Value = serde_json::from_str(
        r#"{"schema":1,"game":"rock-paper-scissors","build":"t","variant":{"rounds":1},"seats":2,"seed":"0000000000000000000000000000000000000000000000000000000000000000","moves":[{"seat":0,"move":"rock"},{"seat":0,"move":"paper"}]}"#,
    )
    .unwrap();
    std::fs::write(&path, record.to_string()).unwrap();
    let broken = table(&["replay", path.to_str().unwrap()], "");
    assert_eq!(broken.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(broken.stderr).unwrap(),
        "Error: Move 2 was recorded for a seat that could not act.\n"
    );
    assert!(broken.stdout.is_empty());

    record["schema"] = 99.into();
    std::fs::write(&path, record.to_string()).unwrap();
    let schema = table(&["replay", path.to_str().unwrap()], "");
    assert_eq!(
        String::from_utf8(schema.stderr).unwrap(),
        "Error: The record uses schema version 99; this build reads version 1.\n"
    );
}

#[test]
fn locale_comes_from_lang_option_then_environment() {
    let german = stdout(&table(&["--lang", "de", "games"], ""));
    assert_eq!(
        german,
        "Spiele:\n  mau-mau: Mau-Mau\n  rock-paper-scissors: Schere, Stein, Papier\n"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_table"))
        .args(["simulate", RPS, "-n", "10"])
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "de_AT.UTF-8")
        .output()
        .unwrap();
    assert!(
        stdout(&output)
            .starts_with("10 Partien Schere, Stein, Papier mit 2 Plätzen, insgesamt 60 Züge.\n")
    );

    // An unsupported locale falls back to English.
    let fallback = stdout(&table(&["games", "--lang", "fr"], ""));
    assert!(fallback.starts_with("Games:\n"));
}

#[test]
fn mau_mau_simulates_and_its_golden_replays_replay() {
    let out = stdout(&table(
        &["simulate", "mau-mau", "-n", "5", "--seats", "3"],
        "",
    ));
    assert!(
        out.starts_with("5 matches of Mau-Mau with 3 seats"),
        "{out}"
    );
    let golden = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../games/mau-mau/tests/golden/two-players-standard.json"
    );
    let out = stdout(&table(
        &["--lang", "de", "replay", golden, "--seat", "0"],
        "",
    ));
    assert!(out.contains("Herz-Bube"), "{out}");
    assert!(
        out.contains("Ergebnis:\n  Platz 0: Rang 1, Punkte 1\n"),
        "{out}"
    );
}
