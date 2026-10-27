# Speichern: Chunking und Entladen

Design-Notiz, Stand Oktober 2026. Ergänzt „Speicherstand und große Welt“. Noch nicht umgesetzt.

## Karten und Gegner entladen

Laden und Entladen der Karten und das Entladen der Gegner stehen in [welt.md](../../welt.md). Für das Speichern wichtig:

- Beim Entladen verschwinden alle Entities der Karte samt Komponenten. Beim erneuten Laden spawnt Tiled die Objekte frisch aus der `.tmx`. Den gespeicherten Zustand danach selbst anwenden, z. B. per Observer auf das Map-Created-Event.
- Gegner werden nicht gespeichert, Despawnen reicht.

## GroupState und CellState entladen

Grundsätzlich bleibt der State beim Entladen in der Map, nur die Entities gehen. Die Chunks sind klein, anfangs reicht das.

Falls aufgeräumt werden soll: **Nur unveränderte Chunks dürfen raus.**

- `Arc::ptr_eq(aktuell, gespeichert)`: Inhalt liegt schon auf der Platte. Aus **beiden** Ständen entfernen. Der Hash bleibt im Index, beim nächsten Laden wird die Datei neu gelesen.
- Kein `ptr_eq`: Chunk ist geändert und noch nicht gespeichert. Bleibt bis zum nächsten Brunnen.
- Chunks ohne Indexeintrag und ohne Änderung stehen ohnehin nicht in der Map.
  Zeitpunkt:

- **Gruppe:** erst, wenn keine geladene Karte mehr Objekte der Gruppe enthält. Ergibt sich aus den geladenen Karten, wie bei den Zellen.
- **Zelle:** erst, wenn keine geladene Karte die Zelle mehr berührt, wie für die Item-Entities.
- **Nach dem Speichern am Brunnen:** Alle nicht geladenen Chunks sind jetzt unverändert und können raus.
  Tod: Der aktuelle Stand wird durch den gespeicherten ersetzt. Entfernte Chunks fehlen in beiden. Das ist korrekt, sie werden bei Bedarf von der Platte nachgeladen.

## Tests (zusätzlich)

- Unveränderte Gruppe entladen und wieder laden: Zustand stimmt, Datei wird neu gelesen.
- Geänderte Gruppe entladen: State bleibt im Speicher und landet im nächsten Commit.
