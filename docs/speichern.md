# Konzept: Speichern

Noch nicht umgesetzt. Baut auf der großen Welt auf ([welt.md](welt.md)).

## Ziel

- Gespeichert wird nur an festen Speicherpunkten, also an den Brunnen, und beim Beenden. Beim Tod
  wird der letzte Brunnen-Stand geladen.
- Gespeichert wird nur, was vom Tiled-Stand abweicht: offene Truhen, ausgelöste Hinterhalte,
  aktive Brunnen, getötete feste Gegner, Items am Boden, Spieler mit Inventar.
- Abgelegte Items bleiben für immer liegen, auch über viele Brunnen hinweg. Deshalb wächst der
  Speicherstand mit der Welt, und die Item-Zellen werden wie die Gruppen nur bei Bedarf gelesen.
- Frühere Brunnen-Stände bleiben ladbar (siehe „Historie“).
- Gegner werden nicht gespeichert. Sie verschwinden mit ihrer Karte (siehe [welt.md](welt.md)).
  Ausnahme sind feste Gegner, die in Tiled als Objekte stehen (Story, Spezialgegner): Von ihnen wird
  nur der Tod gespeichert, als `Removed` wie bei jedem anderen Objekt.
- Die Welt ist groß und wird nur stückweise geladen. Gespeichert und gelesen werden deshalb nur die
  Teile, die sich geändert haben bzw. gebraucht werden.

## IDs

Der Zustand eines Objekts braucht einen Schlüssel, der Umbenennen, Verschieben und Teilen von Karten
übersteht.

- **Objekt-ID:** Jedes Tiled-Objekt bekommt eine eigene, dauerhafte ID als Custom Property. Die
  Tiled-ID taugt nicht, weil Tiled sie beim Einfügen in eine andere Karte neu vergibt. Dateiname der
  Karte und Kartenindex der World taugen auch nicht, sie ändern sich beim Umbenennen bzw. beim
  Bearbeiten der `.world`-Datei.
- **Karten-ID:** Jede Karte bekommt ebenfalls eine eigene, dauerhafte ID als Custom Property
  der Karte. Sie übersteht Umbenennen und Verschieben der `.tmx`-Datei.
- **Gruppen-ID:** Zweite ID pro Objekt, die Karten-ID der Karte, in der das Objekt angelegt wurde.
  Sie ändert sich nie, auch nicht beim Verschieben in eine andere Karte. Die Gruppe bestimmt, in
  welchem Speicher-Chunk der Zustand liegt (siehe unten). So ist der Zustand immer direkt
  auffindbar, zusammengehörige Objekte liegen meist im selben Chunk, und beim Speichern wird wenig
  neu geschrieben.
- Verworfen: Raster für Objekte (der Zustand geht beim Verschieben verloren) und Hash-Bucketing
  über die Objekt-ID (keine räumliche Gruppierung).
- **Format:** 8 zufällige Bytes als 16 Hex-Ziffern (z. B. `3f9a0c27b1e4d856`), als String-Property.
  Im Spiel ein `u64` (`ObjectId`, `GroupId`). Keine UUID: Bindestriche, Version und Variante
  bringen nichts.
- **Eindeutigkeit:** Ein Objekt ist über Gruppen- und Objekt-ID zusammen bestimmt, also 16 Bytes,
  denn sein Zustand liegt in seiner Gruppe unter seiner Objekt-ID. Die Objekt-ID muss deshalb nur
  innerhalb ihrer Gruppe eindeutig sein, Karten-IDs dagegen weltweit. Bei 64 Bit sind Kollisionen
  praktisch ausgeschlossen, und der CI-Test fände sie.

### Tiled-Skript

- JavaScript im Extensions-Ordner von Tiled, hängt sich an `tiled.assetAboutToBeSaved`. Es setzt
  fehlende Karten-IDs, Objekt-IDs und Gruppen automatisch, damit nichts vergessen wird. Neue
  Objekte bekommen die Karten-ID der Karte als Gruppe.
