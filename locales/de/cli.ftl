# Das Kommandozeilenprogramm `table` (crates/cli).

## Hilfe

cli-usage =
    Aufruf:
    table [--lang <sprache>] games
    {"    "}Listet die Spiele.
    table [--lang <sprache>] play <spiel> [--seats human,bot,...] [--seed <n>] [--variant <json>] [--record <datei>]
    {"    "}Spielt eine Partie im Terminal. Jeder Platz ist human (Mensch) oder bot (Standard: human,bot).
    table [--lang <sprache>] replay <partie.json> [--seat <n>] [--each]
    {"    "}Spielt eine aufgezeichnete Partie nach und zeigt den Tisch eines Platzes (Standard: Zuschauer).
    table [--lang <sprache>] simulate <spiel> [-n <partien>] [--seats <anzahl>] [--seed <n>] [--variant <json>]
    {"    "}Lässt Zufallsbots gegeneinander spielen und fasst die Ergebnisse zusammen.
    table help
    {"    "}Zeigt diese Hilfe.
    Ohne --lang kommt die Sprache aus LC_ALL, LC_MESSAGES oder LANG, sonst Englisch.
cli-games-header = Spiele:
cli-games-entry = { $id }: { $name }

## Fehler

cli-error = Fehler: { $message }
cli-error-unknown-command = Unbekannter Befehl „{ $command }“.
cli-error-missing-argument = Es fehlt: { $name }.
cli-error-unknown-option = Unbekannte Option „{ $option }“.
cli-error-option-value = Die Option { $option } braucht einen Wert.
cli-error-bad-number = { $option }: „{ $value }“ ist keine gültige Zahl.
cli-error-unknown-game = Unbekanntes Spiel „{ $game }“. „table games“ zeigt die Liste.
cli-error-seat-kind = Unbekannte Platzart „{ $kind }“; erlaubt sind human und bot.
cli-error-seat-range = Platz { $seat } gehört nicht zu dieser Partie.
cli-error-seat-count = Eine Partie braucht 1 bis 64 Plätze, nicht { $count }.
cli-error-variant-json = Die Variante passt nicht zu diesem Spiel: { $reason }
cli-error-invalid-variant = Ungültige Variante: { $reason }
cli-error-read = { $path } kann nicht gelesen werden: { $reason }
cli-error-write = { $path } kann nicht geschrieben werden: { $reason }
cli-error-io = Ein-/Ausgabefehler: { $reason }
cli-error-record-json = { $path } ist keine gültige Partieaufzeichnung: { $reason }
cli-error-replay-schema = Die Aufzeichnung hat Schemaversion { $found }; dieses Programm liest Version { $expected }.
cli-error-replay-wrong-game = Die Aufzeichnung gehört zu einem anderen Spiel.
cli-error-replay-not-pending = Zug { $number } gehört zu einem Platz, der nicht am Zug war.
cli-error-replay-illegal = Zug { $number } ist nicht erlaubt: { $reason }
cli-error-not-pending = Platz { $seat } ist jetzt nicht am Zug.
cli-error-not-selectable = Auswahl { $number } ist nicht wählbar.
cli-error-incomplete = Die Auswahl ergibt noch keinen vollständigen Zug.
cli-error-ambiguous = Die Auswahlpfade des Spiels sind mehrdeutig.
cli-error-illegal = Unerlaubter Zug: { $reason }
cli-error-too-long = Partie { $number } war nach { $moves } Zügen nicht beendet.

## Spielen

cli-play-header =
    { $game } mit { $seats ->
        [one] einem Platz
       *[other] { $seats } Plätzen
    }, Startwert { $seed }.
cli-seat-human = Platz { $seat }: Mensch
cli-seat-bot = Platz { $seat }: Bot
cli-prompt = Platz { $seat }, wähle:
cli-prompt-item = { $number }) { $choice }
cli-prompt-made = Bisher gewählt: { $choices }
cli-prompt-input = Gib eine Zahl ein, u zum Zurücknehmen, q zum Beenden:
cli-input-invalid = Bitte gib eine Zahl von 1 bis { $max }, u oder q ein.
cli-handover = Gib das Gerät an Platz { $seat } weiter und drücke Enter.
cli-bot-acted = Platz { $seat } (Bot) hat entschieden.
cli-quit = Partie abgebrochen.
cli-record-written = Aufzeichnung in { $path } gespeichert.

## Textdarstellung

cli-table-header = --- Tisch: { $observer } ---
cli-observer-seat = Platz { $seat }
cli-observer-spectator = Zuschauer
cli-indicator = { $label }: { $value }
cli-indicator-owned = { $label } ({ $owner }): { $value }
cli-zone = [{ $id }] { $label }: { $owner }, { $kind }, { $visibility }, { $layout }
cli-zone-empty = (leer)
cli-owner-seat = Platz { $seat }
cli-owner-shared = gemeinsam
cli-zone-kind-draw-pile = Nachziehstapel
cli-zone-kind-hand = Hand
cli-zone-kind-discard = Ablagestapel
cli-zone-kind-trick = Stich
cli-zone-kind-tableau = gemeinsame Auslage
cli-zone-kind-player-board = eigene Auslage
cli-zone-kind-score-sheet = Punkteblock
cli-zone-kind-board = Spielbrett
cli-zone-kind-other = Sonstiges
cli-visibility-public = offen
cli-visibility-owner = nur für den Besitzer
cli-visibility-hidden = verdeckt
cli-layout-stack = Stapel
cli-layout-fan = Fächer
cli-layout-row = Reihe
cli-layout-grid =
    Raster mit { $columns ->
        [one] einer Spalte
       *[other] { $columns } Spalten
    }
cli-layout-free = frei
cli-component-kind-card = Karte
cli-component-kind-tile = Plättchen
cli-component-kind-piece = Figur
cli-component-kind-token = Marker
cli-component-kind-die = Würfel
cli-component-kind-marker = Zählstein
cli-component-hidden = #{ $id } { $kind }: verdeckt
cli-component-shown = #{ $id } { $kind }: { $name }
cli-component-shown-attributes = #{ $id } { $kind }: { $name } ({ $attributes })
cli-attribute = { $name }: { $value }
cli-rotated = { $component }, um { $degrees } Grad gedreht
cli-choice-component = { $component } in { $zone }
cli-choice-zone = [{ $id }] { $zone }
cli-choice-subset = { $components }
cli-choice-none = (keine Auswahl nötig)

## Ergebnisse

cli-outcome-header = Ergebnis:
cli-outcome-seat = Platz { $seat }: Rang { $rank }, Punkte { $score }
cli-outcome-team = Team: { $seats }
cli-unfinished = Die Partie ist nicht beendet.

## Nachspielen und Simulieren

cli-replay-header =
    Nachspielen: { $game } mit { $seats ->
        [one] einem Platz
       *[other] { $seats } Plätzen
    }, { $moves ->
        [one] ein Zug
       *[other] { $moves } Züge
    }.
cli-replay-move = Zug { $number }: Platz { $seat }: { $choices }
cli-simulate-summary =
    { $matches ->
        [one] Eine Partie
       *[other] { $matches } Partien
    } { $game } mit { $seats ->
        [one] einem Platz
       *[other] { $seats } Plätzen
    }, insgesamt { $moves ->
        [one] ein Zug
       *[other] { $moves } Züge
    }.
cli-simulate-seat =
    Platz { $seat }: { $wins ->
        [one] ein Sieg
       *[other] { $wins } Siege
    }, durchschnittlich { $score } Punkte
