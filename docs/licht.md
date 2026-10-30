# Konzept: Licht

Umgesetzt sind Tag und Nacht mit Uhr und Dev-Werkzeugen im HUD, die Werte-Datei
(`assets/light.ron`), die Nacht-Ebene mit leerer Lightmap und der `SightRadius` (`src/light/`).
Lichtquellen, Sonne und Color Grading fehlen noch. Alle Zahlen sind Startwerte fürs Ausbalancieren.

## Ziel

- Tag und Nacht. Nachts ist es nicht komplett dunkel, die ganze Welt wird abgedunkelt und der
  Sichtradius kleiner.
- Später Lichtquellen (Lagerfeuer, Fackel), die die Nacht aufhellen.
- Tagsüber eine Sonne, deren Schatten von Gebäuden und Bäumen mit der Tageszeit wandern. Zwischen
  den Schatten liegt das Sonnenlicht.
- Zum Schluss Color Grading pro Tageszeit.

## Tag und Nacht

- Ressource `DayNight`: Tageszeit und Umgebungslicht. Ein System leitet alles aus der Uhrzeit ab.
- Die Uhr läuft mit der virtuellen Zeit (`Res<Time>` in `Update`), steht also still, solange das
  Menü offen ist.
- Ein Tag dauert 10 Minuten, davon ist ein Drittel Nacht.
- Weiche Übergänge in der Dämmerung, Helligkeit und Radius wandern langsam. Die Nacht beginnt um
  21 Uhr und endet um 5 Uhr, jeweils in der Mitte einer Dämmerung von 1,5 Stunden. Phasen: Tag,
  Abenddämmerung, Nacht, Morgendämmerung.
- Nachts halbe Helligkeit.
- Die Uhr kommt in die globalen Daten des Spielstands, sobald es Speichern gibt
  ([speichern.md](speichern.md)).

## Uhr im HUD

- Das HUD zeigt die Uhrzeit (Spielstunden und -minuten, eine Spielstunde sind 25 s) und ob Tag,
  Dämmerung oder Nacht ist. `hud::State` bekommt sie aus `DayNight`.
- Die Uhr ändert sich jede Spielminute, das HUD malt also etwa alle 0,4 s neu. Reicht das nicht,
  zeigt es nur volle Viertelstunden.
- Dev-Werkzeuge im Debug-Bereich des HUD, wie „Gegner spawnen“ und „Flowfield“:
  - Vorspulen um eine Spielstunde, auch per Taste N.
  - Zum Beginn der nächsten Phase springen.
  - Checkbox „Uhr anhalten“, um eine Tageszeit in Ruhe anzusehen.
- Die Werkzeuge schicken `hud::Action`s, die `DayNight` direkt ändern. Alles davon Abgeleitete
  (Helligkeit, Sichtradius) folgt im selben Frame. Beim Vorspulen springen Helligkeit und Radius
  daher, statt langsam zu wandern.

## Sichtradius

- Komponente `SightRadius` am Spieler statt der Konstanten `SIGHT_RADIUS`.
- Zwei Werte wie bei `Transform` und `GlobalTransform`: Grundwert und fertiger Wert. Das
  Tag-Nacht-System setzt den Grundwert (200 px am Tag, 120 px nachts). Ausrüstung wie Fackel oder
  Trank rechnet später Zuschläge drauf. `Sight` liest nur den fertigen Wert.
- Das Fenster der Sicht hat eine feste Größe, von der die Textur der Maske, `sight::REACH` und
  damit `world::MIN_LOADED_DISTANCE` abhängen. Es richtet sich nach einer neuen Konstanten
  `MAX_SIGHT_RADIUS`, auf die der fertige Wert begrenzt wird, auch die Werte aus der Werte-Datei.
  Sie bleibt deutlich unter `enemy::DEACTIVATION_RADIUS` (400 px), sonst wachen Gegner auf und
  schlafen sofort wieder ein.
- Das Shadowcasting merkt sich, welche Zellen es erreicht, und rechnet wie bisher nur beim
  Zellwechsel oder bei geänderten Karten neu. Ändert sich der Radius, wird nur die Maske aus diesen
  Zellen und der Abfall-Tabelle (`fade`) für den neuen Radius neu zusammengesetzt. In der
  Dämmerung ändert sich der Radius jeden Frame, das bleibt so billig.
