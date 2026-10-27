# Survive – Ideensammlung Inhalt

## Grundrichtung

- Story-getrieben statt Vampire-Survivors-Wellen: Der Spieler erkundet eine größere Welt.
- Gegner kommen vor allem aus Hinterhalten.

## Story-Gerüst: Brunnen (später)

- Jedes Gebiet hat einen versiegten oder verfluchten Brunnen. Solange er inaktiv ist, gibt es dort
  viele Gegner und Hinterhalte.
- Aktivierung durch Verteidigen oder durch einen Gegenstand aus dem Gebiet.
- Ein aktiver Brunnen heilt, dient als Speicherpunkt, und in seiner Nähe spawnen keine Gegner.
- An jedem Brunnen gibt es ein Stück Story. Am Ende stehen die Quelle des Fluchs und ein Endgegner.

## Spawnen

Mehrere Ansätze, die sich nicht ausschließen. Je nach Gebiet und Situation gilt einer allein oder
eine Mischung, auch gleichzeitig. Keiner muss umgesetzt werden. Sie unterscheiden sich darin, wer
entscheidet, wo Gegner sind: die Welt (Population), der Level-Designer (Hinterhalt), die Situation
(Director) oder ein Ereignis (Wellen).

- **Gemeinsame Basis:** `SpawnEnemy` bringt neben dem Gegner auch die Position mit. Die Quellen
  entscheiden, was wo spawnt, `spawn_enemy` baut nur die Entity.

### Hinterhalte

- Eine Objekt-Ebene pro Hinterhalt (z. B. „Hinterhalt Brücke“, gesammelt unter einer
  Gruppen-Ebene „Hinterhalte“). Darin eine oder mehrere Trigger-Zonen und die Spawnpunkte. Beim
  Auslösen gelten die Spawnpunkte derselben Ebene.
- Ebenen haben keine Objekt-ID (siehe [speichern.md](../speichern.md)). Ausgelöst wird deshalb an
  den Trigger-Zonen vermerkt, bei mehreren Zonen an allen der Ebene.
- Werden einmalig ausgelöst. Art und Anzahl der Gegner sind Properties am Spawnpunkt.
- Optional gestaffelt: zuerst kleine Gegner, danach große (z. B. `delay` am Spawnpunkt).
- Mehr Spawnpunkte als Gegner. Beim Auslösen werden nur passende genommen: nicht sichtbar, mit
  einer Laufdistanz über einem Minimum, erreichbar. So läuft derselbe Hinterhalt je nach Richtung
  des Spielers anders ab.
- Die Laufdistanz gibt es nur innerhalb des Flowfield-Radius. Spawnpunkte deshalb in diesem Radius
  um die Trigger-Zone legen, sonst bleibt nur die Sichtprüfung.
- Spawnpunkte haben keinen Kollider. Sie brauchen ein Observer auf das Erzeugen von Objekten, nicht
  auf `ColliderCreated`.

### Director (wie Left 4 Dead)

Ein allgemeines Werkzeug, unabhängig von Brunnen oder anderen Story-Elementen. Er kennt nur seine
Parameter, wer sie setzt, ist ihm egal.

- **Intensität:** Steigt, wenn der Spieler Schaden nimmt, Gegner nah sind oder in der Nähe Gegner
  sterben. Sie fällt langsam wieder ab.
- **Phasen:** Aufbau (spawnen, bis die Intensität einen Höchstwert erreicht), Höhepunkt kurz halten,
  Abklingen, Ruhe ohne Spawns. Die Ruhephasen unterscheiden ihn von Vampire-Survivors-Wellen.
- **Budget pro Aufbauphase:** In Gruppen spawnen, bevorzugt in Laufrichtung des Spielers.
- **Parameter:** Budget, Höchstwert der Intensität, Dauer von Höhepunkt und Ruhe, erlaubte
  Gegnerarten. Ein Budget von null schaltet ihn aus.
- **Woher die Parameter kommen:** aus Zonen in Tiled (Properties an der Zone, wie `level`), in denen
  der Spieler gerade steht, und aus Modifikatoren im Code, die sie verrechnen. Beispiele: Nacht
  (siehe Licht) und Gebiets-Level erhöhen das Budget, eine Schutzzone setzt es auf null. Ein aktiver
  Brunnen wäre so eine Schutzzone, ein inaktiver eine Zone mit hohem Budget.

### Feste Populationen

- Jedes Gebiet hat eine feste Menge Gegner, die von Anfang an auf der Karte stehen und schlafen,
  bis der Spieler in die Sichtweite kommt (siehe Radien um den Spieler).
