# Speichern: Chunking und Entladen

Design-Notiz, Stand Oktober 2026. Ergänzt „Speicherstand und große Welt“. Noch nicht umgesetzt.

## TiledWorldChunking

- Komponente an der Entity mit `TiledWorld`. Ohne sie lädt `bevy_ecs_tiled` alle Karten der World auf einmal.
- Enthält einen Abstand als `Vec2` (x und y getrennt), z. B. `TiledWorldChunking::new(800., 600.)`.
- Jeden Frame wird geprüft, welche Karten innerhalb dieses Abstands um eine **Kamera** liegen. Diese werden gespawnt, alle anderen despawnt.
- Gemessen wird gegen die Kamera, nicht gegen den Spieler. Hier egal, weil die Kamera dem Spieler folgt.
- Karten werden immer komplett geladen oder entladen.
- Beim Entladen verschwinden alle Entities der Karte samt Komponenten. Beim erneuten Laden spawnt Tiled die Objekte frisch aus der `.tmx`. Den gespeicherten Zustand danach selbst anwenden, z. B. per Observer auf das Map-Created-Event.
- Abstand größer als Sicht- und Aggro-Reichweite.
- Unsicher: Namen und Signatur für 0.14 in der Doku prüfen.

## Gegner entladen

Gegner gehören nicht zur Karte und werden nicht automatisch entladen.

Gewählt: eigenes System mit Positionsprüfung.

- Gegner bleiben ohne Eltern.
- Ein System prüft regelmäßig (z. B. einmal pro Sekunde), ob jeder Gegner noch auf einer geladenen Karte steht. Dafür reichen die Rechtecke der geladenen Karten.
- Steht er auf keiner, wird er despawnt.
- Etwas Puffer am Rand, sonst flackern Gegner an der Grenze.
- Alternative: nur beim Entladen einer Karte die Gegner in ihrem Rechteck prüfen.
- Gegner werden nicht gespeichert, Despawnen reicht.
  Verworfen: Gegner als Kinder der Karte. Läuft ein Gegner in eine andere Karte, gehört er noch zur alten und verschwindet mit ihr, außer man hängt ihn bei jedem Kartenwechsel um.

## MapState und CellState entladen

Grundsätzlich bleibt der State beim Entladen in der Map, nur die Entities gehen. Die Chunks sind klein, anfangs reicht das.

Falls aufgeräumt werden soll: **Nur unveränderte Chunks dürfen raus.**

- `Arc::ptr_eq(aktuell, gespeichert)`: Inhalt liegt schon auf der Platte. Aus **beiden** Ständen entfernen. Der Hash bleibt im Index, beim nächsten Laden wird die Datei neu gelesen.
- Kein `ptr_eq`: Chunk ist geändert und noch nicht gespeichert. Bleibt bis zum nächsten Brunnen.
- Chunks ohne Indexeintrag und ohne Änderung stehen ohnehin nicht in der Map.
  Zeitpunkt:

- **Karte:** beim Entladen der Karte prüfen.
- **Zelle:** erst, wenn keine geladene Karte die Zelle mehr berührt. Gleicher Zähler wie für die Item-Entities.
- **Nach dem Speichern am Brunnen:** Alle nicht geladenen Chunks sind jetzt unverändert und können raus.
  Tod: Der aktuelle Stand wird durch den gespeicherten ersetzt. Entfernte Chunks fehlen in beiden. Das ist korrekt, sie werden bei Bedarf von der Platte nachgeladen.

## Tests (zusätzlich)

- Gegner verlässt alle geladenen Karten: wird despawnt.
- Gegner wechselt zwischen zwei geladenen Karten: bleibt erhalten.
- Unveränderte Karte entladen und wieder laden: Zustand stimmt, Datei wird neu gelesen.
- Geänderte Karte entladen: State bleibt im Speicher und landet im nächsten Commit.