- Flowfield- und Deaktivierungs-Radius bleiben Konstanten. Spawnen außer Sicht profitiert nachts
  von selbst, weil mehr Zellen unsichtbar sind. Mit 120 px Sichtweite liegt
  `out_of_sight::MIN_DISTANCE` (120 px) allerdings am Rand der Sicht, Gegner tauchen nachts also
  deutlich näher auf. Das beim Ausbalancieren prüfen.

## Nacht-Ebene

- Eigene Ebene über allem, auch über Items, Gegnern, Spieler und Lebensbalken (z = 20; Einheiten
  liegen bei 10, ihre Balken bis 11,1). Die Materialien der einzelnen Entities ändern sich nicht.
  Das HUD ist eine UI-Node und wird nicht abgedunkelt.
- Sie ist ein `SightMaterial` wie die Sichtabdunklung: invertiert, Schwarz mit der Deckkraft der
  Nacht, aber mit der Lightmap statt der Sichtmaske als Textur. Anfangs ist die Lightmap leer, die
  Ebene dunkelt dann überall gleich stark ab.
- `sight::render::write_mask` setzt den Bereich der Maske nur in den Materialien der Sichtmaske,
  die Nacht-Ebene hat ihren eigenen.
- Deckkraft etwa Nacht × (1 − Licht).
- Die Sichtabdunklung bleibt unverändert, beide Ebenen liegen übereinander.
- Mesh-Picking stört die Ebene nicht, es berücksichtigt nur `Pickable`-Entities.

## Lichtquellen (später)

- Komponente an beliebigen Entities.
- Die Lightmap entsteht auf der CPU, auf dem Zellraster wie die Sichtmaske, über den sichtbaren
  Ausschnitt. Jedes Licht wird als weicher Kreis (etwa per `smoothstep`) aufaddiert. Das Ergebnis
  kommt in eine linear gefilterte Textur, die der Shader wie die Sichtmaske weichzeichnet.
  Bei Zoom 2 sind das etwa 250 × 250 Zellen, notfalls mit gröberen Zellen.
- Additiv auf der GPU zu zeichnen wäre aufwendiger: `AlphaMode2d` kennt kein additives Blending,
  es bräuchte eine eigene Pipeline und eine zweite Kamera, die in eine Textur rendert.
- Anfangs ohne Schatten, Licht scheint durch Wände.
- Schatten nur für wenige große Lichter: dasselbe Shadowcasting wie bei der Sicht, mit den
  vorhandenen undurchsichtigen Zellen der Karten. Eine eigene Maske pro Licht als Komponente am
  Licht (100 px Radius ≈ 2,5 KB). Sie liegt auf demselben Raster und wird beim Aufaddieren mit dem
  Kreis multipliziert.
- Feste Lichter berechnen ihre Maske einmal beim Laden. Neu nur bei geänderten Kollidern in der
  Nähe oder wenn eine Nachbarkarte geladen wird.
- Bewegte Lichter rechnen bei jedem Zellwechsel neu, wie die Sicht.
- Optimierung, falls nötig: feste Lichter einer Karte zu einer Lightmap pro Karte zusammenfassen.
  Der Shader bleibt gleich.
- Lichter können später den Sichtradius vergrößern.

## Sonne und Schatten (später)

- Die Sonne ist ein gerichtetes Licht: Alle Schatten zeigen in dieselbe Richtung, ihre Länge hängt
  vom Sonnenstand und von der Höhe des Objekts ab. Das Licht zwischen Gebäuden ist alles, was
  nicht im Schatten liegt; berechnet werden nur die Schatten.
- Dafür wird die Nacht-Ebene zur Licht-Ebene mit einer Helligkeit pro Pixel: Umgebungslicht aus
  `DayNight` (tagsüber 1, nachts 0,5), im Sonnenschatten gedrückt (etwa auf 70 %), plus die
  Lichtquellen. Die Ebene ist dann auch tagsüber sichtbar. Da sie über allem liegt, werden Spieler
  und Gegner im Schatten von selbst dunkler. Diese Umstellung kommt zuerst, danach lassen sich
  Sonne und Lichtquellen unabhängig voneinander einbauen.
- Höhenraster pro Karte auf den 4-px-Zellen (z. B. Baum 1, Haus 2), aus einer Tiled-Property
  `height` an Ebene, Objekt oder Tile wie `blocks_sight`. Wie die Sichtmaske wird es zu einem
  Fenster um die Kamera zusammengesetzt und als Textur hochgeladen, neu nur, wenn sich Karten
  ändern.
