# Konzept: große Welt

## Ziel

- Die Welt besteht aus vielen Tiled-Karten in einer `.world`-Datei. Geladen sind nur die Karten um
  den Spieler. Das ist Voraussetzung für das Spiel, deshalb von Anfang an.
- Flowfield, Sicht und Gegner funktionieren über Kartengrenzen hinweg.
- Diese Version speichert nichts: Eine entladene Karte kommt beim nächsten Laden im Tiled-Stand
  zurück. Das Speichern baut darauf auf ([speichern.md](speichern.md)).

## Karten

- Jede Karte ist eine Lade-Einheit und wird immer ganz geladen oder entladen. Karten können
  unterschiedlich groß sein, sollten aber ungefähr gleich groß und eher mittelgroß sein (etwa ein
  bis zwei Bildschirme).
- Größe und Position in der `.world` in ganzen Tiles (16 px). Dann liegen alle Karten auf dem
  4-px-Raster von Flowfield und Sicht.
- `startup` spawnt statt der `TiledMap` eine `TiledWorld` im Ursprung. Die Einstellungen, die jetzt
  an der Karte hängen (`TilemapRenderSettings`, `TiledMapLayerZOffset`, `TiledPhysicsSettings`),
  kommen an die World. bevy_ecs_tiled übernimmt sie für jede Karte.
- Der Observer für `ColliderCreated` kommt ebenfalls an die World. `TiledEvent`s wandern über
  `ChildOf` nach oben, die Karten sind Kinder der World.
- Karten bekommen immer den Anker `TopLeft`, ihre Position ist die aus der `.world` (y in Bevy
  negativ). Dafür hat auch die World den Anker `TopLeft`, und keine Karte darf ein negatives x
  haben, sonst verschiebt bevy_ecs_tiled alle Karten. Spieler und Debug-Gegner starten an
  `player::START` auf `map.tmx`. Später kommt der Startpunkt aus einem Tiled-Objekt.
- Die World lädt alle `.tmx` als Assets beim Start. Nur die Entities entstehen erst, wenn eine
  Karte in den Ladebereich kommt.

## Laden und Entladen

- `TiledWorldChunking::new(x, y)` an der World gibt die **halbe** Breite und Höhe eines Rechtecks
  um die Kamera an. Karten, deren Rechteck es schneidet, werden gespawnt, alle anderen samt Kindern
  (Ebenen, Objekte, Kollider) despawnt.
- bevy_ecs_tiled prüft das in jedem Frame, das kostet nur einen Rechteck-Test pro Karte. Gemessen
  wird an der Kamera, die dem Spieler folgt.
- Der Ladebereich muss abdecken:
  - den sichtbaren Ausschnitt, also die halbe Fenstergröße mal Zoom (`area` der Projektion),
  - das Flowfield-Fenster bis zum Rand seiner äußersten Zelle plus größte Clearance
    (`flowfield::REACH`, derzeit 340 px), sonst fehlen am Rand des Fensters Hindernisse,
  - das Fenster der Sicht (`sight::REACH`, kleiner),
  - später den Deaktivierungs-Radius aus [ideen.md](ideen/ideen.md).
- `world::update_chunking` setzt `TiledWorldChunking` auf das Maximum aus diesen Werten, wenn es
  sich ändert, also mit Fenstergröße und Zoom.

## Hindernisse pro Karte

Bisher deckt das Flowfield genau eine Karte ab: `create_grids` ersetzt bei jedem `MapCreated` alle
Raster, und jede Änderung eines Kolliders rastert alle statischen Kollider neu. Mit mehreren Karten
gewänne die zuletzt geladene.

Das Flowfield hat zwei Teile, die sich unterschiedlich verhalten:

- **Belegung**: hängt nur an den statischen Kollidern einer Karte.
- **Distanzen**: das Fenster um den Spieler, wandert mit ihm.

Deshalb:

- Jede geladene Karte bekommt ihr Rechteck als Komponente (`world::MapArea`). Die Belegung jeder
  Karte (`flowfield::Obstacles`) steckt ebenfalls als Komponente an der Karten-Entity und
  verschwindet mit ihr. Sie speichert pro Zelle den Abstand ihres Mittelpunkts zum nächsten
  Hindernis, so gilt sie für alle Clearances: blockiert ist eine Zelle, die näher als die
  Clearance liegt, `solid` eine, deren Mittelpunkt im Hindernis liegt.
- Ändert sich ein Kollider, wird nur die Belegung seiner Karte neu gerastert. Die Karte eines
  Kolliders ergibt sich über seine Eltern: der nächste Vorfahre mit `MapArea`. Kollider ohne Karte
  zählen nicht. Alle Kollider einer Karte sind statisch, gefiltert werden nur Sensoren.
  `world::rasterize` erledigt das für Flowfield und Sicht gleich (`world::MapRaster`).
- Die Belegung deckt die Karte plus einen Rand von der größten Clearance (16 px, 4 Zellen) ab. Ein
  Baum am Rand von Karte A blockiert so auch die Zellen von Karte B, die ihm zu nah sind.
