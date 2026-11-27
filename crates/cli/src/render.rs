//! The text renderer: a seat's table model, the selectable choices and the
//! outcome as plain text (architecture: table model & rendering).
//!
//! It is generic: it knows the table model and the selection protocol, never
//! a game. Output is deterministic, so tests compare it with expected
//! strings. All words come from [`tr`](crate::l10n::tr).

use libretable_game_core::{
    Choice, Component, ComponentKind, Face, Indicator, IndicatorValue, Layout, Observer, Outcome,
    Seat, TableModel, Visibility, Zone, ZoneKind,
};

use crate::tr;

/// Who a rendering is for, as text: "seat 1" or "spectator".
#[must_use]
pub fn observer(observer: Observer) -> String {
    match observer.seat() {
        Some(seat) => tr!("cli-observer-seat", seat = seat.index()),
        None => tr!("cli-observer-spectator"),
    }
}

/// The table model as seen by `viewer`: a header, the indicators, then every
/// zone with its components, one per line.
#[must_use]
pub fn table(table: &TableModel, viewer: Observer) -> String {
    let mut lines = vec![tr!("cli-table-header", observer = observer(viewer))];
    lines.extend(table.indicators.iter().map(indicator));
    for zone in &table.zones {
        lines.push(zone_header(zone));
        if zone.components.is_empty() {
            lines.push(format!("    {}", tr!("cli-zone-empty")));
        }
        lines.extend(
            zone.components
                .iter()
                .map(|item| format!("    {}", component(item))),
        );
    }
    lines.join("\n") + "\n"
}

/// The prompt for `seat`: the choices made so far and the numbered
/// selectable choices.
#[must_use]
pub fn prompt(table: &TableModel, seat: Seat, made: &[Choice], selectable: &[Choice]) -> String {
    let mut lines = Vec::new();
    if !made.is_empty() {
        lines.push(tr!("cli-prompt-made", choices = choices(table, made)));
    }
    lines.push(tr!("cli-prompt", seat = seat.index()));
    lines.extend(selectable.iter().zip(1_usize..).map(|(item, number)| {
        let item = tr!(
            "cli-prompt-item",
            number = number,
            choice = choice(table, item)
        );
        format!("  {item}")
    }));
    lines.push(tr!("cli-prompt-input"));
    lines.join("\n") + "\n"
}

/// A choice path, e.g. "Play > Hearts".
#[must_use]
pub fn choices(table: &TableModel, path: &[Choice]) -> String {
    if path.is_empty() {
        return tr!("cli-choice-none");
    }
    path.iter()
        .map(|item| choice(table, item))
        .collect::<Vec<_>>()
        .join(" > ")
}

/// One choice, described with the help of the chooser's table.
#[must_use]
pub fn choice(table: &TableModel, choice: &Choice) -> String {
    let component_in_table = |id| match table.component(id) {
        Some((zone, item)) => tr!(
            "cli-choice-component",
            component = component(item),
            zone = tr!(&zone.label)
        ),
        None => format!("#{}", id.0),
    };
    match choice {
        Choice::Component(id) => component_in_table(*id),
        Choice::Zone(id) => match table.zone(*id) {
            Some(zone) => tr!("cli-choice-zone", id = id.0, zone = tr!(&zone.label)),
            None => format!("[{}]", id.0),
        },
        Choice::Option(id) => tr!(id),
        Choice::Number(n) => n.to_string(),
        Choice::Subset(set) => {
            let members: Vec<String> = set
                .as_slice()
                .iter()
                .map(|id| component_in_table(*id))
                .collect();
            tr!("cli-choice-subset", components = members.join("; "))
        }
    }
}