- **Doppelte IDs in einer Karte** entstehen beim Duplizieren. Das Objekt mit der niedrigsten
  Tiled-ID behält die ID, die anderen bekommen eine neue, weil Tiled-IDs pro Karte hochzählen.
- **Doppelte IDs über Karten:** Ein in eine andere Karte kopiertes Objekt behält Gruppe und
  Objekt-ID. Das Skript sieht nur die aktuelle Karte und kann Verschieben nicht von Kopieren
  unterscheiden. Ein CI-Test findet solche Fälle, entschieden wird von Hand.
- **Doppelte Karten-IDs** entstehen, wenn eine `.tmx`-Datei kopiert wird. Auch sie findet der
  CI-Test.
- **CI-Test:** prüft alle Karten auf fehlende IDs, doppelte Karten-IDs und doppelte Objekt-IDs
  innerhalb einer Gruppe.
- Das Skript sollte vor dem ersten Speicherstand kommen, den jemand behalten will. Objekte ohne
  Objekt-ID lassen sich nicht speichern.
- Brunnen und Truhen müssen dafür Tile-Objekte werden. Der Brunnen ist bisher ein Tile mit
  `type="well"` in einer Tile-Ebene und kann so keine Objekt-ID tragen.

## Was gespeichert wird

Zwei voneinander unabhängige Arten von Speicher-Chunks:

- **Gruppen:** Zustand der Tiled-Objekte, ein Chunk pro Gruppe, Schlüssel ist die Gruppen-ID.
- **Item-Zellen:** abgelegte Items in einem festen Raster, unabhängig von den Karten. Schlüssel
  sind die Rasterkoordinaten `(x, y)` = Weltposition / Zellgröße. Items gibt es in Tiled nicht
  (später höchstens Spawner, die als Objekte in ihrer Gruppe stehen), sie haben nur eine Position.
  Im Raster überleben sie das Teilen oder Neuzuschneiden von Karten, und alle Zellen sind gleich
  groß.

Fehlt eine Gruppe oder Zelle im Index, gilt der Zustand aus Tiled bzw. die Zelle ist leer. Globale
Daten (Spieler, Inventar, Spielzeit, Flags, Item-ID-Zähler) stehen im Index.

```rust
/// Alles, was in einer Gruppe vom Tiled-Stand abweicht.
#[derive(Serialize, Deserialize, Clone, Default)]
struct GroupState {
    /// Objekt-ID → Abweichung
    objects: BTreeMap<ObjectId, ObjectState>,
}

#[derive(Serialize, Deserialize, Clone)]
enum ObjectState {
    Removed,
    Well { used: bool },
    /// Trigger-Zone eines Hinterhalts
    Triggered,
}

/// Abgelegte Items in einer Rasterzelle.
#[derive(Serialize, Deserialize, Clone, Default)]
struct CellState {
    items: Vec<DroppedItem>,
}

#[derive(Serialize, Deserialize, Clone)]
struct DroppedItem {
    id: u64,         // global eindeutig, Zähler in den globalen Daten
    kind: ItemId,
    count: u32,
    pos: Vec2,       // Weltkoordinaten
}
```

- `BTreeMap` statt `HashMap`: Die Reihenfolge muss deterministisch sein, sonst ergibt gleicher
  Inhalt andere Bytes und einen anderen Hash. Die Deduplizierung wäre kaputt.
- Item-Positionen in Weltkoordinaten. Verschiebt man eine Karte in der World, bleiben die Items
  liegen, wo sie sind.
- `id` pro Item, um beim Aufheben den passenden Eintrag zu finden. Entity-IDs ändern sich bei
  jedem Start.
- Ein Item gehört zu der Zelle, in der es liegt. Wechselt es die Zelle, sind beide Zellen geändert.

## State als Wahrheit

`GroupState` und `CellState` sind die Wahrheit, Entities sind nur die Darstellung.

- Jede Änderung geht sofort in den State. Dadurch ist er immer aktuell, das passt zu `ptr_eq` (siehe
  „Laufzeit“) und zum Speichern im Hintergrund, ohne Abgleich vor jedem Snapshot.
