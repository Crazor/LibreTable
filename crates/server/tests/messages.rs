//! The server's message IDs: all have English messages, all are listed, and
//! the web page uses no ID outside the list (docs/design/localization.md).

use libretable_l10n::Localizer;
use libretable_server::MESSAGE_IDS;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{ROOT}/{path}")).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn every_server_message_exists_in_english() {
    assert_eq!(
        Localizer::new("en").missing(MESSAGE_IDS),
        Vec::<&str>::new()
    );
}

#[test]
fn every_server_message_is_listed() {
    for line in read("locales/en/server.ftl").lines() {
        if let Some((id, _)) = line.split_once(" =") {
            assert!(MESSAGE_IDS.contains(&id), "{id} not in MESSAGE_IDS");
        }
    }
}

#[test]
fn ids_are_unique() {
    for (index, id) in MESSAGE_IDS.iter().enumerate() {
        assert!(!MESSAGE_IDS[index + 1..].contains(id), "{id} twice");
    }
}

/// Every `server-…` token in a page file.
fn page_ids(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("server-") {
        let before = rest[..start].chars().next_back();
        let tail = &rest[start..];
        let end = tail
            .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
            .unwrap_or(tail.len());
        if !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '-') {
            ids.push(tail[..end].trim_end_matches('-').to_owned());
        }
        rest = &tail[end.max(1)..];
    }
    ids
}

#[test]
fn the_page_uses_only_listed_ids() {
    let mut found = 0;
    for file in ["apps/web/dev/index.html", "apps/web/dev/app.js"] {
        for id in page_ids(&read(file)) {
            assert!(MESSAGE_IDS.contains(&id.as_str()), "{file}: {id}");
            found += 1;
        }
    }
    assert!(found > 30, "the scan found the page's IDs");
}
