# Konzept: einfaches Item-System

## Umfang der ersten Version

- Items besitzen: aufheben, im Inventar halten und stapeln, fallenlassen.
- Items entstehen per Debug-Taste. Gegner-Drops, Truhen und Platzierung in Tiled kommen später.
- Aufheben per Mausklick auf das Item, mit Reichweitenprüfung.
- Inventarfenster im HUD, per Taste geöffnet.
- Benutzen von Items mit einer Wirkung (Heiltrank, Apfel heilen), nachträglich dazugekommen.

Noch nicht dabei: Ausrüstung, Stufen, Gewicht als Masse, Haltbarkeit, Speichern. Das
Modell ist so angelegt, dass diese Punkte ohne Umbau dazukommen (siehe [ideen.md](ideen/ideen.md)).

## Item-Definitionen

- Die Grundtypen stehen zentral in `assets/items.ron`, als Map von der ID auf die Definition:

  ```ron
  {
      "healing_potion": (name: "Heiltrank", weight: 0.5, color: (0.8, 0.2, 0.2),
          effect: Some(Heal(30.))),
      "apple": (name: "Apfel", weight: 0.2, color: (0.3, 0.8, 0.2), effect: Some(Heal(5.))),
      "sword": (name: "Schwert", weight: 3.0, color: (0.7, 0.7, 0.8)),
  }
  ```

- Die Datei wird als Asset geladen und wie das Stylesheet hot-reloaded. Bei jeder Änderung kopiert
  ein System sie in die Ressource `ItemDefinitions`. Alle anderen Systeme lesen nur die Ressource.
  Tests setzen sie dadurch direkt, ohne Asset-System.
- `color` ist der Platzhalter, bis es Sprites gibt. `weight` wird im Inventar angezeigt und wirkt
  später auf die Masse.
- `effect` ist optional und sagt, was das Benutzen eines Items bewirkt (`Effect`). Ohne `effect`
  lässt sich ein Item nicht benutzen. Bisher gibt es nur `Heal(Lebenspunkte)`.

## Items als Entities

- Jedes Item ist eine eigene Entity mit `Item(ItemId)`. Die ID verweist auf die Definition.
- Jedes Item hat `Stack(u32)` mit der Anzahl (von `Item` verlangt, sonst 1). Ob Items stapeln, ist
  keine Eigenschaft des Typs, sondern folgt aus einer Regel: Gleicher Typ stapelt immer, außer
  einzigartige Items. Einzigartig ist ein Exemplar mit eigenem Zustand (später Haltbarkeit,
  Verzauberung oder ein Story-Einzelstück), markiert mit `Unique`. Bisher setzt das noch nichts.
- Die Anzahl zeigt das HUD nur ab 2, ein einzelnes Item hat keine Zahl.
- **Am Boden:** `Sprite` (ein Quadrat in der Farbe der Definition, 8 × 8 px) und `Transform`
  zwischen Karte (z = 0) und Spieler (z = 10), dazu `Pickable`. Ohne `Pickable` ignoriert Bevys
  Sprite-Picking ein Sprite.
- **Im Besitz:** Die Relationship `HeldBy(owner)` hängt am Item, Bevy pflegt dazu `Inventory` am
  Besitzer (Liste der Items). Ein Item im Besitz ist unsichtbar
  (`Visibility::Hidden`) und hat kein `Pickable`. Das `Sprite` bleibt dran: Bevy zeichnet Sprites
  als Meshes, die es beim Hinzufügen des Sprites mit anlegt und die beim Entfernen bleiben würden.
- Ob ein Item am Boden liegt, ergibt sich also aus `Without<HeldBy>`.
- Besitzer kann jede Entity sein, in dieser Version nur der Spieler. Später kommen Gegner und
  Truhen dazu.

## Aufheben

- Ein globaler Observer für `PointerClick` (linke Taste) auf ein Item am Boden prüft die
  Reichweite. Liegt das Item höchstens `PICKUP_REACH` (24 px) vom Spieler entfernt, löst er
  `PickUp { item, by }` aus. Sonst passiert nichts.
- Dank des HUD-Pickings ([ui-events.md](ui-events.md)) kommt ein Klick durch ein HUD-Fenster nicht
  beim Item an.
- `PickUp` versteckt das Item, entfernt `Pickable` und hängt es per `HeldBy` an den Besitzer.
  - Hat der Besitzer schon einen Stapel derselben ID und ist keins der beiden `Unique`, wird die
    Anzahl dort addiert und das aufgehobene Item despawnt.
- Hervorhebung: Bei `PointerOver` wird das Item heller, bei `PointerOut` wieder normal.
  Einen Tooltip mit Namen gibt es noch nicht.

## Fallenlassen