- Getötete bleiben tot, oder das Gebiet füllt sich nach einer Weile wieder auf.
- Vorhersehbar: Erkunden lohnt sich, und der Spieler kann Gebiete meiden oder säubern.
- **Einzelne feste Gegner:** In Tiled als Objekte platziert, für die Story, Spezialgegner mit
  eigenem Loot oder Endgegner. Werte und Loot als Properties, wie bei Story-Waffen nur für
  Einzelstücke. Als Tiled-Objekte haben sie eine Objekt-ID: Ein getöteter landet als `Removed` im
  Zustand seiner Gruppe und bleibt tot (siehe [speichern.md](../speichern.md)). Gespeichert wird nur
  der Tod. Ein lebender kommt nach dem Entladen seiner Karte im Tiled-Stand zurück, mit Position und
  Leben aus Tiled.
- Läuft ein fester Gegner in eine andere Karte und wird seine eigene neu geladen, darf er nicht
  doppelt entstehen. Vor dem Spawnen per Gruppen- und Objekt-ID prüfen, ob er noch lebt.

### Wellen

- Für Ereignisse wie das Verteidigen eines Brunnens oder die Nacht.
- Ein Budget pro Welle, Pausen dazwischen. Die Gegner einer Welle versetzt spawnen, gern in
  Gruppen.

### Ort außer Sicht (für Director und Wellen)

- **Ort:** In Zellen, die der Spieler gerade nicht sieht (`Sight`), etwa hinter Gebäuden. Sie
  müssen innerhalb des Flowfield-Radius liegen und im Gitter für den Radius des Gegners frei sein.
  Die Sichtbarkeit mit dem Radius des Gegners prüfen, damit nichts hervorschaut.
- **Mindestabstand:** Über die Flowfield-Distanz messen, nicht über die Luftlinie, sonst stehen
  Gegner direkt hinter einer dünnen Wand.
- **Gewichtung:** Ein Band in der Laufdistanz (etwa 120–250 px). Darin werden Zellen bevorzugt,
  deren Laufdistanz klein ist gegenüber der Luftlinie, also Zellen hinter etwas, statt Zellen im
  offenen Feld knapp außerhalb der Sichtweite.

### Zusammenspiel

- Der Director zählt alle Gegner in der Nähe, egal aus welcher Quelle, sonst stapeln sich die
  Ansätze. Ein ausgelöster Hinterhalt ist ein erzwungener Höhepunkt, danach gibt der Director Ruhe.
- Jeder Gegner merkt sich seine Quelle. Sie bestimmt, was passiert, wenn er weit zurückfällt: Ein
  Gegner der Population schläft ein und bleibt, wo er ist. Ein Gegner des Directors wird recycelt
  und geht zurück ins Budget.
- Die Mischung pro Gebiet als Property an der Gebiets-Zone (wie `level`). Beispiele: ein Wald mit
  einer Wolfs-Population und ruhigem Director, das Gebiet um einen verfluchten Brunnen mit
  aggressivem Director und Hinterhalten.

### Verhalten

- Angriffs-Tokens, damit nur wenige Gegner gleichzeitig angreifen und der Rest wartet oder den
  Spieler umkreist.
- Reize wie Lärm locken Gegner an.

## Fog of War

- Sichtradius und Sichtlinie sind umgesetzt ([sicht.md](../sicht.md)).
- Erkundete Bereiche (unerkundete schwarz, erkundete sichtbar, aber ohne Gegner) erst, wenn es eine
  Weltkarte oder Minimap gibt. Dann müssen sie auch gespeichert werden.

## Licht

- **Tag und Nacht:** Ein Umgebungslicht-Faktor im Shader (`assets/shaders/sight.wesl`),
  multipliziert mit der vorhandenen Sichtmaske.
- **Color Grading:** LUTs pro Tageszeit, zwischen denen überblendet wird. Vorher testen, ob Bevys
  `ColorGrading` an der 2D-Kamera funktioniert.
- **Lichtquellen:** Eine Komponente an normalen Entities. Jedes Licht wird als Quad mit weichem
  Abfall (etwa per `smoothstep`) additiv in eine Lightmap gezeichnet. Schatten gibt es nur für
  wenige große Lichter.

## Radien um den Spieler (drei Kreise ineinander)

1. **Sichtweite:** Gegner werden aktiv, sobald sie darin auftauchen.
2. **Flowfield-Radius:** etwas größer als die Sichtweite, damit verfolgende Gegner und Gegner aus
   Hinterhalten knapp im Nebel noch einen Weg haben.
