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

## Hinterhalte

- Trigger-Zonen in Tiled, die mit Spawnpunkt-Objekten verknüpft sind (Objekt-Referenz als
  Property).
- Werden einmalig ausgelöst. Art und Anzahl der Gegner sind Properties am Spawnpunkt.
- Optional gestaffelt: zuerst kleine Gegner, danach große.

## Fog of War

- Unerkundete Bereiche sind schwarz, erkundete bleiben sichtbar, aber ohne Gegner.
- Dazu eine echte Sichtlinie: Wände und Gebäude werfen Schatten. Die Schattenformen lassen sich aus
  den Tiled-Kollidern berechnen.
- Ein Sichtradius um den Spieler.

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

- Nur an festen Speicherpunkten, also an den Brunnen. Beim Tod wird der letzte Brunnen-Stand
  geladen.
- Gespeichert wird nur, was sich gegenüber der Tiled-Karte geändert hat: offene Truhen, ausgelöste
  Hinterhalte, aktive Brunnen, Items am Boden, Spieler mit Inventar, erkundeter Fog of War
  (Bitmaske).
- Veränderbare Objekte bekommen `Persistent` mit Kartenname und Tiled-ID. Der Zustand steckt in
  Komponenten wie `Opened` oder `Triggered`.
- Laden: Karte normal spawnen, danach gespeicherte Zustände anwenden.
- Items brauchen eine eigene stabile ID, weil sich Entity-IDs bei jedem Start ändern.
- Struktur `SaveGame` als RON, mit Versionsnummer. Zuerst in eine temporäre Datei schreiben, dann
  umbenennen. Die Spielerposition ist die ID des Brunnens.
- Optional statt Tiled-IDs eine Property `uuid` per Tiled-Script, mit CI-Prüfung auf doppelte oder
  fehlende Werte.

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
  Tile-Ebene und hat damit keine Tiled-ID. Für `Persistent` und die Brunnen-ID als Spielerposition
  müssen Brunnen (und Truhen) Tile-Objekte werden, oder die Tile-Position dient als Schlüssel.
- **Stabile IDs früh einführen:** Wird die Karte für das neue Tileset neu gebaut, ändern sich die
  Tiled-IDs und alte Speicherstände passen nicht mehr. Die `uuid`-Property sollte daher vor dem
  ersten Speicherstand kommen, den jemand behalten will.
- **Gewicht als Masse:** `Strength` begrenzt nur die Kraft. Ein schwererer Spieler beschleunigt
  langsamer und wird schwerer weggeschubst, erreicht aber dieselbe Höchstgeschwindigkeit. Soll
  Überladung auch langsamer machen, muss zusätzlich `DesiredVelocity` skaliert werden.
- **Tod:** Bisher heilt `combat` den Spieler und entfernt alle Gegner. Das Laden des letzten
  Brunnen-Stands ersetzt das. Vorher den TODO-Punkt umsetzen, dass `combat` ein `Died`-Event
  auslöst.

## Mögliche Reihenfolge

Jeder Schritt baut auf dem vorherigen auf:

1. Hinterhalte und Aktivierungsradien
2. Flowfield-Optimierung (zusammen mit dem Performance-Punkt in `TODO.md`)
3. Items und Inventar
4. Truhen und Gebiets-Level
5. Brunnen
6. Speichern
7. Fog of War

Grafik (Sprites, neues Tileset) läuft unabhängig nebenher.
