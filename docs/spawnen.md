# Konzept: Spawnen

Noch nicht umgesetzt. Bisher spawnen Gegner nur über die Debug-Tasten E / R / T und den HUD-Button,
immer an derselben Stelle.

## Ziel

- Mehrere Ansätze, die sich nicht ausschließen. Je nach Gebiet und Situation gilt einer allein oder
  eine Mischung, auch gleichzeitig. Keiner muss umgesetzt werden.
- Sie unterscheiden sich darin, wer entscheidet, wo Gegner sind:
  - die Welt (feste Populationen und Gegner),
  - der Level-Designer (Hinterhalte),
  - die Situation (Director),
  - ein Ereignis (Wellen).
- Gegner tauchen nie im Blickfeld des Spielers auf.

## Gemeinsame Basis

- Bisher enthält `SpawnEnemy` nur den Gegner (seinen Radius), `spawn_enemy` setzt ihn immer auf
  `(50, -50)`.
- Künftig bringt `SpawnEnemy` auch die Position mit. Die Quellen entscheiden, was wo spawnt,
  `spawn_enemy` baut nur die Entity. Die Debug-Tasten und der HUD-Button geben ihren Ort ebenfalls
  mit, etwa die Mausposition.

## Hinterhalte

- Eine Objekt-Ebene pro Hinterhalt (z. B. „Hinterhalt Brücke“, gesammelt unter einer
  Gruppen-Ebene „Hinterhalte“). Darin eine oder mehrere Trigger-Zonen und die Spawnpunkte. Beim
  Auslösen gelten die Spawnpunkte derselben Ebene.
- Ebenen haben keine Objekt-ID (siehe [speichern.md](speichern.md)). Ausgelöst wird deshalb an
  den Trigger-Zonen vermerkt (`Triggered`), bei mehreren Zonen an allen der Ebene.
- Werden einmalig ausgelöst. Art und Anzahl der Gegner sind Properties am Spawnpunkt.
- Optional gestaffelt: zuerst kleine Gegner, danach große (z. B. `delay` am Spawnpunkt).
- Mehr Spawnpunkte als Gegner. Beim Auslösen werden nur passende genommen: nicht sichtbar, mit
  einer Laufdistanz über einem Minimum, erreichbar. So läuft derselbe Hinterhalt je nach Richtung
  des Spielers anders ab.
- Die Laufdistanz gibt es nur innerhalb des Flowfield-Radius. Spawnpunkte deshalb in diesem Radius
  um die Trigger-Zone legen, sonst bleibt nur die Sichtprüfung.
- Trigger-Zonen werden wie der Brunnen zu `Sensor + CollisionEventsEnabled`. Spawnpunkte haben
  keinen Kollider. Sie brauchen ein Observer auf das Erzeugen von Objekten, nicht auf
  `ColliderCreated`.

## Director (wie Left 4 Dead)

Ein allgemeines Werkzeug, unabhängig von Brunnen oder anderen Story-Elementen. Er kennt nur seine
Parameter, wer sie setzt, ist ihm egal.

- **Intensität:** Steigt, wenn der Spieler Schaden nimmt, Gegner nah sind oder in der Nähe Gegner
  sterben. Sie fällt langsam wieder ab.
- **Phasen:** Aufbau (spawnen, bis die Intensität einen Höchstwert erreicht), Höhepunkt kurz halten,
  Abklingen, Ruhe ohne Spawns. Die Ruhephasen unterscheiden ihn von Vampire-Survivors-Wellen.
- **Budget pro Aufbauphase:** In Gruppen spawnen, bevorzugt in Laufrichtung des Spielers. Die Orte
  kommen aus der Suche außer Sicht (siehe unten).
- **Parameter:** Budget, Höchstwert der Intensität, Dauer von Höhepunkt und Ruhe, erlaubte
  Gegnerarten. Ein Budget von null schaltet ihn aus.
- **Woher die Parameter kommen:** aus Zonen in Tiled (Properties an der Zone, wie `level`), in denen
  der Spieler gerade steht, und aus Modifikatoren im Code, die sie verrechnen. Beispiele: Nacht
  und Gebiets-Level erhöhen das Budget, eine Schutzzone setzt es auf null. Ein aktiver Brunnen wäre
  so eine Schutzzone, ein inaktiver eine Zone mit hohem Budget.

## Feste Populationen und Gegner

- Jedes Gebiet hat eine feste Menge Gegner, die von Anfang an auf der Karte stehen und schlafen,
  bis der Spieler in die Sichtweite kommt (siehe „Radien um den Spieler“ in
  [ideen.md](ideen/ideen.md)).
- Getötete bleiben tot, oder das Gebiet füllt sich nach einer Weile wieder auf.
- Vorhersehbar: Erkunden lohnt sich, und der Spieler kann Gebiete meiden oder säubern.

### Einzelne feste Gegner

