# Speicherstand und große Welt

Design-Notiz, Stand Oktober 2026. Noch nicht umgesetzt.

## Welt

Tiled-World, Laden und Entladen der Karten, Flowfield und Gegner über Kartengrenzen: siehe [welt.md](../../welt.md).

## Zwei getrennte Systeme

Gespeichert wird in zwei voneinander unabhängigen Systemen:

- **Gruppen:** Zustand der Tiled-Objekte, ein Speicher-Chunk pro Gruppe (Gruppen-ID, siehe „Speichern 2“).
- **Item-Zellen:** abgelegte Items in einem festen Raster, unabhängig von den Karten.

Begründung: Tiled-Objekte haben eine Objekt- und eine Gruppen-ID, die das Tiled-Skript vergibt. Items gibt es in Tiled nicht, sie haben nur eine Position. Im Raster überleben sie das Teilen oder Neuzuschneiden von Karten, und alle Zellen sind gleich groß.

## Was gespeichert wird

- Nur Abweichungen vom Tiled-Stand. Fehlt eine Gruppe oder Zelle im Index, gilt der Zustand aus Tiled bzw. die Zelle ist leer.
- Schlüssel für Objekte: Objekt-ID, als Custom Property vom Tiled-Skript vergeben und weltweit eindeutig. Sie übersteht Umbenennen, Verschieben und Teilen von Karten. Weder Dateiname noch Tiled-ID noch Kartenindex der World verwenden.
- Schlüssel für Gruppen: Gruppen-ID.
- Schlüssel für Item-Zellen: Rasterkoordinaten `(x, y)` = Weltposition / Zellgröße.
- Liegengelassene Items gehören zu der Rasterzelle, in der sie liegen. Wechselt ein Item die Zelle, sind beide Zellen geändert.
- Globale Daten (Spieler, Inventar, Spielzeit, Flags, Item-ID-Zähler) stehen im Index.

### GroupState

Alles, was in einer Gruppe vom Tiled-Stand abweicht.

```rust
#[derive(Serialize, Deserialize, Clone, Default)]
struct GroupState {
    /// Objekt-ID → Abweichung
    objects: BTreeMap<ObjectId, ObjectState>,
}

#[derive(Serialize, Deserialize, Clone)]
enum ObjectState {
    Removed,
    Well { used: bool },
}
```

### CellState

Abgelegte Items in einer Rasterzelle.

```rust
#[derive(Serialize, Deserialize, Clone, Default)]
struct CellState {
    items: Vec<DroppedItem>,
}

#[derive(Serialize, Deserialize, Clone)]
struct DroppedItem {
    id: u64,           // global eindeutig, Zähler in den globalen Daten
    kind: ItemKind,
    count: u32,
    pos: Vec2,         // Weltkoordinaten
    dropped_at: f64,   // Spielzeit, falls Items verfallen sollen
}
```

- `BTreeMap` statt `HashMap`: Die Reihenfolge muss deterministisch sein, sonst ergibt gleicher Inhalt andere Bytes und einen anderen Hash. Die Deduplizierung wäre kaputt.
- Item-Positionen in Weltkoordinaten. Verschiebt man eine Karte in der World, bleiben die Items liegen, wo sie sind.
- `id` pro Item, um beim Aufheben den passenden Eintrag zu finden.

## Dateiformat

- Gleiche serde-Structs, zwei Formate:
    - Dev-Build (`dev`-Feature): RON, lesbar.
    - Release: MessagePack mit Feldnamen (`rmp_serde::to_vec_named`).
- Beide Builds können beide Formate lesen.
- Jede Datei beginnt mit einem Header: Magic, Format-Byte, Version. Die Dateiendung spielt keine Rolle.
- Neue Felder mit `#[serde(default)]`, damit alte Stände lesbar bleiben. Größere Änderungen über die Version migrieren.
- Kein bincode/postcard: ohne Feldnamen bricht jede Struct-Änderung alte Stände.
- Optional später: zstd-Kompression.

## Ablage auf der Platte

```
saves/
  lock
  groups/      GroupState-Dateien, Name = Content-Hash
  cells/       CellState-Dateien, Name = Content-Hash
  save_0042.idx
  suspend.idx
```