- `DropItem { item, count }` legt Items neben den Besitzer, z. B. 12 px unter ihn.
  - Ist `count` kleiner als der Stapel, wird ein neues Item mit `count` abgespalten und abgelegt.
    Der Rest bleibt im Inventar.
  - Sonst wird das Item selbst abgelegt: `HeldBy` entfernen, `Sprite` (in der normalen Farbe),
    `Transform`, Sichtbarkeit und `Pickable` setzen.
- Am Boden wird automatisch gestapelt: Kommt ein Item auf den Boden (abgelegt oder gespawnt, beides
  setzt `Pickable`), geht es im nächstgelegenen Stapel seines Typs innerhalb von `MERGE_RADIUS`
  (12 px, so weit wie das Ablegen) auf. Mehrmals Ablegen landet so auf einem Stapel unter dem
  Spieler. Items bewegen sich am Boden nicht, deshalb wird nur in diesem Moment geprüft, ohne
  Physik und Kollider.

## Benutzen

- `UseItem { item }` benutzt ein Item im Besitz. Hat sein Typ eine Wirkung, wird es verbraucht:
  Ein Stapel verliert eins, das letzte Item wird despawnt. Dann meldet `ItemUsed { by, effect }`
  die Wirkung für den Besitzer. Items ohne Wirkung und Items am Boden bleiben unverändert.
- Das Item-Modul wendet Wirkungen nicht selbst an. Das tun die Module, die sie betreffen: `combat`
  heilt bei `Effect::Heal` bis höchstens zum Maximum. Neue Wirkungen sind eine neue Variante von
  `Effect` und ein System in dem Modul, das sie umsetzt.
- Ein Item wird auch bei voller Gesundheit verbraucht.

## Inventarfenster

- Tab öffnet und schließt das Fenster. Escape schließt zuerst das Popup der Auswahl, dann das
  Fenster; erst danach öffnet Escape das Einstellungsmenü. Es steht direkt über der Leiste, unten mittig. Das
  Spiel läuft dabei weiter.
- Das HUD bekommt im `hud::State` die Items des Spielers: Entity, Typ, Name, Farbe, Anzahl,
  Gewicht und ob sie benutzbar sind. Dazu kommt das Gesamtgewicht.
- Die Items stehen als Kacheln in einem Raster mit sechs Spalten, das mit ihnen wächst. Es gibt
  kein Slot-Limit, die Grenze ist das Gewicht. Sortiert wird automatisch: benutzbare Items zuerst,
  dann nach Name. Manuelles Sortieren gibt es nicht, ohne Abbauen und Bauen gibt es wenige
  Item-Arten.
- Ein Klick auf eine Kachel wählt sie aus. Ein Popup über der Kachel zeigt Name, Anzahl und
  Gewicht mit den Buttons „Benutzen“ (bei Items mit Wirkung), „1 ablegen“ (bei Stapeln) und „Ablegen“. Sie
  schicken `hud::Action::UseItem(item)` bzw. `hud::Action::DropItem { item, count }`, das Spiel
  schreibt daraus `UseItem` bzw. `DropItem`. Als Popup verändert es die Größe des Fensters
  nicht, sodass die Kacheln an ihrem Platz bleiben.
- Ein Rechtsklick auf eine Kachel benutzt das Item direkt. Bei offenem Inventar ist das der
  einzige Weg zu benutzen, die Leiste belegt dann nur.
- Auswahl und die Kachel unter dem Zeiger kennt das Spiel (`hud::Action::Select`,
  `hud::Action::Hover`), weil die Tasten der Leiste beim Spiel ankommen, nicht beim HUD.
- Das Fenster hat die Klasse `blocks-pointer`. Klicks darauf erreichen die Welt nicht.

## Leiste

- Fünf Slots am unteren Bildschirmrand, immer sichtbar, Tasten 1–5 (`items::hotbar`).
- Ein Slot hält einen Item-Typ, kein einzelnes Item: `Hotbar([Option<ItemId>; SLOTS])` am Spieler.
  Er zeigt, wie viele Items dieses Typs der Spieler hat, und behält den Typ auch bei 0 (dann
  abgeblendet), damit der Trank-Slot nach dem letzten Trank nicht verschwindet.
- Nur benutzbare Typen (mit `effect`) kommen in die Leiste.
- Fährt der Zeiger über eine Kachel oder einen Slot, wird der Item-Typ überall hervorgehoben, im
  Inventar und in der Leiste. Das hält das HUD selbst, das Spiel braucht es nicht.
- Im Spiel (Inventar zu) benutzen Taste oder Klick auf den Slot ihn: `UseSlot { owner, slot }`,
  daraus wird `UseItem` für das erste gehaltene Item des Typs.
- Bei offenem Inventar werden Slots bedient wie Kacheln (eine gemeinsame Funktion `cell` im HUD,
  `hud::Target` für beide): Rechtsklick benutzt, Hover hebt den Typ hervor.