- Gameplay-Systeme ändern den State nie direkt, sie schicken nur Events. Ein zentrales System pro
  Objekttyp verarbeitet das Event: zuerst den State per `Arc::make_mut`, dann die Entity. Beide
  werden geändert, aber nur an einer Stelle im Code, so laufen sie nicht auseinander.
- Beispiel Item aufheben: `ItemPickedUp` kommt rein, Zelle per `make_mut` holen, Item über seine ID
  entfernen, ins Inventar legen, im selben System die Entity despawnen.
- Beispiel Brunnen: `used = true` im State, dann Sprite oder Komponente anpassen.

Verworfen: ECS als Wahrheit.

- Entladene Karten haben keine Entities. Ihr Zustand braucht trotzdem eine Map, also gäbe es zwei
  Wahrheiten.
- Vor jedem Snapshot müssten alle geladenen Entities in den State zurückgeschrieben werden. Das läuft
  im Hauptthread und wächst mit der Zahl der Objekte.
- `Arc::ptr_eq` reicht dann nicht. Geänderte Chunks müssten über `Changed` und `RemovedComponents`
  erkannt werden.
- Despawnte Objekte sind einfach weg. `Removed` müsste extra festgehalten werden.
- Items, die in eine nicht geladene Zelle rutschen, landen trotzdem im State.
- Einziger Vorteil: Systeme ändern nur Komponenten und können den State nicht vergessen. Bei uns
  kommen Änderungen ohnehin als einzelne Events, das Mitschreiben kostet fast nichts.

### Rutschende Items

Ausnahme vom Prinzip: Die Physik bewegt zuerst die Entity. Die Position kommt erst in den State,
wenn das Item liegen bleibt.

- Beim Ablegen oder Droppen schreibt das Event das Item sofort mit ID und Startposition in die
  passende Zelle. Erst danach rutscht es los.
- Speichern während es rutscht: Im State steht die Startposition. Nach dem Laden liegt es dort. Es
  ist nie weg und nie doppelt.
- Beim Liegenbleiben werden Position und ggf. Zellwechsel nachgetragen. Zellwechsel: beide Zellen
  per `make_mut` ändern.
- Optional, falls die alte Position stört: Direkt vor dem Snapshot alle wachen Items durchgehen und
  ihre aktuelle Position in den State schreiben. Läuft im selben Frame wie der Snapshot, bleibt also
  atomar. Nach dem Laden hat das Item keinen Schwung mehr.

Liegenbleiben erkennen mit Avian-Sleeping:

- Avian fügt die Marker-Komponente `Sleeping` hinzu. Darauf per Observer oder `Added<Sleeping>`
  reagieren: Position holen, Zielzelle berechnen, per `make_mut` in den State schreiben.
- Items dürfen kein `SleepingDisabled` haben.
- **Einschlafen:** Liegen lineare und Drehgeschwindigkeit unter `SleepingThreshold`, läuft ein
  Timer. Bleibt das länger als `DeactivationTime` so (etwa eine halbe Sekunde), bekommt der Körper
  `Sleeping`. Bewegt er sich vorher, startet der Timer neu.
- **Schlafend:** keine Integration und keine Kontaktauflösung mehr.
- **Aufwachen:** durch Berührung eines wachen Körpers, durch Setzen von Geschwindigkeit, Kraft oder
  Impuls oder durch einen Joint.
- **Inseln:** Sich berührende Körper bilden Simulationsinseln, die nur gemeinsam einschlafen und
  aufwachen. Ein Gegner, der durch einen Item-Haufen läuft, weckt alle auf. Beim erneuten
  Einschlafen werden die neuen Positionen geschrieben.
- `SleepingThreshold` wird mit `PhysicsLengthUnit` skaliert, die hier gesetzt ist
  (`movement::LENGTH_UNIT`).
- Körper haben keine Reibung. Items brauchen deshalb etwas `LinearDamping`, sonst rutschen sie lange
  und schlafen kaum ein.
- Alternative, falls Sleeping zu träge ist: selbst prüfen, wann `LinearVelocity` unter einen
  Schwellwert fällt.
- Unsicher: Avian hat Sleeping in den letzten Versionen umgebaut. Namen und Defaults für 0.8
  prüfen.