- Der Schatten entsteht im Shader: Von jedem Pixel aus geht er ein paar Schritte Richtung Sonne.
  Ist eine Zelle höher als der Sonnenstrahl an dieser Stelle (Abstand × Steigung der Sonne), liegt
  das Pixel im Schatten. So ergibt sich die Schattenlänge pro Objekt von selbst. Pro Frame ändern
  sich nur Richtung und Höhe der Sonne als Uniform, die Sonne wandert also ohne Mehrkosten. Auf der
  CPU müsste der Schatten bei jeder Winkeländerung neu berechnet werden.
- Die Zellen der Schattenwerfer selbst bleiben hell, wie Hindernisse in der Sicht ihre Kante
  zeigen, sonst läge ein Dach im eigenen Schatten.
- Sonnenrichtung und -höhe leitet `DayNight` aus der Uhrzeit ab: morgens lange Schatten nach
  Westen, mittags kurze, abends lange nach Osten. Nachts keine oder schwache Schatten vom Mond.
- Erst einmal werfen die Kollider den Schatten, mit ihrer Form wie beim Rastern der
  undurchsichtigen Zellen. Kollider sind aber oft nur Stamm oder Grundfläche, ein Baum soll
  eigentlich den Schatten seiner Krone werfen. Später daher eigene Schattenformen: Objekte in Tiled
  oder eine Höhe pro Tile. Das ist mehr Arbeit beim Kartenbau als im Code.
- Die 4-px-Zellen geben diagonalen Schatten Treppenkanten. Das Weichzeichnen wie bei der Sicht
  glättet sie, harte Schatten gibt es damit nicht.
- Einheiten werfen keine Schatten über das Raster. Für sie besser kleine weiche Blob-Schatten als
  Sprite unter jeder Einheit.
- Der Look kommt vor allem über die Farbe: sonnige Bereiche etwas warm, Schatten etwas kühl. Das
  kann der Shader der Licht-Ebene selbst, ohne Color Grading.
- Optional: Lichtflecken unter Bäumen (eine langsam wandernde Rauschtextur) statt sichtbarer
  Lichtstrahlen, die man von oben kaum sieht. Ein eigener kleiner Effekt, unabhängig von den
  Schatten.

## Color Grading

- Bevys `ColorGrading` an der Kamera, mit Parametern (Belichtung, Temperatur, Sättigung, Kontrast
  usw.) pro Tageszeit, zwischen denen überblendet wird. Die Werte kommen in die Werte-Datei.
- An der 2D-Kamera wirkt es, weil Bevy 0.20 ihr `Tonemapping::Linear` gibt. Ohne HDR macht aber
  jeder Shader die Farbanpassung selbst (`SightMaterial` tut es schon), und der Tilemap-Shader von
  `bevy_ecs_tilemap` 0.20 tut es nicht: Die Karte bliebe ungefärbt. Daher das Bevy-Feature `hdr`
  einschalten und die Kamera auf HDR stellen, dann färbt der Tonemapping-Pass alles gemeinsam.
  Vorher prüfen, ob das HUD dabei mitgefärbt wird und was HDR kostet.

## Werte

- Alle Werte (Tageslänge, Nachtanteil, Helligkeit, Sichtradien, Sonne, Color Grading) in einer
  eigenen RON-Datei unter `assets/`, mit Hot-Reload wie `items.ron` (eigener `AssetLoader`, Kopie
  in eine Ressource, die Tests direkt einsetzen können). So lassen sie sich im laufenden Spiel
  ausbalancieren.

## Reihenfolge

1. `DayNight` mit Uhr, Werte-Datei, Uhr und Dev-Werkzeuge im HUD.
2. Nacht-Ebene mit leerer Lightmap.
3. `SightRadius` am Spieler.
4. Licht-Ebene mit Helligkeit pro Pixel statt Nacht-Ebene.
5. Lichtquellen und Sonne, unabhängig voneinander.
6. Color Grading mit HDR.

## Später

- Director nachts mit höherem Budget, Wellen in der Nacht ([spawnen.md](spawnen.md)).
- LUTs statt der Parameter von `ColorGrading`, für genauere Farbstimmungen pro Tageszeit. Bevy
  wendet keine eigenen LUTs an, das bräuchte einen eigenen Post-Processing-Pass, der zwischen zwei
  LUTs überblendet.
