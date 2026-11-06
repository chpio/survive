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

Ausgearbeitet in [spawnen.md](../spawnen.md). Die Grundsätze:

- Mehrere Ansätze, die sich nicht ausschließen und je nach Gebiet gemischt werden: Hinterhalte,
  Director, feste Populationen und Gegner, Wellen.
- Hinterhalte: eine Objekt-Ebene pro Hinterhalt mit Trigger-Zonen und Spawnpunkten.
- Director wie in Left 4 Dead, als allgemeines Werkzeug, eingestellt über Zonen und Modifikatoren.
- Gegner spawnen außer Sicht, mit einem Mindestabstand über die Flowfield-Distanz.
- Feste Gegner sind Tiled-Objekte, von ihnen wird nur der Tod gespeichert.

## Fog of War

- Sichtradius und Sichtlinie sind umgesetzt ([sicht.md](../sicht.md)).
- Erkundete Bereiche (unerkundete schwarz, erkundete sichtbar, aber ohne Gegner) erst, wenn es eine
  Weltkarte oder Minimap gibt. Dann müssen sie auch gespeichert werden.

## Licht

Ausgearbeitet in [licht.md](../licht.md). Die Grundsätze:

- **Tag und Nacht:** Nachts dunkelt eine eigene Ebene über allem die Welt ab, und der Sichtradius
  wird kleiner.
- **Lichtquellen:** Eine Komponente an normalen Entities. Jedes Licht wird als weicher Kreis
  additiv in eine Lightmap gezeichnet, die die Nacht-Ebene aufhellt. Schatten gibt es nur für
  wenige große Lichter.
- **Color Grading:** Bevys `ColorGrading` pro Tageszeit, zwischen denen überblendet wird. Es
  braucht HDR, damit auch die Karte gefärbt wird. Später vielleicht LUTs.

## Radien um den Spieler (drei Kreise ineinander)

Umgesetzt, siehe „Schlafen und Aufwachen“ in [spawnen.md](../spawnen.md).

1. **Sichtweite (200 px):** Gegner werden aktiv, sobald der Spieler sie sieht (Sichtlinie, nicht
   nur Abstand).
2. **Flowfield-Radius (320 px):** etwas größer als die Sichtweite, damit verfolgende Gegner und
   Gegner aus Hinterhalten knapp im Nebel noch einen Weg haben.
3. **Deaktivierungs-Radius (400 px):** noch größer. Gegner schlafen erst wieder ein, wenn sie
   deutlich weiter weg sind als die Sichtweite. Dieser Abstand verhindert, dass sie an der Grenze
   ständig zwischen aktiv und inaktiv wechseln. Dazwischen laufen sie direkt auf den Spieler zu.

## Flowfield-Optimierung

Umgesetzt, siehe `src/flowfield/`:

- Ein Gitter wird nur berechnet, wenn wache Gegner dieser Größe oder ein Spawner (per
  `flowfield::Request`) es brauchen. Sonst wird gar nichts berechnet.
- Die Dijkstra-Suche ist auf ein Fenster im Flowfield-Radius um den Spieler begrenzt.
- Neu berechnet wird nur beim Zellwechsel des Spielers oder wenn sich die geladenen Karten ändern.

## Items

- Arten: Verbrauchsgüter (Heiltrank, Essen), Ausrüstung (Waffen, Rüstung), Story-Gegenstände
  (Briefe, Schlüssel).
- Truhen als Tiled-Objekte mit `type` „chest“, Inhalt per Property.
- Besser über Tiled Custom Types: Rust-Komponente `Chest` mit `Reflect` registrieren, über das
  Feature `user_properties` von `bevy_ecs_tiled` als JSON exportieren und in Tiled unter View →
  Custom Types Editor → Import einlesen.
- Item-Werte zentral festlegen, umgesetzt in `assets/items.ron` ([items.md](../items.md)). Die
  Truhe verweist nur auf den Grundtyp. Eigene Werte in Tiled nur für Einzelstücke (Story-Waffen).
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
- Entschieden: Figuren bekommen Skeletal-Animation statt Spritesheets, da Einzelbilder zu groß
  werden. Erstmal starre, einfarbige 2D-Teile ohne Skinning, jedes an einem Knochen. Animiert
  wird in Blender, exportiert als glTF (Hierarchie und Bewegungen); in Bevy wird jedes Teil als
  2D-Fläche mit dem `SightMaterial` gezeichnet, so wie die Gegner heute. Übergänge zwischen
  Animationen übernimmt Bevys Animationssystem. Kein Spine: Die Lizenz der Runtimes passt nicht
  zu einem Open-Source-Spiel. Skinning (sich biegende Teile) bräuchte Bevys 3D-Pipeline und
  eine 3D-Fassung des `SightMaterial`, also erst später, falls nötig.

## Inventar

Die erste Version ist umgesetzt ([items.md](../items.md)): Items als Entities mit `HeldBy` und
`Inventory`, Stapeln, Definitionen in RON, Benutzen und das Inventarfenster im HUD.