/// The result of a finished match, one seat per line.
#[must_use]
pub fn outcome(outcome: &Outcome) -> String {
    let mut lines = vec![tr!("cli-outcome-header")];
    lines.extend(outcome.seats.iter().zip(0_u8..).map(|(result, seat)| {
        let line = tr!(
            "cli-outcome-seat",
            seat = seat,
            rank = result.rank,
            score = result.score
        );
        format!("  {line}")
    }));
    lines.extend(outcome.teams.iter().map(|team| {
        let seats: Vec<String> = team.iter().map(|seat| seat.index().to_string()).collect();
        format!("  {}", tr!("cli-outcome-team", seats = seats.join(", ")))
    }));
    lines.join("\n") + "\n"
}

fn owner(owner: Option<Seat>) -> String {
    match owner {
        Some(seat) => tr!("cli-owner-seat", seat = seat.index()),
        None => tr!("cli-owner-shared"),
    }
}

fn indicator(indicator: &Indicator) -> String {
    let value = match &indicator.value {
        IndicatorValue::Number(n) => n.to_string(),
        IndicatorValue::Seat(seat) => tr!("cli-owner-seat", seat = seat.index()),
        IndicatorValue::Message(id) => tr!(id),
    };
    let label = tr!(&indicator.label);
    match indicator.owner {
        Some(_) => tr!(
            "cli-indicator-owned",
            label = label,
            owner = owner(indicator.owner),
            value = value
        ),
        None => tr!("cli-indicator", label = label, value = value),
    }
}

fn zone_header(zone: &Zone) -> String {
    let kind = match zone.kind {
        ZoneKind::DrawPile => "cli-zone-kind-draw-pile",
        ZoneKind::Hand => "cli-zone-kind-hand",
        ZoneKind::Discard => "cli-zone-kind-discard",
        ZoneKind::Trick => "cli-zone-kind-trick",
        ZoneKind::Tableau => "cli-zone-kind-tableau",
        ZoneKind::PlayerBoard => "cli-zone-kind-player-board",
        ZoneKind::ScoreSheet => "cli-zone-kind-score-sheet",
        ZoneKind::Board => "cli-zone-kind-board",
        ZoneKind::Other => "cli-zone-kind-other",
    };
    let visibility = match zone.visibility {
        Visibility::Public => "cli-visibility-public",
        Visibility::Owner => "cli-visibility-owner",
        Visibility::Hidden => "cli-visibility-hidden",
    };
    let layout = match zone.layout {
        Layout::Stack => tr!("cli-layout-stack"),
        Layout::Fan => tr!("cli-layout-fan"),
        Layout::Row => tr!("cli-layout-row"),
        Layout::Grid { columns } => tr!("cli-layout-grid", columns = columns),
        Layout::Free => tr!("cli-layout-free"),
    };
    tr!(
        "cli-zone",
        id = zone.id.0,
        label = tr!(&zone.label),
        owner = owner(zone.owner),
        kind = tr!(kind),
        visibility = tr!(visibility),
        layout = layout
    )
}

fn component(item: &Component) -> String {
    let kind = tr!(match item.kind {
        ComponentKind::Card => "cli-component-kind-card",
        ComponentKind::Tile => "cli-component-kind-tile",
        ComponentKind::Piece => "cli-component-kind-piece",
        ComponentKind::Token => "cli-component-kind-token",
        ComponentKind::Die => "cli-component-kind-die",
        ComponentKind::Marker => "cli-component-kind-marker",
    });
    let text = match &item.face {
        Face::Hidden => tr!("cli-component-hidden", id = item.id.0, kind = kind),
        Face::Shown { key, attributes } if attributes.is_empty() => {
            tr!(
                "cli-component-shown",
                id = item.id.0,
                kind = kind,
                key = key
            )
        }
        Face::Shown { key, attributes } => {
            let attributes: Vec<String> = attributes
                .iter()
                .map(|attribute| {
                    tr!(
                        "cli-attribute",
                        name = &attribute.name,
                        value = &attribute.value
                    )
                })
                .collect();
            tr!(
                "cli-component-shown-attributes",
                id = item.id.0,
                kind = kind,
                key = key,
                attributes = attributes.join(", ")
            )
        }
    };
    if item.rotation == 0 {
        text
    } else {
        tr!("cli-rotated", component = text, degrees = item.rotation)
    }
}