## Laufzeit

- Zustand als zwei getrennte Maps plus globale Daten:
  - `HashMap<GroupId, Arc<GroupState>>`
  - `HashMap<CellKey, Arc<CellState>>`
- `Arc` statt `Rc`, weil Bevy-Ressourcen `Send + Sync` sein müssen.
- Ändern nur über `Arc::make_mut` (Copy-on-Write), und nur, wenn wirklich etwas geändert wird.
- Zwei Stände im Speicher:
  - **aktuell**
  - **zuletzt gespeichert**, plus pro Gruppe und Zelle der Hash (= der aktuelle Index im Speicher)
- Ein Snapshot ist ein Klon beider Maps, die Chunks werden geteilt. Bei sehr vielen Chunks
  persistente Maps (`im`, `rpds`) erwägen.
- **Geänderte Chunks:** `!Arc::ptr_eq(aktuell, gespeichert)`, für Gruppen und Zellen gleich. Keine
  Dirty-Flags nötig.
- Item wechselt die Zelle (auch in eine nicht geladene): Die Zielzelle ergibt sich aus der Position.
  Deren Zustand in die Map holen, beide Zellen per `make_mut` ändern.

### Entladen

- Karte entladen: Entities weg, der State bleibt in der Map. Die Chunks sind klein, anfangs reicht
  das.
- Die Item-Entities einer Zelle erst entfernen, wenn keine geladene Karte die Zelle mehr berührt.
  Eine Zelle kann zu mehreren Karten gehören.

Falls aufgeräumt werden soll: **Nur unveränderte Chunks dürfen raus.**

- `Arc::ptr_eq(aktuell, gespeichert)`: Der Inhalt liegt schon auf der Platte. Aus **beiden** Ständen
  entfernen. Der Hash bleibt im Index, beim nächsten Laden wird die Datei neu gelesen.
- Kein `ptr_eq`: Der Chunk ist geändert und noch nicht gespeichert. Er bleibt bis zum nächsten
  Brunnen.
- Chunks ohne Indexeintrag und ohne Änderung stehen ohnehin nicht in der Map.

Zeitpunkt:

- **Gruppe:** erst, wenn keine geladene Karte mehr Objekte der Gruppe enthält. Eine Karte kann
  Objekte aus mehreren Gruppen enthalten, weil verschobene Objekte ihre Gruppe behalten.
- **Zelle:** erst, wenn keine geladene Karte die Zelle mehr berührt, wie für die Item-Entities.
- **Nach dem Speichern am Brunnen:** Alle nicht geladenen Chunks sind jetzt unverändert und können
  raus.
- Tod: Der aktuelle Stand wird durch den gespeicherten ersetzt. Entfernte Chunks fehlen in beiden.
  Das ist korrekt, sie werden bei Bedarf von der Platte nachgeladen.

Beides ergibt sich aus den geladenen Karten, ein eigener Zähler ist nicht nötig.

## Dateiformat

- Gleiche serde-Structs, zwei Formate:
  - **lesbar:** RON, im Dev-Build (`dev`-Feature),
  - **binär:** MessagePack mit Feldnamen (`rmp_serde::to_vec_named`), mit zstd komprimiert, im
    Release-Build.
- Beide Builds können beide Formate lesen.
- Jede Datei beginnt mit einem unkomprimierten Header: Magic, Format-Byte, Version. Die
  Dateiendung spielt keine Rolle.
- Neue Felder mit `#[serde(default)]`, damit alte Stände lesbar bleiben. Größere Änderungen über die
  Version migrieren.
- Kein bincode/postcard: Ohne Feldnamen bricht jede Struct-Änderung alte Stände.

## Ablage auf der Platte

```
saves/
  lock
  groups/      GroupState-Dateien, Name = Content-Hash
  cells/       CellState-Dateien, Name = Content-Hash
  save_0042.idx
  suspend.idx
```

- Chunk-Dateien sind unveränderlich. Dateiname = Hash des Inhalts (z. B. blake3). Gleiche Chunks
  werden so automatisch dedupliziert, beim Laden lässt sich die Integrität prüfen.
