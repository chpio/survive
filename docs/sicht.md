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

- Ein Zellraster pro Karte (4 px wie beim Flowfield, siehe [welt.md](welt.md)). Eine Zelle ist
  undurchsichtig, wenn ihr Mittelpunkt in der Form eines Kolliders mit `BlocksSight` liegt. Neu
  berechnet wird das nur, wenn sich diese Kollider ändern. Das Fenster der Maske setzt seine
  undurchsichtigen Zellen aus den geladenen Karten zusammen.
- Die Maske deckt ein quadratisches Fenster um die Zelle des Spielers ab, so groß wie der größte
  Sichtradius (`MAX_SIGHT_RADIUS`, 300 px), plus ein Rand mit 0. Der Radius selbst ist die
  Komponente `SightRadius` am Spieler (200 px, nachts weniger, siehe [licht.md](licht.md)). Welche
  Zellen sichtbar sind, wird nur neu berechnet, wenn der Spieler die Zelle wechselt. Ändert sich
  nur der Radius, wird die Maske aus diesen Zellen neu zusammengesetzt.
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
- Vorher wird sie auf der CPU verwischt (`sight::render::blur`), sodass Schattenkanten und
  angeschnittene Gegner weich auslaufen: Jede Zelle bekommt den Mittelwert der linear gefilterten
  Maske an 16 Punkten in einer Kreisscheibe mit dem Radius `BLUR` (6 px) um ihre Mitte, als
  fester Kern über 5×5 Zellen. Der Shader liest die Textur dann einmal pro Pixel. Anfangs las er
  die 16 Punkte pro Pixel selbst; das kostete bei 4K etwa 1,8 ms pro Ebene, die die Maske liest
  (Sichtabdunklung und Licht-Ebene), und sah nicht anders aus (höchstens 5/255 Unterschied).
- Das Material `SightMaterial` (eigener WESL-Shader) rechnet aus der Weltposition eines Pixels die
  Stelle in der Maske aus:
  - **Gegner**: Körper und Punkt verwenden es statt `ColorMaterial`. Die Deckkraft wird mit der
    Maske multipliziert. Außerhalb der Sicht ist der Gegner unsichtbar, an einer Ecke nur zum Teil.
  - **Items am Boden**: ebenso, als Quadrat in der Platzhalterfarbe ihres Typs (`items::render`).
    Ein Klick hebt ein Item nur auf, wenn ein Teil davon zu sehen ist (`Sight::sees`), da das
    Picking auch den ausgeblendeten Teil trifft.
  - **Abdunklung**: ein großes Rechteck um den Spieler über der Karte, unter Items (z = 9) und
    Einheiten (z = 10), schwarz mit der Deckkraft `DARKNESS × (1 − Maske)`.
- Neue Materialien bekommen den aktuellen Bereich der Maske aus `SightMask` mit, sonst wären sie
  unsichtbar, bis sich die Sicht das nächste Mal ändert.
- Der Spieler und die Karte verwenden es nicht. Die Karte wird nur durch die Abdunklung verdeckt.
- Außerhalb der Sicht wird nur die Karte gezeichnet. Lebensbalken sind Sprites und lassen sich
  nicht maskieren. Sie tragen deshalb `render::HiddenOutsideSight` und werden als Ganzes versteckt,
  solange `Sight::shows` ihre Position nicht mindestens zur Hälfte sichtbar sieht. Ein Balken hängt
  dafür an einem Halter in der Mitte seiner Einheit, der ausgeblendet wird; die eigene
  Sichtbarkeit des Balkens hängt nur am Schaden.

## Spiellogik

- Wache Gegner bleiben auch unsichtbar aktiv: Sie laufen, greifen an und lassen sich treffen.
- Schlafende Gegner wachen auf, sobald der Spieler einen Teil von ihnen sieht
  (`Sight::sees`, siehe „Schlafen und Aufwachen“ in [spawnen.md](spawnen.md)).

## Code

- `src/sight/mod.rs`: `BlocksSight`, die Ressource `Sight` (Raster, Maske, Strahlen), Systeme zum
  Aktualisieren.
- `src/sight/render.rs`: Textur, `SightMaterial`, Abdunklung.
- `src/combat/health_bar.rs`: Lebensbalken, über ihren Halter nur in Sicht.
- `assets/shaders/sight.wesl`: Shader.
- `tests/sight.rs`: freies Feld, Schatten hinter einem Blocker und ihr Winkel, sichtbare Kante,
  Radius und Auslauf.
