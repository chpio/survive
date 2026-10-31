# Konzept: Spawnen

Umgesetzt sind die gemeinsame Basis, Hinterhalte, Schlafen und Aufwachen sowie der Ort außer
Sicht (`src/spawning/`). Director, feste Populationen und Wellen fehlen noch.

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

- `SpawnEnemy` bringt den Gegner (seinen Radius), die Position und ob er schlafend startet. Die
  Quellen entscheiden, was wo spawnt, `spawn_enemy` baut nur die Entity.
- Die Debug-Tasten E / R / T spawnen an der Mausposition, schlafend. Der HUD-Button spawnt einen
  wachen Gegner außer Sicht (siehe unten).

## Schlafen und Aufwachen

Die Radien um den Spieler aus [ideen.md](ideen/ideen.md):

- Ein schlafender Gegner (`enemy::Asleep`) steht still, greift nicht an und zählt nicht fürs
  Flowfield. Getroffen und geschubst werden kann er.
- Er wacht auf, sobald der Spieler einen Teil von ihm sieht (`Sight::sees` mit seinem Radius), also
  über die Sichtlinie, nicht über den Abstand allein. Hinter einer Wand bleibt er liegen, so lassen
  sich Gebiete umgehen.
- Ein wacher Gegner schläft ein, wo er steht, sobald er weiter als `DEACTIVATION_RADIUS` (400 px)
  vom Spieler entfernt ist. Zwischen Flowfield-Radius (320 px) und diesem Radius läuft er direkt auf
  den Spieler zu, wie jeder Gegner ohne Feld.
- Gegner aus Hinterhalten und vom Ort außer Sicht spawnen wach, sie sollen ja angreifen.

## Hinterhalte

- Eine Objekt-Ebene pro Hinterhalt (z. B. „Hinterhalt Brücke“, gesammelt unter einer
  Gruppen-Ebene „Hinterhalte“). Darin eine oder mehrere Trigger-Zonen (`type="ambush_trigger"`) und
  die Spawnpunkte (Punkte mit `type="ambush_spawn"`). Beim Auslösen gelten die Spawnpunkte
  derselben Ebene.
- Ebenen haben keine Objekt-ID (siehe [speichern.md](speichern.md)). Ausgelöst wird deshalb an
  den Trigger-Zonen vermerkt (`Triggered`), bei mehreren Zonen an allen der Ebene. Bis es
  Speichern gibt, kommt eine neu geladene Karte ungelöst zurück, der Hinterhalt lässt sich dann
  erneut auslösen.
- Werden einmalig ausgelöst. Art und Anzahl der Gegner sind Properties am Spawnpunkt: `radius`
  (px, Standard 5), `count` (1).
- Optional gestaffelt: `delay` (s nach dem Auslösen, Standard 0) am Spawnpunkt, etwa zuerst kleine
  Gegner, danach große.
- Mehr Spawnpunkte als Gegner. Die Property `spawn_points` an der Ebene begrenzt, wie viele genutzt
  werden (ohne sie alle passenden). Ist die Verzögerung eines Spawnpunkts um, wird er genommen,
  wenn er passt: Der Spieler sieht nichts von seinen Gegnern, und die Laufdistanz ist mindestens
  `ambush::MIN_DISTANCE` (80 px), also auch erreichbar. Von gleichzeitig fälligen gehen die mit der
  kleinsten Laufdistanz vor. Ein unpassender Spawnpunkt wird übersprungen, nicht später erneut
  geprüft. So läuft derselbe Hinterhalt je nach Richtung des Spielers anders ab.
- Die Laufdistanz gibt es nur innerhalb des Flowfield-Radius. Spawnpunkte außerhalb werden
  übersprungen, deshalb gehören sie in diesen Radius um die Trigger-Zone.
- Trigger-Zonen werden zu Sensoren mit `AmbushTrigger`. Ausgelöst wird, sobald eine ungelöste Zone
  den Spieler berührt, abgefragt im Kontaktgraphen von Avian. `CollisionStart` kommt für Tiled-Kollider
  nicht an, da Avian `CollisionEventsEnabled` nur beim Hinzufügen des Kolliders liest und der
  Observer es erst danach einfügt (das betrifft auch den Brunnen).
