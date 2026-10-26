# Konzept: Mausereignisse zwischen HUD und Spielwelt

## Ziel

Für jede Mausposition gibt es genau eine Entscheidung: Entweder bekommt das HUD den Zeiger oder die
Spielwelt. Liegt ein HUD-Fenster über einem Item, erreicht der Klick das Item nicht. Über leeren
Stellen des HUDs geht er an die Welt.

## Ausgangslage

- `ui::forward_input` gibt alle Mausereignisse (`CursorMoved`, `MouseButtonInput`) an das
  Blitz-Dokument weiter. Blitz macht Hover, Klicks und Checkboxen selbst.
- Bevy Picking ist aktiv (Features `sprite_picking` und `ui_picking`). Das HUD ist ein
  bildschirmfüllender `ImageNode`, den Bevys UI-Picking überall trifft. Er blockiert damit bisher
  jedes Picking darunter.
- Spieler und Gegner sind `Mesh2d` und haben noch kein Picking (Mesh-Picking ist aus). Picking an
  der Welt kommt erst mit Sprites und Items.

## Ansatz: eigenes Picking-Backend für das HUD

Die Entscheidung trifft Bevy Picking. Das HUD meldet dafür als eigenes Backend seine Treffer, und
zwar über den Treffern der Sprites. Die Spielwelt muss vom HUD nichts wissen.

Bevy Picking liest die Maus selbst aus den Fensterereignissen. Was `forward_input` an Blitz
weitergibt, hält es nicht auf. Damit die Welt über dem HUD nichts bekommt, muss das HUD als Treffer
gemeldet werden, und ein Treffer nennt immer eine Entity. Diese Entity ist der `ImageNode` des HUDs
mit der Markierung `ui::Hud`.

1. **Blitz bekommt weiterhin alles.** `forward_input` bleibt unverändert, damit Hover-Effekte,
   Loslassen außerhalb eines Buttons usw. im HUD stimmen. Das gilt auch über der Welt, denn Blitz
   muss merken, wenn die Maus einen Button verlässt. Berührungen gehen als eigene Zeiger mit
   Finger-ID an Blitz. Für Stifte hat winit 0.30 keine Schnittstelle: Unter X11 (XWayland) kommen
   sie als Maus an, unter Wayland gar nicht (siehe `TODO.md`).
2. **Kein Bevy-UI-Picking.** Das Feature `ui_picking` ist aus. Sonst würde Bevys UI-Picking den
   bildschirmfüllenden `ImageNode` überall treffen. Bevys UI dient nur noch dem `ImageNode` und
   dem FPS-Overlay, und beide brauchen kein Picking.
3. **Hit-Test in Blitz.** Das System `pick_hud` läuft in `PreUpdate` im Set
   `PickingSystems::Backend`. Pro Zeiger fragt es `elements_from_point` an dessen Position ab,
   also das oberste Element und seine Vorfahren. Hat eines davon die Klasse `blocks-pointer`,
   schreibt es einen `PointerHits` für den `ImageNode`.
4. **Blockieren.** Der `ImageNode` hat kein `Pickable` und blockiert damit wie jede Entity ohne
   `Pickable` alle Treffer darunter.
5. **Reihenfolge.** Die Treffer bekommen die Order `camera.order + 1`. Das liegt über den Sprites
   (`camera.order`), weil das HUD über der Welt gezeichnet wird.

Die Prüfung selbst ist eine reine Funktion `ui::blocks_pointer(doc, position)` und wird in
`tests/` mit einem kleinen Dioxus-Dokument getestet.

## Was blockiert

- Nur Elemente mit der Klasse `blocks-pointer` und ihre Kinder. Wurzel, `body` und
  Layout-Container blockieren nicht.
- `.hud` und `.menu` bekommen die Klasse. Das Menü überdeckt den ganzen Bildschirm. Solange es
  offen ist, erreicht also kein Klick die Welt, passend dazu, dass das Spiel pausiert ist.
- Später bekommen ganze Fenster die Klasse (Inventar, Truhe), samt ihrer Ränder und Lücken.

Verworfen wurde die Variante „jedes Element außer Wurzel und `body` blockiert“. Ein
bildschirmfüllender Container zum Positionieren würde sonst unbemerkt die ganze Welt sperren. Die
Klasse macht das Blockieren zu einer bewussten Entscheidung. Elemente mit `pointer-events: none`
übergeht Blitz beim Hit-Test ohnehin.

## Koordinaten

- Bevys Zeigerposition ist in logischen Pixeln, und das sind genau die CSS-Pixel von Blitz: Der
  Viewport bekommt den HiDPI-Faktor des Fensters, und das Dokument wird nie gescrollt.
- Bei Vollbild und Größenänderung folgen beide demselben Fenster. Das Layout passt `paint_ui` an
  die neue Größe an.
- Der Hit-Test nutzt das Layout des zuletzt gezeichneten HUDs, also genau das, was zu sehen ist.

## Nutzung in der Spielwelt

- Objekte in der Welt (Items, Truhen) bekommen Observer für `PointerClick` und
  `PointerOver`/`PointerOut`. Über dem HUD lösen sie nicht aus.
- Eingaben ohne Ziel-Entity, z. B. ein Angriff per Klick auf eine Position, lesen nicht
  `ButtonInput<MouseButton>` direkt. Sie prüfen vorher, ob `ui::Hud` in der `HoverMap` liegt.
  Dafür kommt eine Run-Condition `ui::pointer_over_hud` dazu, sobald es so eine Eingabe gibt.

## Offen

- Das Mausrad wird noch nicht an Blitz weitergegeben. Kommt Scrollen im HUD (Inventarliste) oder
  Zoom per Rad dazu, gilt dieselbe Entscheidung.
- Tastatur: Sobald das HUD Textfelder hat, muss das Spiel Tasten ignorieren, solange ein
  HUD-Element den Fokus hat. Das geht analog über eine Abfrage an Blitz.
- Drag & Drop zwischen HUD und Welt (Item aus dem Inventar fallen lassen) ist damit noch nicht
  abgedeckt. Es braucht eine eigene Regel, wer den Zeiger während des Ziehens besitzt.
