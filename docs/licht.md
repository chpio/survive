# Konzept: Licht

Umgesetzt sind Tag und Nacht mit Uhr und Dev-Werkzeugen im HUD, die Werte-Datei
(`assets/light.ron`), der `SightRadius`, die Licht-Ebene mit Nacht, Sonnenschatten und leerer
Lightmap und das Color Grading (`src/light/`). Lichtquellen fehlen noch. Alle Zahlen sind Startwerte
fürs Ausbalancieren.

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
  - Zeitraffer: Der Knopf schaltet die Uhr durch ×10, ×60 und normal. Dafür liegt die Tageslänge
    selbst in `DayNight` (`day_length`), anfangs der Wert der Werte-Datei; ändert sich die Datei,
    gilt wieder ihr Wert. Bei ×60 ändert sich die Anzeige jeden Frame, das HUD malt dann ständig
    neu.
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

## Licht-Ebene

- Eigene Ebene über allem, auch über Items, Gegnern, Spieler und Lebensbalken (z = 20; Einheiten
  liegen bei 10, ihre Balken bis 11,1). Die Materialien der einzelnen Entities ändern sich nicht.
  Das HUD ist eine UI-Node und wird nicht abgedunkelt.
- Ein eigenes `LightMaterial` (Shader `assets/shaders/light.wesl`) mit einer Helligkeit pro Pixel:
  (1 − Dunkelheit der Nacht) × (1 − Schatten der Sonne) + Licht aus der Lightmap. Die Ebene ist
  schwarz mit der Deckkraft 1 − Helligkeit. Anfangs ist die Lightmap leer (ein dunkles Pixel).
- Tagsüber zeigt sie die Schatten, nachts die Dunkelheit. Unsichtbar ist sie nur, wenn es weder
  das eine noch das andere gibt (etwa mit `shadow_strength` 0 oder ausgeschalteten Schatten am
  Tag). Ohne Schatten liest der Shader weder die Schattenmaske noch die Sicht.
- Die Sichtabdunklung bleibt unverändert darunter. Die Schatten der Sonne zeigt die Licht-Ebene
  nur in der Sicht: Sie liest die Sichtmaske, die schon verwischt ist wie für die Sichtabdunklung
  (gemeinsamer Shader-Teil `assets/shaders/sight_mask.wesl`), und gewichtet den Schatten mit der
  Sichtbarkeit. Sonst lägen außerhalb der Sicht beide Abdunklungen übereinander, was seltsam
  aussah. Die Nacht dunkelt weiterhin die ganze Welt ab.
- Mesh-Picking stört die Ebene nicht, es berücksichtigt nur `Pickable`-Entities.

## Lichtquellen (später)

- Komponente an beliebigen Entities.
- Die Lightmap entsteht auf der CPU, auf dem Zellraster wie die Sichtmaske, über den sichtbaren
  Ausschnitt. Jedes Licht wird als weicher Kreis (etwa per `smoothstep`) aufaddiert. Das Ergebnis
  kommt in eine linear gefilterte Textur, die der Shader der Licht-Ebene liest.
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

## Sonne und Schatten

- Die Sonne ist ein gerichtetes Licht: Alle Schatten zeigen in dieselbe Richtung, ihre Länge hängt
  vom Sonnenstand und von der Höhe des Objekts ab. Das Licht zwischen Gebäuden ist alles, was
  nicht im Schatten liegt; berechnet werden nur die Schatten.
- Die Licht-Ebene drückt die Helligkeit im Sonnenschatten (`shadow_strength`, 30 %), nur in der
  Sicht (siehe Licht-Ebene). Da sie über allem liegt, werden Spieler und Gegner im Schatten von
  selbst dunkler.
- Die Schatten lassen sich in den Einstellungen ausschalten (`Settings::shadows`). Dann rechnet
  `SunShadow` nicht mehr, und die Licht-Ebene bekommt keinen Schatten. Später gilt der Schalter
  auch für die Schatten der Lichtquellen.
