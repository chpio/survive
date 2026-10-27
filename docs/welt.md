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
  negativ). Die festen Startpunkte von Spieler und Debug-Gegnern (`50, -50`) müssen dazu passen.
  Später kommt der Startpunkt aus einem Tiled-Objekt.
- Die World lädt alle `.tmx` als Assets beim Start. Nur die Entities entstehen erst, wenn eine
  Karte in den Ladebereich kommt.

## Laden und Entladen

- `TiledWorldChunking::new(x, y)` an der World gibt die **halbe** Breite und Höhe eines Rechtecks
  um die Kamera an. Karten, deren Rechteck es schneidet, werden gespawnt, alle anderen samt Kindern
  (Ebenen, Objekte, Kollider) despawnt.
- Geprüft wird nur, wenn sich die Kamera bewegt hat. Gemessen wird an der Kamera, die dem Spieler
  folgt.
- Der Ladebereich muss abdecken:
  - den sichtbaren Ausschnitt, also die halbe Fenstergröße mal Zoom,
  - das Flowfield-Fenster: `FLOW_FIELD_RADIUS` plus größte Clearance, derzeit 336 px, sonst fehlen
    am Rand des Fensters Hindernisse,
  - später den Deaktivierungs-Radius aus [ideen.md](ideen/ideen.md).
- Ein System setzt `TiledWorldChunking` neu, wenn sich Fenstergröße oder Zoom ändern, und nimmt
  das Maximum aus diesen Werten.

## Hindernisse pro Karte

Bisher deckt das Flowfield genau eine Karte ab: `create_grids` ersetzt bei jedem `MapCreated` alle
Raster, und jede Änderung eines Kolliders rastert alle statischen Kollider neu. Mit mehreren Karten
gewänne die zuletzt geladene.

Das Flowfield hat zwei Teile, die sich unterschiedlich verhalten:

- **Belegung** (`blocked`, `solid` pro Clearance): hängt nur an den statischen Kollidern einer
  Karte.
- **Distanzen**: das Fenster um den Spieler, wandert mit ihm.

Deshalb:

- Die Belegung jeder Karte steckt als Komponente an der Karten-Entity und verschwindet mit ihr.
  Ändert sich ein statischer Kollider, wird nur die Belegung seiner Karte neu gerastert. Die Karte
  eines Kolliders ergibt sich über seine Eltern.
- Die Belegung deckt die Karte plus einen Rand von der größten Clearance (16 px, 4 Zellen) ab. Ein
  Baum am Rand von Karte A blockiert so auch die Zellen von Karte B, die ihm zu nah sind.
- Das Fenster bleibt in der Ressource `FlowField`. Seine Belegung wird aus den Komponenten der
  geladenen Karten zusammengesetzt, wenn sich das Fenster verschiebt oder eine Karte geladen,
  entladen oder neu gerastert wird. Eine Zelle ist blockiert, wenn eine der Karten sie blockiert,
  deren Rechteck mit Rand die Zelle enthält (höchstens vier). Zellen außerhalb aller geladenen
  Karten sind blockiert.
- Die Suche selbst bleibt, wie sie ist. Sie liest nur noch die Belegung des Fensters statt die der
  ganzen Karte.
- Das Debug-Overlay zeigt die Belegung des Fensters statt der ganzen Karte.

Die Sicht geht genauso vor: Die undurchsichtigen Zellen jeder Karte stecken an der Karte, ohne
Rand, weil die Sicht keine Clearance hat. Zellen außerhalb geladener Karten sind durchsichtig.

`flowfield::map_area` rechnet mit der `Transform` der Karte. Die ist jetzt relativ zur World, das
passt, solange die World im Ursprung bleibt.

## Gegner

Gegner gehören nicht zur Karte und werden nicht automatisch entladen. Bleiben sie stehen, laufen
sie durch die fehlenden Kollider.

- Gegner bleiben ohne Eltern.
- Ein System prüft regelmäßig (etwa einmal pro Sekunde), ob jeder Gegner noch auf einer geladenen
  Karte steht. Dafür reichen die Rechtecke der geladenen Karten, mit etwas Puffer am Rand, sonst
  flackern Gegner an der Grenze.
- Steht er auf keiner, wird er despawnt. Gegner werden nicht gespeichert.
- Feste Gegner aus Tiled-Objekten entstehen beim Laden ihrer Karte neu. Läuft einer in eine andere
  geladene Karte und wird seine eigene neu geladen, darf er nicht doppelt entstehen: vor dem
  Spawnen per Objekt-ID prüfen, ob er noch lebt.
- Alternative: nur beim Entladen einer Karte die Gegner in ihrem Rechteck prüfen.
- Verworfen: Gegner als Kinder der Karte. Läuft ein Gegner in eine andere Karte, gehört er noch zur
  alten und verschwindet mit ihr, außer man hängt ihn bei jedem Kartenwechsel um.

## Items

- Items am Boden bleiben vorerst bestehen, wenn ihre Karte entladen wird. Sie haben keine Physik
  und brauchen die Kollider nicht.
- Mit dem Speichern kommen sie in Zellen eines festen Rasters und werden mit entladen.

## Code

- `main.rs`: `TiledWorld` statt `TiledMap`, Einstellungen und Observer an der World,
  `TiledWorldChunking`.
- `assets/`: eine `.world`-Datei. Zuerst `map.tmx` als einzige Karte, dazu eine zweite
  Testkarte daneben.
- `flowfield`: Belegung als Komponente pro Karte, Fenster-Belegung zusammensetzen, `create_grids`
  entfällt. Tests bauen die Belegung von Karten direkt statt eines Rasters über eine Karte.
- `sight`: dasselbe für die undurchsichtigen Zellen.
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