- Chunk-Dateien sind unveränderlich. Dateiname = Hash des Inhalts (z. B. blake3). Gleiche Chunks werden so automatisch dedupliziert, beim Laden lässt sich die Integrität prüfen.
- Gruppen und Zellen liegen in getrennten Ordnern (`groups/`, `cells/`).
- Ein Speicherstand ist **ein gemeinsamer Index** mit zwei Abschnitten: Gruppe → Hash und Zelle → Hash, dazu die globalen Daten. Gemeinsam, weil ein Speicherstand atomar sein muss: Hebt der Spieler ein Item auf, ändern sich Zelle und Inventar gleichzeitig. Getrennte Commits könnten nach einem Absturz ein Item verdoppeln oder verlieren.
- **Append-only:** Jedes Speichern erzeugt einen neuen Index (`save_0042.idx`). Bestehende Indizes werden nie überschrieben. Der neueste ist der aktuelle Stand.
- Ergebnis: volle Historie, jeder frühere Brunnen bleibt ladbar. Der Ordner wächst pro Speichern nur um die geänderten Chunks.

### Commit

1. Geänderte Gruppen- und Zell-Chunks als neue Dateien in `groups/` bzw. `cells/` schreiben, `fsync`. Existiert eine Datei mit dem Hash schon, entfällt das Schreiben.
2. Neuen Index als temporäre Datei schreiben, `fsync`.
3. Auf den endgültigen Namen umbenennen, Verzeichnis `fsync`.

Absturz vor Schritt 3: Es gibt keinen neuen Index, der vorherige gilt. Übrig bleiben nur verwaiste Chunk-Dateien.

### Verwaiste Dateien und GC

- Entstehen nur durch Absturz während eines Commits und durch gelöschte `suspend`-Indizes.
- Selten, klein und harmlos. Durch Content-Hashing werden sie wiederverwendet, wenn derselbe Inhalt erneut gespeichert wird.
- Vorerst kein GC.
- Nachrüsten, sobald Stände gelöscht werden können: Mark-and-Sweep über alle Indizes, `groups/` und `cells/` getrennt aufräumen. Nie parallel zu einem Commit (z. B. am Ende des Speicher-Tasks).

### Lock

- Beim Start Lock-Datei im Speicherordner öffnen und mit `File::try_lock()` exklusiv sperren. Offen lassen bis zum Ende.
- Schlägt das fehl: Meldung, dass das Spiel schon läuft, und beenden.
- Gilt für die ganze Sitzung, nicht nur fürs Schreiben: Lazy Loading liest während der ganzen Sitzung, und zwei Instanzen würden konkurrierende Stände anlegen.

## Laufzeit

- Zustand als zwei getrennte Maps plus globale Daten:
    - `HashMap<GroupId, Arc<GroupState>>` (Schlüssel: Gruppen-ID)
    - `HashMap<CellKey, Arc<CellState>>` (Schlüssel: Rasterkoordinaten)
- `Arc` statt `Rc`, weil Bevy-Ressourcen `Send + Sync` sein müssen.
- Ändern nur über `Arc::make_mut` (Copy-on-Write). Nur aufrufen, wenn wirklich etwas geändert wird.
- Zwei Stände im Speicher:
    - **aktuell**
    - **zuletzt gespeichert**, plus pro Gruppe und Zelle der Hash bzw. Dateiname (= der aktuelle Index im Speicher)
- Ein Snapshot ist ein Klon beider Maps. Chunk-Zustände werden geteilt. Bei sehr vielen Chunks persistente Maps (`im`, `rpds`) erwägen.
- **Geänderte Chunks:** `!Arc::ptr_eq(aktuell, gespeichert)`, für Gruppen und Zellen gleich. Keine Dirty-Flags nötig.
- Karte entladen: Entities weg, Zustand bleibt in der Map. Eine Karte kann Objekte aus mehreren Gruppen enthalten (verschobene Objekte behalten ihre Gruppe).
- Item-Zelle entladen: Eine Zelle kann zu mehreren Karten gehören. Ihre Entities erst entfernen, wenn keine dieser Karten mehr geladen ist (Zähler oder Prüfung pro Zelle).
- Item wechselt die Zelle (auch in eine nicht geladene): Zielzelle ergibt sich aus der Position. Deren Zustand in die Map holen, beide Zellen per `make_mut` ändern.