- Höhenraster pro Karte auf den 4-px-Zellen (`light::shadow::Heights`, Höhe in px, höchstens 255),
  aus der Tiled-Property `height` an Objekt, Tile oder Ebene wie `blocks_sight`. Die Bäume haben
  24 px. `ShadowHeights` setzt die Raster der geladenen Karten im kleinsten Rechteck um sie
  zusammen, neu nur, wenn Karten geladen, entladen oder neu gerastert werden. Ein Fenster um die
  Kamera wäre kleiner, müsste aber beim Laufen nachgeführt werden.
- Der Schatten entsteht pro Zelle auf der CPU (`SunShadow`): Jede Zelle mit Höhe markiert in
  Schritten von 2 px die Zellen von der Sonne weg, so weit wie ihre Höhe durch die Steigung der
  Sonne (höchstens 64 px, `MAX_SHADOW_LENGTH`). So ergibt sich die Schattenlänge pro Objekt von
  selbst, und die Kosten hängen nur an der Zahl der Schattenwerfer. Neu gerechnet wird erst, wenn
  die Spitze des längsten Schattens um 1 px gewandert ist. Der Shader liest die Maske als linear
  gefilterte Textur, an vier Punkten um jedes Pixel (±1,5 px) gemittelt, das macht die Kanten
  weich.
- Zuerst lief die Suche pro Pixel im Shader, Richtung Sonne über die Höhen. Das kostete bei 4K
  8–14 ms pro Frame, da eine Zelle dort Hunderte Pixel groß ist und alle dasselbe rechneten; jetzt
  kostet die Licht-Ebene etwa 0,5 ms.
- Schatten können bis 64 px in die Ansicht fallen, daher lädt `world::update_chunking` die Karten
  so weit über die Ansicht hinaus.
- Die Zellen der Schattenwerfer selbst bleiben hell, wie Hindernisse in der Sicht ihre Kante
  zeigen, sonst läge ein Dach im eigenen Schatten.
- Sonnenrichtung und -höhe leitet `DayNight` aus der Uhrzeit ab: morgens lange Schatten nach
  Westen, mittags (13 Uhr, `sun_elevation` 50°) kurze nach Norden, abends lange nach Osten. Die
  Sonne steht vom Beginn der Morgen- bis zum Ende der Abenddämmerung am Himmel, ihre Schatten
  blenden mit der Nacht weich aus und ein. Nachts keine Schatten; schwache vom Mond vielleicht
  später.
- Erst einmal werfen die Kollider den Schatten, mit ihrer Form wie beim Rastern der
  undurchsichtigen Zellen. Kollider sind aber oft nur Stamm oder Grundfläche, ein Baum soll
  eigentlich den Schatten seiner Krone werfen. Später daher eigene Schattenformen: Objekte in Tiled
  oder eine Höhe pro Tile. Das ist mehr Arbeit beim Kartenbau als im Code.
- Die 4-px-Zellen geben diagonalen Schatten Treppenkanten, die lineare Filterung und das Mitteln
  machen sie weich.
- Einheiten werfen keine Schatten über das Raster. Für sie besser kleine weiche Blob-Schatten als
  Sprite unter jeder Einheit.
- Später: Der Look kommt vor allem über die Farbe, sonnige Bereiche etwas warm, Schatten etwas
  kühl. Das kann der Shader der Licht-Ebene selbst, ohne Color Grading. Bisher dunkelt er nur
  schwarz ab.
- Optional: Lichtflecken unter Bäumen (eine langsam wandernde Rauschtextur) statt sichtbarer
  Lichtstrahlen, die man von oben kaum sieht. Ein eigener kleiner Effekt, unabhängig von den
  Schatten.

## Color Grading

- Bevys `ColorGrading` an der Kamera, mit einer Stimmung pro Phase in der Werte-Datei
  (`light::grading::Gradings`): Belichtung, Temperatur, Tint, Sättigung und Kontrast, gleich für
  Schatten, Mitteltöne und Lichter. Ausgelassene Werte ändern nichts. Temperatur und Tint
  verschieben den Weißpunkt in CIE-xy, schon wenige Hundertstel sind deutlich.