- Spawnpunkte haben keinen Kollider. Sie bekommen `SpawnPoint` im Observer auf `ObjectCreated`,
  nicht auf `ColliderCreated`.
- Testkarte: „Hinterhalt Brücke“ auf `map_east.tmx` am Ende der Landbrücke, mit Spawnpunkten
  hinter den beiden großen Bäumen und einem am Teich, den man von der Brücke aus sieht.

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

Für Director und Wellen, die anders als Hinterhalte keine festen Spawnpunkte haben. Umgesetzt in
`spawning::out_of_sight` (`find_place`, Message `SpawnOutOfSight`), bisher nur vom HUD-Button
genutzt.

- **Ort:** In Zellen, die der Spieler gerade nicht sieht (`Sight`, siehe [sicht.md](sicht.md)),
  etwa hinter Gebäuden. Sie müssen innerhalb des Flowfield-Radius liegen und im Gitter für den
  Radius des Gegners frei sein. Die Sichtbarkeit mit dem Radius des Gegners prüfen, damit nichts
  hervorschaut.
- **Mindestabstand:** Über die Flowfield-Distanz messen, nicht über die Luftlinie, sonst stehen
  Gegner direkt hinter einer dünnen Wand.
- **Gewichtung:** Ein Band in der Laufdistanz (120–250 px). Darin gewinnt die Zelle mit der
  kürzesten Luftlinie zum Spieler. Hinter etwas ist die Luftlinie klein gegenüber der Laufdistanz,
  so gewinnen Zellen hinter Gebäuden gegen Zellen im offenen Feld knapp außerhalb der Sichtweite.
  Gibt es nichts, wohinter man sich verstecken kann, spawnt der Gegner knapp außerhalb der
  Sichtweite. Die Wahl ist fest, mehrere Gegner hintereinander landen in derselben Zelle und gehen
  über die Trennung auseinander.
- **Flowfield anfordern:** Die Distanzen gibt es nur für Gitter, die gerade ein wacher Gegner (oder
  das Debug-Overlay) nutzt, und nur innerhalb des Flowfield-Radius. Der Spawner fordert das Gitter
  für die Größe des neuen Gegners mit einer Entity mit `flowfield::Request` an und spawnt, sobald es
  berechnet ist, also eine Suche später. Danach entfernt er die Anforderung. Das gilt auch für
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
  Spieler umkreist (siehe [schwarm.md](schwarm.md)).
- Reize wie Lärm locken Gegner an.

## Code

- `objects/enemy.rs`: `SpawnEnemy` mit Position, `spawn_enemy` baut nur die Entity. Dort liegen
  auch die Debug-Tasten, `Asleep` und `wake_and_sleep`.
- `src/spawning/`: eigenes Modul für die Quellen, eine Datei pro Quelle (bisher `ambush.rs`, später
  Director und Wellen) und `out_of_sight.rs` für die Suche außer Sicht. Läuft im Perf-Schritt
  „Spawnen“.
- `flowfield`: `Request`, `FlowField::walking_distance`, `Grid::reachable`. `sight`: `Sight::sees`.

## Offen

- **Mischung der Spawn-Ansätze:** Welche Ansätze wo gelten. Offen ist vor allem, welche Zonen den
  Director wie einstellen und ob es eine Grundeinstellung außerhalb aller Zonen gibt.
- **Sicht zwischen den Radien:** Zwischen Sichtweite (200 px) und Flowfield-Radius (320 px) ist jede
  Zelle unsichtbar. Die Gewichtung oben (kürzeste Luftlinie) holt Gegner hinter Gebäude in der
  Nähe. Ob das so gewollt ist oder etwas Zufall dazu soll, muss sich beim Ausprobieren zeigen.
- **Quelle merken:** Noch merkt sich kein Gegner seine Quelle. Alle schlafen beim Zurückfallen ein,
  auch die aus Hinterhalten. Mit dem Director braucht es die Quelle fürs Recyceln.
- **Nachts:** Wo nachts gespawnt werden darf, hängt davon ab, ob der Spieler beleuchtete Bereiche
  außerhalb seiner Sicht sieht und ob ein Gegner im Dunkeln als sichtbar zählt (siehe „Licht und
  Sicht“ in [ideen.md](ideen/ideen.md)).