### Wahrheit ist der State

- `GroupState` und `CellState` sind die Wahrheit, Entities sind nur die Darstellung.
- Jede Änderung geht sofort per `make_mut` in den State (Item aufgehoben, Brunnen benutzt). Diese Änderungen kommen ohnehin als einzelne Events.
- Dadurch ist der State immer aktuell. Das passt zu `ptr_eq` und zum Speichern im Hintergrund, ohne Abgleich vor jedem Snapshot.
- Items, die durch Rückstoß rutschen: Position (und ggf. Zellwechsel) erst beim Liegenbleiben in den State schreiben.

## Laden

### Beim Start

1. Lock holen.
2. `suspend`-Index lesen, falls vorhanden, sonst den neuesten Index.
3. Globale Daten übernehmen, Spieler spawnen.
4. Chunks noch nicht lesen. Im Speicher steht nur der Index (Schlüssel → Hash, für Gruppen und Zellen).

### Beim Laden einer Karte

1. Tiled spawnt Karte und Objekte wie bisher.
2. Für jede Gruppe, zu der ein gespawntes Objekt gehört, `GroupState` holen:
    - steht im aktuellen Stand → nehmen
    - sonst Hash im Index → Datei aus `groups/` lesen, dekodieren und **dasselbe** `Arc` in aktuellen und gespeicherten Stand legen. Dann gilt `ptr_eq`, der Chunk zählt nicht als geändert.
    - sonst nichts eintragen, Tiled-Zustand gilt
3. Für jedes gespawnte Tiled-Objekt per Objekt-ID im State nachsehen: `Removed` → despawnen, sonst Zustand anwenden.
4. Alle Item-Zellen bestimmen, die die Fläche der Karte berühren. Für jede noch nicht geladene Zelle `CellState` genauso holen (aus `cells/`) und ihre Items spawnen.

Chunk-Dateien sind wenige KB groß, synchrones Lesen reicht anfangs. Später vorladen im `IoTaskPool`, wenn der Spieler sich nähert.

## Wann gespeichert wird

- **Am Brunnen:** Snapshot des aktuellen Stands an einen Task im `IoTaskPool`. Der schreibt die geänderten Gruppen und Zellen und einen neuen Index. Das Spiel läuft weiter. Danach wird der Snapshot zum gespeicherten Stand. Immer nur ein Speicher-Task gleichzeitig.
- **Beim Beenden:** Commit in einen separaten `suspend`-Index.
    - Beim Start: `suspend`-Index laden und sofort löschen. Gruppen und Zellen, die er anders referenziert als der neueste Index, gelten als geändert.
    - Beim nächsten Brunnen zeigt der neue Index auf die schon vorhandenen Dateien.
- **Tod:** Aktuellen Stand durch den gespeicherten ersetzen. Kein Lesen von der Platte für bekannte Chunks.
- **Absturz/kill:** letzter Brunnen gilt.
- Bevy: `WindowPlugin { close_when_requested: false, .. }`, `WindowCloseRequested` abfangen, speichern, dann `AppExit`. Gleiches für „Beenden“ im Menü.

## Tests

- Round-Trip eines Spielstands durch RON und MessagePack, Gleichheit prüfen.
- Commit mit simuliertem Abbruch vor dem Umbenennen: vorheriger Stand muss vollständig laden.
- Speichern im Hintergrund: Änderungen während des Commits landen nicht im geschriebenen Stand, bleiben aber im aktuellen.
- Unveränderte Gruppen und Zellen werden beim Commit nicht neu geschrieben.
- Item rutscht über eine Zellgrenze: Danach sind beide Zellen geändert, und das Item existiert genau einmal.
- Item aufheben: Zelle und Inventar landen im selben Commit.

## Offen

- Zellgröße des Item-Rasters.
- Verschwinden abgelegte Items nach einiger Zeit? Falls ja: Zeitstempel pro Item.
- Historie im Spiel anbieten (zu früherem Brunnen zurück)?
- Stände löschen können? Dann GC nötig.
- Export eines einzelnen Stands (Index + referenzierte Chunks in eine Datei).