- In Tiled als Objekte platziert, für die Story, Spezialgegner mit eigenem Loot oder Endgegner.
  Werte und Loot als Properties, wie bei Story-Waffen nur für Einzelstücke.
- Als Tiled-Objekte haben sie eine Objekt-ID: Ein getöteter landet als `Removed` im Zustand seiner
  Gruppe und bleibt tot (siehe [speichern.md](speichern.md)). Gespeichert wird nur der Tod. Ein
  lebender kommt nach dem Entladen seiner Karte im Tiled-Stand zurück, mit Position und Leben aus
  Tiled.
- Läuft ein fester Gegner in eine andere Karte und wird seine eigene neu geladen, darf er nicht
  doppelt entstehen. Vor dem Spawnen per Gruppen- und Objekt-ID prüfen, ob er noch lebt (siehe
  [welt.md](welt.md)).

## Wellen

- Für Ereignisse wie das Verteidigen eines Brunnens oder die Nacht.
- Ein Budget pro Welle, Pausen dazwischen. Die Gegner einer Welle versetzt spawnen, gern in
  Gruppen.

## Ort außer Sicht

Für Director und Wellen, die anders als Hinterhalte keine festen Spawnpunkte haben.

- **Ort:** In Zellen, die der Spieler gerade nicht sieht (`Sight`, siehe [sicht.md](sicht.md)),
  etwa hinter Gebäuden. Sie müssen innerhalb des Flowfield-Radius liegen und im Gitter für den
  Radius des Gegners frei sein. Die Sichtbarkeit mit dem Radius des Gegners prüfen, damit nichts
  hervorschaut.
- **Mindestabstand:** Über die Flowfield-Distanz messen, nicht über die Luftlinie, sonst stehen
  Gegner direkt hinter einer dünnen Wand.
- **Gewichtung:** Ein Band in der Laufdistanz (etwa 120–250 px). Darin werden Zellen bevorzugt,
  deren Laufdistanz klein ist gegenüber der Luftlinie, also Zellen hinter etwas, statt Zellen im
  offenen Feld knapp außerhalb der Sichtweite.
- **Flowfield anfordern:** Die Distanzen gibt es nur für Gitter, die gerade ein Gegner (oder das
  Debug-Overlay) nutzt, und nur innerhalb des Flowfield-Radius. Der Spawner muss das Gitter für die
  Größe des neuen Gegners anfordern und gespawnt wird eine Suche später. Das gilt auch für
  Hinterhalte.

## Zusammenspiel

- Der Director zählt alle Gegner in der Nähe, egal aus welcher Quelle, sonst stapeln sich die
  Ansätze. Ein ausgelöster Hinterhalt ist ein erzwungener Höhepunkt, danach gibt der Director Ruhe.
- Jeder Gegner merkt sich seine Quelle. Sie bestimmt, was passiert, wenn er weit zurückfällt: Ein
  Gegner der Population schläft ein und bleibt, wo er ist. Ein Gegner des Directors wird recycelt
  und geht zurück ins Budget.
- Die Mischung pro Gebiet als Property an der Gebiets-Zone (wie `level`). Beispiele: ein Wald mit
  einer Wolfs-Population und ruhigem Director, das Gebiet um einen verfluchten Brunnen mit
  aggressivem Director und Hinterhalten.

## Verhalten

- Angriffs-Tokens, damit nur wenige Gegner gleichzeitig angreifen und der Rest wartet oder den
  Spieler umkreist.
- Reize wie Lärm locken Gegner an.

## Code

- `objects/enemy.rs`: `SpawnEnemy` mit Position, `spawn_enemy` baut nur die Entity. Die Debug-Tasten
  bleiben dort.
- `src/spawning/`: eigenes Modul für die Quellen, sobald es mehr als die Debug-Tasten gibt. Eine
  Datei pro Quelle (Hinterhalte, Director, Wellen) und eine für die Suche außer Sicht.

## Offen

- **Mischung der Spawn-Ansätze:** Welche Ansätze wo gelten. Offen ist vor allem, welche Zonen den
  Director wie einstellen und ob es eine Grundeinstellung außerhalb aller Zonen gibt.
- **Sicht zwischen den Radien:** Zwischen Sichtweite (200 px) und Flowfield-Radius (320 px) ist jede
  Zelle unsichtbar. Ohne weitere Regel spawnen Gegner vor allem dort, nicht hinter Gebäuden in der
  Nähe. Die Gewichtung oben ist ein Vorschlag, was gewollt ist, muss sich beim Ausprobieren zeigen.
- **Nachts:** Wo nachts gespawnt werden darf, hängt davon ab, ob der Spieler beleuchtete Bereiche
  außerhalb seiner Sicht sieht und ob ein Gegner im Dunkeln als sichtbar zählt (siehe „Licht und
  Sicht“ in [ideen.md](ideen/ideen.md)).
