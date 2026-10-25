# Konzept: Sicht

## Ziel

- Gegner sieht man nur, wo der Spieler sie sehen kann: im Sichtradius und nicht hinter einem
  Sichtblocker. Schaut ein Gegner um eine Ecke, ist nur der sichtbare Teil zu sehen.
- Der sichtbare Bereich wird dargestellt: Außerhalb ist die Karte abgedunkelt, aber erkennbar.
  Sichtblocker werfen Schatten.

Es gibt keinen „erkundet“-Zustand. Er bringt nur etwas, wenn man Gebiete außerhalb der Sicht
wieder anschauen kann (Minimap, Weltkarte, freie Kamera). Bei einer Kamera, die dem Spieler folgt,
lohnen weder Speicher noch Spielstand dafür. Kommt eine Karte dazu, lässt er sich nachrüsten.

## Sichtblocker

- Die Property `blocks_sight = true` (bool) markiert, was die Sicht blockiert:
  - an einer **Kachelebene**: alle Kollider ihrer Kacheln (bevy_ecs_tiled fasst sie pro Ebene zu
    einem Kollider zusammen, deshalb gilt die Property für die ganze Ebene),
  - an einem **Objekt** oder der Kachel eines Kachelobjekts: der Kollider dieses Objekts.
- Der Observer für `ColliderCreated` in `main.rs` hängt an solche Kollider die Komponente
  `sight::BlocksSight`.
- Wasser blockiert also nur die Bewegung. Auf der Karte liegen die drei Baumgruppen auf einer
  eigenen Ebene „Bäume“ mit `blocks_sight`.
- Ebenen liegen 2 in z auseinander (`TiledMapLayerZOffset`, Standard wäre 100). So bleiben obere
  Ebenen unter Items und Einheiten, und das y-Sortieren der Chunks (bis zu 1,5) mischt keine
  Ebenen.

## Sichtmaske

- Ein Zellraster über die Karte (4 px wie beim Flowfield). Eine Zelle ist undurchsichtig, wenn ihr
  Mittelpunkt in der Form eines Kolliders mit `BlocksSight` liegt. Neu berechnet wird das nur, wenn
  sich diese Kollider ändern.
- Die Maske deckt ein quadratisches Fenster um die Zelle des Spielers ab, so groß wie der
  Sichtradius (`SIGHT_RADIUS`, 200 px), plus ein Rand mit 0. Neu berechnet wird sie nur, wenn der
  Spieler die Zelle wechselt.
- Wert pro Zelle (0–255):
  - 0, wenn der Strahl vom Spieler zur Zelle eine undurchsichtige Zelle kreuzt. Die
    undurchsichtige Zelle selbst ist sichtbar, so werden die Kanten der Blocker hell.
  - Sonst 255, auf den letzten `SIGHT_FADE` px vor dem Radius auslaufend bis 0.
- Berechnet wird das mit rekursivem Shadowcasting: Je Achtel um den Spieler werden die Zellen
  Reihe für Reihe nach außen durchgegangen, und hinter undurchsichtigen Zellen geht es nur im
  verbleibenden Winkelbereich weiter. Das kostet einen Schritt pro sichtbarer Zelle, und die
  Schattenkanten behalten ihren genauen Winkel.
- Ein erster Ansatz hat jede Zelle von ihrer Nachbarzelle einen Schritt näher zum Spieler erben
  lassen. Dabei driften die Ketten durch das Runden auf die Achsen und Diagonalen, und die
  Schattenkanten rasten auf 0°, 45° und 90° ein.

## Darstellung

- Die Maske wird in eine Textur geschrieben (ein Pixel pro Zelle, `R8Unorm`, lineare Filterung).
- Der Shader liest sie an 16 Punkten in einer Kreisscheibe mit dem Radius `BLUR` (6 px) und mittelt
  sie, sodass Schattenkanten und angeschnittene Gegner weich auslaufen.
- Das Material `SightMaterial` (eigener WESL-Shader) rechnet aus der Weltposition eines Pixels die
  Stelle in der Maske aus:
  - **Gegner**: Körper und Punkt verwenden es statt `ColorMaterial`. Die Deckkraft wird mit der
    Maske multipliziert. Außerhalb der Sicht ist der Gegner unsichtbar, an einer Ecke nur zum Teil.
  - **Abdunklung**: ein großes Rechteck um den Spieler zwischen Items (z = 5) und Einheiten
    (z = 10), schwarz mit der Deckkraft `DARKNESS × (1 − Maske)`.
- Der Spieler und die Karte verwenden es nicht. Die Karte wird nur durch die Abdunklung verdeckt.
- Lebensbalken sind Sprites und lassen sich so nicht maskieren. Sie werden versteckt, solange die
  Mitte des Gegners nicht sichtbar ist.

## Spiellogik

- Unsichtbare Gegner bleiben aktiv: Sie laufen, greifen an und lassen sich treffen. Die Sicht ist
  vorerst rein optisch. Die Aktivierung bei Sicht (siehe [ideen.md](ideen.md)) baut später darauf
  auf (`Sight::visibility`).

## Code

- `src/sight/mod.rs`: `BlocksSight`, die Ressource `Sight` (Raster, Maske, Strahlen), Systeme zum
  Aktualisieren.
- `src/sight/render.rs`: Textur, `SightMaterial`, Abdunklung.
- `src/combat/health_bar.rs`: Lebensbalken von Gegnern nur in Sicht.
- `assets/shaders/sight.wesl`: Shader.
- `tests/sight.rs`: freies Feld, Schatten hinter einem Blocker und ihr Winkel, sichtbare Kante,
  Radius und Auslauf.