- Kontrast unter 1 vermeiden: Bevy dreht ihn um 0,5 in linearen Farben, Schwarz wird dabei grau
  (0,95 hebt es schon auf 44/255), was auf OLED-Displays auffällt. Die übrigen Werte lassen
  Schwarz schwarz. Eine flachere Nacht besser über die Sättigung.
- `DayNight::grading` blendet die Stimmungen von Tag und Nacht mit der Nacht über, und zur Mitte
  der Dämmerung hin (um `night_start` und am Ende der Nacht) in die der Abend- bzw.
  Morgendämmerung. Am Anfang und Ende der Dämmerung springt nichts.
- Ohne HDR macht jeder Shader die Farbanpassung selbst (`SightMaterial` tut es schon), der
  Tilemap-Shader von `bevy_ecs_tilemap` 0.20 aber nicht: Die Karte bliebe ungefärbt. Daher rendert
  die Kamera mit der Komponente `Hdr` in eine Zwischentextur (`Rgba16Float`), und der
  Tonemapping-Pass (`Tonemapping::Linear`, Bevys Vorgabe für 2D) färbt alles gemeinsam, auch die
  Licht-Ebene. Ausgegeben wird weiter SDR; HDR-Ausgabe kann Bevy nicht. Das Bevy-Feature `hdr`
  braucht es nicht, es lädt nur `.hdr`-Bilder.
- Das HUD zeichnet Bevy erst nach dem Tonemapping, es behält seine Farben. Überblendet wird mit
  HDR wie ohne linear, die Licht-Ebene sieht also gleich aus.
- Kosten: die größere Zwischentextur und ein Vollbild-Pass, bei 4K auf einer integrierten GPU
  etwa 1,3 ms. Zusammen mit Bevys 4×-MSAA der 2D-Kamera verdoppelte HDR die Frame-Zeit (13,3 statt
  6,8 ms). MSAA glättet nur die Kanten von Meshes und ist daher standardmäßig aus.
- Color Grading und MSAA sind Einstellungen (`Settings::color_grading`, `Settings::msaa`). Ohne
  Color Grading rendert die Kamera ohne `Hdr`, und `grade_colors` setzt ein neutrales
  `ColorGrading` (ohne HDR würden nur manche Shader färben, die Tilemap nicht).

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
5. Sonne mit Schatten der Kollider.
6. Lichtquellen.
7. Color Grading mit HDR (vor den Lichtquellen umgesetzt).

## Später

- Director nachts mit höherem Budget, Wellen in der Nacht ([spawnen.md](spawnen.md)).
- LUTs statt der Parameter von `ColorGrading`, für genauere Farbstimmungen pro Tageszeit. Bevy
  wendet keine eigenen LUTs an, das bräuchte einen eigenen Post-Processing-Pass, der zwischen zwei
  LUTs überblendet.
- Niedrige Priorität, machbar: Sonnenschatten auf der GPU statt auf der CPU. Eine zweite Kamera
  rendert vor der Hauptkamera ein Quad auf eigener Render-Ebene in eine kleine Textur, ein Texel
  pro Zelle über die geladenen Karten (etwa 350 × 275). Dort läuft die Suche Richtung Sonne über
  die Höhen pro Texel statt pro Bildschirmpixel, die Licht-Ebene liest das Ergebnis wie jetzt die
  Maske. Vorteile: Die Sonne wandert jeden Frame weich statt in 1-px-Schritten, die CPU rechnet
  nichts, die Kosten hängen nicht an der Zahl der Schattenwerfer, und Lichtquellen mit Schatten
  könnten denselben Weg nehmen. Nachteile: Alles, was genau eine `Camera2d` erwartet
  (`update_camera_position`, `world::update_chunking`, HUD, Einstellungen, `follow_camera`),
  braucht eine Markierung der Hauptkamera; Textur und Kamera müssen nachgeführt werden, wenn sich
  die geladenen Karten ändern; der Schatten ist dann nicht mehr per Test prüfbar. Lohnt sich bei
  viel mehr Schattenwerfern oder Lichtquellen mit Schatten.