- Klicken und Ziehen wirken exakt gleich. Ein Klick nimmt eine Kachel oder einen Slot auf (er ist
  dann ausgewählt und zeigt das Popup), der nächste Klick legt sie dort ab, wo er hinklickt, wie
  das Loslassen beim Ziehen. Beides landet in `ui::put_down`:
  - auf einem anderen Slot: Der Typ kommt dorthin, sofern er benutzbar ist. Stand er schon in
    einem Slot, tauschen die beiden (`AssignSlot`: Ein Typ steht nur in einem Slot, sein alter
    Slot bekommt den bisherigen Inhalt des Ziel-Slots).
  - auf einer Kachel oder außerhalb von Kacheln und Slots: Ein Slot wird geleert, eine Kachel
    bleibt, wie sie ist.
  - auf sich selbst: nichts. Die Auswahl endet trotzdem.
- Der Typ, der gerade bewegt wird (ausgewählt oder gezogen), ist überall markiert, im Inventar und
  in der Leiste, wie beim Hover. Ist er benutzbar, sind dazu die Slots hervorgehoben.
- Die Tasten 1–5 legen bei offenem Inventar die Kachel oder den Slot unter dem Zeiger auf ihren
  Slot (wie bei Minecraft), sonst die Auswahl.
- Das HUD schickt nur `hud::Action::Click`, `hud::Action::Drop` und `hud::Action::Hover`. Was
  daraus wird, entscheidet `ui::sync_state` an einer Stelle.
- Blitz kennt kein Drag & Drop. Das HUD baut es aus Pointer-Events: Drücken auf Kachel oder Slot
  merkt sich den Typ, erst eine Bewegung über 4 px macht daraus ein Ziehen (sonst ist es ein
  Klick), Loslassen schickt `Drop` mit dem Feld darunter. Ein Element `.root` über dem ganzen Fenster, ohne
  `blocks-pointer`, bekommt die Bewegungen und das Loslassen überall. Das gezogene Symbol sitzt
  neben dem Zeiger, damit es nicht unter ihm liegt und das Loslassen abfängt.

## Debug-Taste

- F spawnt ein Item an der Mausposition in der Welt. Ist die Maus außerhalb des Fensters, liegt es
  neben dem Spieler.
- Jeder Druck nimmt die nächste ID aus den Definitionen (alphabetisch, reihum).
- Noch keine Definitionen geladen: Die Taste tut nichts.

## Code

Drei Plugins, damit die Tests ohne Asset-System und Eingabe auskommen:

- `src/items/mod.rs`: `Item`, `Stack`, `HeldBy`/`Inventory`, `ItemDefinitions`, `Effect`, die
  Messages `PickUp`, `DropItem`, `UseItem` und `ItemUsed` und die Systeme dazu (`items::Plugin`).
- `src/combat/mod.rs`: Heilen bei `ItemUsed`.
- `src/items/definitions.rs`: Asset-Loader für `items.ron` und Übernahme in die Ressource
  (`items::definitions::Plugin`).
- `src/items/input.rs`: Aufheben per Klick, Hervorhebung, Debug-Taste (`items::input::Plugin`).
- `src/items/hotbar.rs`: `Hotbar`, `AssignSlot`, `UseSlot` (in `items::Plugin`). Tasten 1–5 in
  `items::input`.
- `src/ui/hud.rs`: Inventarfenster, Leiste und das Ziehen. `src/ui/mod.rs`: Tab, Zustand
  und die Actions.
- `tests/items.rs`: Aufheben mit Stapeln, Ablegen eines ganzen Stapels, Abspalten beim Ablegen,
  einzigartige Items stapeln weder im Inventar noch am Boden, Stapeln am Boden (nur gleicher Typ
  im Radius), Benutzen (Stapel, letztes Item, ohne Wirkung,
  am Boden) und ob sich `assets/items.ron` lesen lässt, Leiste (Belegen, Verschieben, Leeren,
  nur benutzbare Typen, Benutzen über den Slot).
- `tests/combat.rs`: Heilen durch `ItemUsed` bis zum Maximum.
- `tests/hud.rs`: das echte HUD mit Pointer-Events, wie das Spiel sie an Blitz gibt: Klick wählt
  eine Kachel aus, Rechtsklick benutzt sie, Klick auf einen Slot, Ziehen einer Kachel auf einen
  Slot (nicht bei unbenutzbaren Items), Herausziehen eines Slots.

## Danach

- Gewicht als Masse des Spielers. Überladung macht auch langsamer (siehe [ideen.md](ideen/ideen.md)).
- Gegner-Drops mit Loot-Tabellen.
- Tooltip mit Namen beim Hover.