- Items sind eigene Entities, die per Relationship am Besitzer hängen (Spieler, Gegner, Truhe).
  Grund: Haltbarkeit und Verzauberungen machen jedes Exemplar einzigartig.
- Aufheben bzw. Fallenlassen = Sprite und Position entfernen bzw. wieder setzen und das Item
  umhängen.
- Stapeln über eine Komponente `Stack` mit Anzahl: beim Aufheben zusammenführen und die zweite
  Entity despawnen, beim Teilen eine neue spawnen. Nur Items ohne eigenen Zustand werden gestapelt
  (`Unique`), auch am Boden (umgesetzt, siehe [items.md](../items.md)).
- Kein festes Limit: Das Inventargewicht zählt zur Masse des Spielers. Er wird träger und schwerer
  wegzuschubsen. Die natürliche Grenze entsteht über die Stärke.
- Überladung macht auch langsamer. `Strength` begrenzt nur die Kraft: Ein schwererer Spieler
  beschleunigt langsamer, erreicht aber dieselbe Höchstgeschwindigkeit. Deshalb wird zusätzlich
  die `DesiredVelocity` des Spielers mit dem Gewicht herunterskaliert.
- Benutzen ist umgesetzt: Die Wirkung steht in der Definition, `ItemUsed` meldet sie.
- Jedes Item hat einen Wert, sodass Verkaufen oder Verarbeiten immer etwas bringt. Unnützes wirft
  man daher nicht weg, sondern macht es zu Geld oder Material.
- Zerlegen direkt im Inventar statt eines Mülleimers: Ein Button macht aus dem Item Material, ohne
  Händler oder Werkbank. So wird man Items überall los, und die Karte füllt sich nicht mit
  liegengelassenen Items. Verkaufen kommt mit den Händlern dazu.
- Aufheben per Taste (E) für das nächste Item in Reichweite, dazu der Mausklick. Automatisch beim
  Drüberlaufen nur für Typen, die das in der Definition erlauben (leichte wie Tränke).
  Selbst abgelegte Items hebt das automatische Aufheben erst wieder auf, wenn der Spieler sich
  einmal außer Reichweite entfernt hat, sonst ließe sich nichts ablegen.
- Noch offen: Gewicht als Masse, Icons statt der Platzhalterfarbe, Ausrüstung wirkt auf
  Kampfwerte, Wert und Zerlegen in Material, Verkaufen (braucht Händler), Aufheben per Taste und
  automatisch.

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

Umgesetzt ([items.md](../items.md), [ui-events.md](../ui-events.md)), bis auf den Tooltip.

- Bevy Picking an Sprites: Observer für `Pointer<Click>` mit Distanzprüfung zum Spieler,
  `Pointer<Over>` für Hervorhebung und Tooltip.
- Eigenes Picking-Backend für das Dioxus-HUD (ca. 50–100 Zeilen): Blitz-Hit-Test an der
  Mausposition.
- Nur Elemente mit der Klasse `blocks-pointer` (oder deren Kinder) zählen als Treffer, also auch
  ganze Fenster wie das Inventar. Wurzel und `body` blockieren nicht.
- Trifft das HUD, geht der Klick ans UI, sonst an die Welt. Dafür meldet das HUD-Backend seine
  Treffer mit höherer Priorität als die Sprites.
- Auf passende Koordinaten bei Skalierung und Vollbild achten.

## Offene Punkte

- **Brunnen und Truhen als Objekte:** Der Brunnen ist bisher ein Tile mit `type="well"` in einer
  Tile-Ebene und kann damit keine Objekt-ID tragen. Für den gespeicherten Zustand und die
  Brunnen-ID als Spielerposition müssen Brunnen (und Truhen) Tile-Objekte werden.
- **Tiled-Skript früh einführen:** Objekte ohne Objekt-ID lassen sich nicht speichern. Das Skript
  sollte daher vor dem ersten Speicherstand kommen, den jemand behalten will.
- **Tod:** Bisher heilt `combat` den Spieler und entfernt alle Gegner. Das Laden des letzten
  Brunnen-Stands ersetzt das. Vorher den TODO-Punkt umsetzen, dass `combat` ein `Died`-Event
  auslöst.
- **Licht und Sicht:** Sieht der Spieler beleuchtete Bereiche außerhalb seiner Sicht, etwa ein
  Lagerfeuer hinter einer Wand? Und zählt ein Gegner im Dunkeln als sichtbar? Davon hängt ab, wo
  nachts gespawnt werden darf (siehe [spawnen.md](../spawnen.md)).

## Mögliche Reihenfolge

Jeder Schritt baut auf dem vorherigen auf:

1. Große Welt aus mehreren Karten ([welt.md](../welt.md), umgesetzt)
2. Hinterhalte und Aktivierungsradien, Spawnen außer Sicht ([spawnen.md](../spawnen.md),
   umgesetzt)
3. Flowfield-Optimierung (umgesetzt)
4. Items und Inventar (erste Version und Benutzen umgesetzt, offen: Gewicht als Masse)
5. Truhen und Gebiets-Level
6. Brunnen
7. Speichern

Grafik (Sprites, neues Tileset) und Licht laufen unabhängig nebenher.