- Ein Speicherstand ist **ein gemeinsamer Index** mit zwei Abschnitten, Gruppe → Hash und
  Zelle → Hash, dazu die globalen Daten. Gemeinsam, weil ein Speicherstand atomar sein muss: Hebt
  der Spieler ein Item auf, ändern sich Zelle und Inventar gleichzeitig. Getrennte Commits könnten
  nach einem Absturz ein Item verdoppeln oder verlieren.
- **Append-only:** Jedes Speichern erzeugt einen neuen Index (`save_0042.idx`). Bestehende Indizes
  werden nie überschrieben. Der neueste ist der aktuelle Stand.
- Ergebnis: volle Historie, jeder frühere Brunnen bleibt ladbar. Der Ordner wächst pro Speichern nur
  um die geänderten Chunks.

### Commit

1. Geänderte Gruppen und Zellen als neue Dateien in `groups/` bzw. `cells/` schreiben, `fsync`.
   Existiert eine Datei mit dem Hash schon, entfällt das Schreiben.
2. Neuen Index als temporäre Datei schreiben, `fsync`.
3. Auf den endgültigen Namen umbenennen, Verzeichnis `fsync`.

Absturz vor Schritt 3: Es gibt keinen neuen Index, der vorherige gilt. Übrig bleiben nur verwaiste
Chunk-Dateien.

### Verwaiste Dateien und GC

- Entstehen nur durch einen Absturz während eines Commits und durch gelöschte `suspend`-Indizes.
- Selten, klein und harmlos. Durch Content-Hashing werden sie wiederverwendet, wenn derselbe Inhalt
  erneut gespeichert wird.
- Vorerst kein GC. Nachrüsten, sobald Stände gelöscht werden können: Mark-and-Sweep über alle
  Indizes, `groups/` und `cells/` getrennt aufräumen. Nie parallel zu einem Commit (z. B. am Ende
  des Speicher-Tasks).

### Lock

- Beim Start die Lock-Datei im Speicherordner öffnen und mit `File::try_lock()` exklusiv sperren.
  Offen lassen bis zum Ende.
- Schlägt das fehl: Meldung, dass das Spiel schon läuft, und beenden.
- Gilt für die ganze Sitzung, nicht nur fürs Schreiben: Lazy Loading liest während der ganzen
  Sitzung, und zwei Instanzen würden konkurrierende Stände anlegen.

## Laden

### Beim Start

1. Lock holen.
2. `suspend`-Index lesen, falls vorhanden, sonst den neuesten Index.
3. Globale Daten übernehmen, Spieler spawnen.
4. Chunks noch nicht lesen. Im Speicher steht nur der Index.

### Beim Laden einer Karte

Beim Entladen verschwinden alle Entities der Karte, beim erneuten Laden spawnt Tiled die Objekte
frisch aus der `.tmx`. Der gespeicherte Zustand wird danach angewendet, z. B. per Observer auf
`MapCreated`:

1. Tiled spawnt Karte und Objekte wie bisher.
2. Für jede Gruppe, zu der ein gespawntes Objekt gehört, `GroupState` holen:
   - steht im aktuellen Stand → nehmen,
   - sonst Hash im Index → Datei aus `groups/` lesen, dekodieren und **dasselbe** `Arc` in den
     aktuellen und den gespeicherten Stand legen. Dann gilt `ptr_eq`, der Chunk zählt nicht als
     geändert,
   - sonst nichts eintragen, der Tiled-Zustand gilt.
3. Für jedes gespawnte Tiled-Objekt per Objekt-ID im State nachsehen: `Removed` → despawnen, sonst
   Zustand anwenden.
4. Alle Item-Zellen bestimmen, die die Fläche der Karte berühren. Für jede noch nicht geladene
   Zelle `CellState` genauso holen (aus `cells/`) und ihre Items spawnen.

Chunk-Dateien sind wenige KB groß, synchrones Lesen reicht anfangs. Später im `IoTaskPool`
vorladen, wenn der Spieler sich nähert.

## Wann gespeichert wird