- Das Fenster bleibt in der Ressource `FlowField`. Seine Belegung wird aus den Komponenten der
  geladenen Karten zusammengesetzt, wenn sich das Fenster verschiebt oder eine Karte geladen,
  entladen oder neu gerastert wird. Eine Zelle ist blockiert, wenn eine der Karten sie blockiert,
  deren Rechteck mit Rand die Zelle enthält (höchstens vier). Zellen außerhalb aller geladenen
  Karten sind blockiert.
- Die Suche selbst bleibt, wie sie ist. Sie liest nur noch die Belegung des Fensters statt die der
  ganzen Karte. Zusammengesetzt wird bei jeder Suche, denn gesucht wird genau dann, wenn sich das
  Fenster verschiebt oder sich Karten ändern (dann vergessen alle Raster ihr Ziel).
- Das Debug-Overlay zeigt die Belegung des Fensters statt der ganzen Karte.

Die Sicht geht genauso vor: Die undurchsichtigen Zellen jeder Karte stecken an der Karte, ohne
Rand, weil die Sicht keine Clearance hat. Zellen außerhalb geladener Karten sind durchsichtig.

Alle Raster liegen auf einem gemeinsamen Zellraster mit Ursprung im Weltursprung (`cells`).
`MapArea` rechnet mit der `Transform` der Karte. Die ist relativ zur World, das passt, solange die
World im Ursprung bleibt.

## Gegner

Gegner gehören nicht zur Karte und werden nicht automatisch entladen. Bleiben sie stehen, laufen
sie durch die fehlenden Kollider.

- Gegner bleiben ohne Eltern.
- `enemy::despawn_outside_maps` prüft einmal pro Sekunde, ob jeder Gegner noch auf einer geladenen
  Karte steht. Dafür reichen die Rechtecke der geladenen Karten, mit etwas Puffer am Rand
  (`UNLOAD_MARGIN`, 16 px), sonst flackern Gegner an der Grenze.
- Steht er auf keiner, wird er despawnt. Gegner werden nicht gespeichert.
- Feste Gegner aus Tiled-Objekten (gibt es noch nicht, siehe [spawnen.md](spawnen.md)) entstehen
  beim Laden ihrer Karte neu. Läuft einer in eine andere
  geladene Karte und wird seine eigene neu geladen, darf er nicht doppelt entstehen: vor dem
  Spawnen per Gruppen- und Objekt-ID prüfen, ob er noch lebt.
- Alternative: nur beim Entladen einer Karte die Gegner in ihrem Rechteck prüfen.
- Verworfen: Gegner als Kinder der Karte. Läuft ein Gegner in eine andere Karte, gehört er noch zur
  alten und verschwindet mit ihr, außer man hängt ihn bei jedem Kartenwechsel um.

## Items

- Items am Boden bleiben vorerst bestehen, wenn ihre Karte entladen wird. Sie haben keine Physik
  und brauchen die Kollider nicht.
- Mit dem Speichern kommen sie in Zellen eines festen Rasters und werden mit entladen.

## Code

- `main.rs`: `TiledWorld` statt `TiledMap`, Einstellungen und Observer an der World.
- `world`: `MapArea`, `rasterize` für die Raster pro Karte, `update_chunking`.
- `cells`: das gemeinsame Zellraster (`CellRect`, `Raster`) von Flowfield und Sicht.
- `assets/`: `world.world` mit `map.tmx` und der Testkarte `map_east.tmx` östlich davon, erreichbar
  über eine Landbrücke am Ostufer von `map.tmx`.
- `flowfield`: Belegung als Komponente pro Karte, Fenster-Belegung zusammensetzen, `create_grids`
  entfällt. Tests bauen die Belegung von Karten direkt statt eines Rasters über eine Karte.
- `sight`: dasselbe für die undurchsichtigen Zellen (`OpaqueCells`).
- `enemy`: System zum Entladen.

## Tests

- Hindernis am Rand einer Karte blockiert die Zellen der Nachbarkarte innerhalb der Clearance.
- Weg über eine Kartengrenze hinweg wird gefunden.
- Zellen außerhalb geladener Karten sind blockiert.
- Kollider ändert sich: nur die Belegung seiner Karte wird neu gerastert.
- Karte entladen: Ihre Belegung verschwindet aus dem Fenster.
- Gegner verlässt alle geladenen Karten: wird despawnt.
- Gegner wechselt zwischen zwei geladenen Karten: bleibt erhalten.

## Offen

- **Ein- und Ausladen an der Grenze:** bevy_ecs_tiled lädt und entlädt mit demselben Rechteck.
  Läuft der Spieler an der Grenze hin und her, wird eine Karte ständig neu gespawnt. Erst
  beobachten. Hilft nur eigenes Chunking (`TiledWorldChunking(None)` und eigene Logik) mit
  größerem Rechteck fürs Entladen.
- **Kosten beim Spawnen:** Eine Karte zu spawnen trianguliert ihre Kollider im Hauptthread. Bei
  Rucklern Karten kleiner halten oder vorladen.
- **Startpunkt:** Tiled-Objekt (z. B. `type="spawn"`) oder zuletzt benutzter Brunnen.
