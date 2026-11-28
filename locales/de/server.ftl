# The server `table-server` (crates/server) and its dev multiclient page
# (apps/web/dev). German translation of locales/en/server.ftl.

## Console

server-usage =
    Aufruf:
    table-server [--listen <Adresse>]
    {"    "}Startet den Dev-Server (Standardadresse: 127.0.0.1:8080).
    Die Sprache kommt aus LC_ALL, LC_MESSAGES oder LANG, sonst Englisch.
server-listening = Dev-Server lauscht auf { $url }
server-dev-build = Dev-Build: Allwissende Ansicht, Spielprotokolle und Ereignisse werden ausgeliefert.
server-bind-failed = Kann nicht auf { $address } lauschen: { $reason }
server-bad-argument = Unbekannte Argumente: { $arguments }

## Errors sent to clients

server-error-protocol-version = Dieser Server spricht Protokollversion { $expected }, nicht { $version }.
server-error-bad-message = Fehlerhafte Nachricht: { $detail }
server-error-unknown-game = Unbekanntes Spiel „{ $game }“.
server-error-bad-seats = Eine Partie braucht 1 bis 64 Plätze, nicht { $seats }.
server-error-drivers = Es gibt { $seats } Plätze, aber { $drivers } Angaben, wer sie spielt.
server-error-variant-json = Die Variante passt nicht zu diesem Spiel: { $detail }
server-error-invalid-variant = Ungültige Variante: { $reason }
server-error-unknown-match = Es gibt keine Partie { $match }.
server-error-no-match = Starte oder betritt zuerst eine Partie.
server-error-bad-seat = Platz { $seat } gehört nicht zu dieser Partie.
server-error-seat-is-bot = Platz { $seat } wird von einem Bot gespielt.
server-error-no-ismcts = Dieses Spiel hat keinen ISMCTS-Bot.
server-error-not-pending = Platz { $seat } ist gerade nicht am Zug.
server-error-not-selectable = Diese Auswahl ist nicht möglich.
server-error-incomplete = Die Auswahl ergibt noch keinen vollständigen Zug.
server-error-ambiguous = Die Auswahlpfade des Spiels sind mehrdeutig.
server-error-illegal = Unerlaubter Zug: { $reason }
server-error-bad-position = Zug { $position } gibt es nicht; die Partie hat { $length } Züge.
server-error-omniscient-unavailable = Die allwissende Ansicht gibt es nur in Dev-Builds.
server-error-internal = Interner Fehler: { $detail }

## Web page

server-page-title = Dev-Multiclient
server-page-connecting = Verbinde …
server-page-connected = Verbunden
server-page-disconnected = Getrennt. Lade die Seite neu, um dich wieder zu verbinden.
server-page-dev-build = Dev-Build
server-page-language = Sprache
server-page-game = Spiel
server-page-seats = Plätze
server-page-seed = Startwert
server-page-seed-random = zufällig
server-page-variant = Variante (JSON)
server-page-create = Neue Partie
server-page-match-id = Partienummer
server-page-join = Beitreten
server-page-match = Partie { $match }: { $game }
server-page-seat = Platz { $seat }
server-page-spectator = Zuschauer
server-page-omniscient = Allwissend
server-page-driver = Gespielt von
server-page-driver-human = Mensch
server-page-driver-bot = Bot
server-page-driver-ismcts = ISMCTS-Bot
server-page-to-act = Am Zug
server-page-waiting = Wartet
server-page-chosen = Gewählt:
server-page-nothing-chosen = noch nichts
server-page-choose = Wähle:
server-page-complete = Vollständig; abschicken oder zurücknehmen.
server-page-undo = Zurücknehmen
server-page-submit = Abschicken
server-page-owner = Platz { $seat }
server-page-empty = (leer)
server-page-face-down = verdeckt
server-page-log = Zugliste
server-page-log-start = Anfang
server-page-log-entry = { $number }. Platz { $seat }: { $choices }
server-page-log-hidden = verdeckt
server-page-first = Anfang
server-page-previous = Zurück
server-page-next = Vor
server-page-last = Ende
server-page-live = Live
server-page-position = Nach Zug { $position } von { $length }
server-page-history = Du siehst eine frühere Stellung; Eingaben sind aus, bis du zu Live zurückkehrst.
server-page-outcome = Ergebnis
server-page-outcome-seat = Platz { $seat }: Rang { $rank }, Punkte { $score }
server-page-record = Spielprotokoll
server-page-download-record = Protokoll herunterladen
server-page-error = Fehler: { $message }