- **Am Brunnen:** Snapshot des aktuellen Stands an einen Task im `IoTaskPool`. Der schreibt die
  geänderten Gruppen und Zellen und einen neuen Index, das Spiel läuft weiter. Danach wird der
  Snapshot zum gespeicherten Stand. Immer nur ein Speicher-Task gleichzeitig.
- **Beim Beenden:** Commit in einen separaten `suspend`-Index.
  - Beim Start: `suspend`-Index laden und sofort löschen. Gruppen und Zellen, die er anders
    referenziert als der neueste Index, gelten als geändert.
  - Beim nächsten Brunnen zeigt der neue Index auf die schon vorhandenen Dateien.
- **Tod:** Den aktuellen Stand durch den gespeicherten ersetzen. Bekannte Chunks werden nicht von
  der Platte gelesen. Bisher heilt `combat` den Spieler und entfernt alle Gegner; vorher den
  TODO-Punkt umsetzen, dass `combat` ein `Died`-Event auslöst.
- **Absturz/kill:** Der letzte Brunnen gilt.
- Bevy: `WindowPlugin { close_when_requested: false, .. }`, `WindowCloseRequested` abfangen,
  speichern, dann `AppExit`. Gleiches für „Beenden“ im Menü.

## Historie

Gehört zur ersten Version. Weil Indizes und Chunks unveränderlich sind und nie überschrieben werden,
ist jeder frühere Brunnen-Stand noch vollständig auf der Platte.

- Jeder Index enthält dazu den Brunnen (seine Objekt-ID), Datum und Spielzeit. Die Liste im Menü
  liest nur die Indizes, keine Chunks.
- Einen älteren Stand laden:
  1. Index lesen, aktuellen und gespeicherten Stand leeren, Index und globale Daten übernehmen.
  2. Gespawnte Items despawnen und die World neu spawnen (`RespawnTiledWorld`). Die Karten laden
     ihre Gruppen und Zellen dann wie beim Start nach.
- Ungespeicherte Änderungen gehen dabei verloren wie beim Tod, das Menü fragt deshalb nach.
- Neuere Stände bleiben erhalten. Das nächste Speichern hängt einen neuen Index an, die Liste bleibt
  nach Zeit geordnet. Optional vermerkt jeder Index seinen Vorgänger, dann lassen sich Zweige
  erkennen.

## Tests

- Round-Trip eines Spielstands durch RON und MessagePack mit zstd, Gleichheit prüfen.
- Commit mit simuliertem Abbruch vor dem Umbenennen: Der vorherige Stand muss vollständig laden.
- Speichern im Hintergrund: Änderungen während des Commits landen nicht im geschriebenen Stand,
  bleiben aber im aktuellen.
- Unveränderte Gruppen und Zellen werden beim Commit nicht neu geschrieben.
- Item aufheben: Zelle und Inventar landen im selben Commit.
- Item rutscht über eine Zellgrenze: Danach sind beide Zellen geändert, und das Item existiert
  genau einmal.
- Speichern, während ein Item rutscht: Nach dem Laden existiert es genau einmal, an der Start- bzw.
  zuletzt geschriebenen Position.
- Gegner weckt schlafende Items: Die neuen Positionen landen nach dem Einschlafen im State.
- Unveränderte Gruppe entladen und wieder laden: Zustand stimmt, Datei wird neu gelesen.
- Geänderte Gruppe entladen: Der State bleibt im Speicher und landet im nächsten Commit.
- Älteren Stand laden: Gruppen, Zellen und globale Daten entsprechen ihm, neuere Indizes bleiben
  unverändert, und das nächste Speichern legt einen neuen Index an.

## Offen

- **Zellgröße des Item-Rasters:** Startwert 256 px (16 Tiles) als Konstante, später
  ausprobieren. Kleinere Zellen schreiben pro Änderung weniger, ergeben aber mehr Dateien und mehr
  Lesezugriffe beim Laden einer Karte.
- **Stände löschen:** später. Dann ist GC nötig.
- **Export** eines einzelnen Stands (Index und referenzierte Chunks in eine Datei): vielleicht
  später.