3. **Deaktivierungs-Radius:** noch größer. Gegner schlafen erst wieder ein, wenn sie deutlich weiter
   weg sind als die Sichtweite. Dieser Abstand verhindert, dass sie an der Grenze ständig zwischen
   aktiv und inaktiv wechseln.

## Flowfield-Optimierung

- Ein Gitter nur berechnen, wenn es aktive Gegner dieser Größe gibt. Ohne Gegner wird gar nichts
  berechnet.
- Die Dijkstra-Suche auf den Flowfield-Radius begrenzen.
- (Neuberechnung nur beim Zellwechsel des Spielers ist bereits umgesetzt.)

## Items

- Arten: Verbrauchsgüter (Heiltrank, Essen), Ausrüstung (Waffen, Rüstung), Story-Gegenstände
  (Briefe, Schlüssel).
- Truhen als Tiled-Objekte mit `type` „chest“, Inhalt per Property.
- Besser über Tiled Custom Types: Rust-Komponente `Chest` mit `Reflect` registrieren, über das
  Feature `user_properties` von `bevy_ecs_tiled` als JSON exportieren und in Tiled unter View →
  Custom Types Editor → Import einlesen.
- Item-Werte zentral festlegen (z. B. in RON). Die Truhe verweist nur auf den Grundtyp. Eigene Werte
  in Tiled nur für Einzelstücke (Story-Waffen).
- Grundtyp und Stufe trennen: eine Stufentabelle multipliziert die Grundwerte.

## Gebiets-Level und Loot

- Gebiets-Zonen in Tiled mit der Property `level`.
- Alles, was dort spawnt (Gegner, Truhen), übernimmt das Level.
- Gegner haben Loot-Tabellen (z. B. 30 % Heiltrank, 10 % Waffe). Die Stufe ergibt sich aus dem
  Gebiets-Level, mit etwas Zufall nach oben.
- Das Level skaliert auch die Gegnerwerte.

## Grafik

- Spieler und Gegner sind bisher Kreise. Sprites darüberlegen, die Kreise bleiben als Kollider.
- Das Tileset ersetzen, weil die Lizenz unklar ist. Tileset und Figuren am besten aus einem Pack.
- Quellen: Kenney (CC0), itch.io-Packs, LPC auf OpenGameArt (CC-BY-SA/GPL).
- SVG wäre möglich über `bevy_svg`, `bevy_vello` oder Rastern mit `resvg`. Für Animation und
  Pixel-Look sind PNG-Spritesheets einfacher.

## Inventar

- Items sind eigene Entities, die per Relationship am Besitzer hängen (Spieler, Gegner, Truhe).
  Grund: Haltbarkeit und Verzauberungen machen jedes Exemplar einzigartig.
- Aufheben bzw. Fallenlassen = Sprite und Position entfernen bzw. wieder setzen und das Item
  umhängen.
- Stapeln über eine Komponente `Stack` mit Anzahl: beim Aufheben zusammenführen und die zweite
  Entity despawnen, beim Teilen eine neue spawnen. Nur Items ohne eigenen Zustand werden gestapelt.
- Kein festes Limit: Das Inventargewicht zählt zur Masse des Spielers. Er wird träger und schwerer
  wegzuschubsen. Die natürliche Grenze entsteht über die Stärke.
- Noch offen: Item-Definitionen (Name, Icon, Grundwerte, stapelbar), Ausrüstung wirkt auf
  Kampfwerte, Benutzen über Events, Oberfläche in Dioxus.

## Speichern

Ausgearbeitet in [speichern.md](../speichern.md). Die Grundsätze:

- Nur an festen Speicherpunkten, also an den Brunnen. Beim Tod wird der letzte Brunnen-Stand
  geladen.
- Gespeichert wird nur, was sich gegenüber Tiled geändert hat: offene Truhen, ausgelöste
  Hinterhalte, aktive Brunnen, getötete feste Gegner, Items am Boden, Spieler mit Inventar.
- Jede Karte hat eine Karten-ID, jedes Tiled-Objekt eine Objekt-ID und eine Gruppen-ID (die
  Karten-ID seiner ursprünglichen Karte), alle als Custom Properties. Ein Tiled-Skript
  vergibt sie beim Speichern der Karte, ein CI-Test prüft auf fehlende und doppelte IDs. Weder
  Dateiname noch Tiled-ID taugen als Schlüssel.
- Objekt-Zustände liegen pro Gruppe, abgelegte Items in Zellen eines festen Rasters.
- Laden: Karte normal spawnen, danach gespeicherte Zustände anwenden.

## Items mit der Maus aufheben

- Bevy Picking an Sprites: Observer für `Pointer<Click>` mit Distanzprüfung zum Spieler,
  `Pointer<Over>` für Hervorhebung und Tooltip.
- Eigenes Picking-Backend für das Dioxus-HUD (ca. 50–100 Zeilen): Blitz-Hit-Test an der
  Mausposition.
- Nur Elemente mit der Klasse `blocks-pointer` (oder deren Kinder) zählen als Treffer, also auch
  ganze Fenster wie das Inventar. Wurzel und `body` blockieren nicht.
- Trifft das HUD, geht der Klick ans UI, sonst an die Welt. Dafür meldet das HUD-Backend seine
  Treffer mit höherer Priorität als die Sprites.
- Auf passende Koordinaten bei Skalierung und Vollbild achten.
- Passt zum TODO, das HUD nur bei geändertem Hover-Zustand neu zu zeichnen: beides nutzt denselben
  Hit-Test.

## Offene Punkte

- **Radien:** Zwischen Flowfield- und Deaktivierungs-Radius sind Gegner aktiv, haben aber kein
  Feld. Sie brauchen dort ein eigenes Verhalten (direkt auf den Spieler zu, stehen bleiben, zum
  letzten bekannten Punkt), oder der Flowfield-Radius wird gleich dem Deaktivierungs-Radius.
- **Sichtweite oder Sichtlinie:** Werden Gegner hinter einer Wand innerhalb der Sichtweite aktiv,
  obwohl der Spieler sie nicht sieht? Für Hinterhalte vermutlich gewollt, aber festlegen.
- **Brunnen und Truhen als Objekte:** Der Brunnen ist bisher ein Tile mit `type="well"` in einer
  Tile-Ebene und kann damit keine Objekt-ID tragen. Für den gespeicherten Zustand und die
  Brunnen-ID als Spielerposition müssen Brunnen (und Truhen) Tile-Objekte werden.
- **Tiled-Skript früh einführen:** Objekte ohne Objekt-ID lassen sich nicht speichern. Das Skript
  sollte daher vor dem ersten Speicherstand kommen, den jemand behalten will.
- **Gewicht als Masse:** `Strength` begrenzt nur die Kraft. Ein schwererer Spieler beschleunigt
  langsamer und wird schwerer weggeschubst, erreicht aber dieselbe Höchstgeschwindigkeit. Soll
  Überladung auch langsamer machen, muss zusätzlich `DesiredVelocity` skaliert werden.
- **Tod:** Bisher heilt `combat` den Spieler und entfernt alle Gegner. Das Laden des letzten
  Brunnen-Stands ersetzt das. Vorher den TODO-Punkt umsetzen, dass `combat` ein `Died`-Event
  auslöst.
- **Mischung der Spawn-Ansätze:** Welche Ansätze wo gelten. Der Director ist ein allgemeines
  Werkzeug, offen ist nur, welche Zonen ihn wie einstellen und ob es eine Grundeinstellung außerhalb
  aller Zonen gibt.
- **Flowfield beim Spawnen:** Die Distanzen gibt es nur für Gitter, die gerade ein Gegner (oder das
  Debug-Overlay) nutzt, und nur innerhalb des Flowfield-Radius. Der Spawner muss das Gitter für die
  Größe des neuen Gegners anfordern und wartet bis zur nächsten Suche.
- **Sicht zwischen den Radien:** Zwischen Sichtweite (200 px) und Flowfield-Radius (320 px) ist jede
  Zelle unsichtbar. Ohne weitere Regel spawnen Gegner vor allem dort, nicht hinter Gebäuden in der
  Nähe. Der Mindestabstand bzw. eine Gewichtung entscheidet, was gewollt ist.
- **Licht und Sicht:** Sieht der Spieler beleuchtete Bereiche außerhalb seiner Sicht, etwa ein
  Lagerfeuer hinter einer Wand? Und zählt ein Gegner im Dunkeln als sichtbar? Davon hängt ab, wo
  nachts gespawnt werden darf.

## Mögliche Reihenfolge

Jeder Schritt baut auf dem vorherigen auf:

1. Große Welt aus mehreren Karten ([welt.md](../welt.md))
2. Hinterhalte und Aktivierungsradien, Spawnen außer Sicht
3. Flowfield-Optimierung (zusammen mit dem Performance-Punkt in `TODO.md`)
4. Items und Inventar
5. Truhen und Gebiets-Level
6. Brunnen
7. Speichern

Grafik (Sprites, neues Tileset) und Licht laufen unabhängig nebenher.
