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
      "healing_potion": (name: "Heiltrank", weight: 0.5, stackable: true, color: (0.8, 0.2, 0.2),
          effect: Some(Heal(30.))),
      "apple": (name: "Apfel", weight: 0.2, stackable: true, color: (0.3, 0.8, 0.2),
          effect: Some(Heal(5.))),
      "sword": (name: "Schwert", weight: 3.0, stackable: false, color: (0.7, 0.7, 0.8)),
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
- Stapelbare Items haben zusätzlich `Stack(u32)` mit der Anzahl. Nicht stapelbare Items haben kein
  `Stack` und zählen als 1.
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
  - Ist es stapelbar und hat der Besitzer schon einen Stapel derselben ID, wird die Anzahl dort
    addiert und das aufgehobene Item despawnt.
- Hervorhebung: Bei `PointerOver` wird das Item heller, bei `PointerOut` wieder normal.
  Einen Tooltip mit Namen gibt es noch nicht.

## Fallenlassen

- `DropItem { item, count }` legt Items neben den Besitzer, z. B. 12 px unter ihn.
  - Ist `count` kleiner als der Stapel, wird ein neues Item mit `count` abgespalten und abgelegt.
    Der Rest bleibt im Inventar.
  - Sonst wird das Item selbst abgelegt: `HeldBy` entfernen, `Sprite` (in der normalen Farbe),
    `Transform`, Sichtbarkeit und `Pickable` setzen.
- Am Boden werden Stapel nicht zusammengeführt.

## Benutzen

- `UseItem { item }` benutzt ein Item im Besitz. Hat sein Typ eine Wirkung, wird es verbraucht:
  Ein Stapel verliert eins, das letzte Item wird despawnt. Dann meldet `ItemUsed { by, effect }`
  die Wirkung für den Besitzer. Items ohne Wirkung und Items am Boden bleiben unverändert.
- Das Item-Modul wendet Wirkungen nicht selbst an. Das tun die Module, die sie betreffen: `combat`
  heilt bei `Effect::Heal` bis höchstens zum Maximum. Neue Wirkungen sind eine neue Variante von
  `Effect` und ein System in dem Modul, das sie umsetzt.
- Ein Item wird auch bei voller Gesundheit verbraucht.

## Inventarfenster

- Taste I öffnet und schließt das Fenster. Das Spiel läuft dabei weiter.
- Das HUD bekommt im `hud::State` die Liste der Items des Spielers: Entity, Name, Anzahl,
  Gewicht. Dazu kommt das Gesamtgewicht.
- Pro Zeile gibt es die Buttons „Ablegen“ (ganzer Stapel) und bei Stapeln „1 ablegen“. Beide
  schicken `hud::Action::DropItem { item, count }`, das Spiel schreibt daraus `DropItem`.
- Items mit Wirkung haben zusätzlich „Benutzen“: `hud::Action::UseItem(item)` wird zu `UseItem`.
- Das Fenster hat die Klasse `blocks-pointer`. Klicks darauf erreichen die Welt nicht.

## Debug-Taste

- F spawnt ein Item an der Mausposition in der Welt. Ist die Maus außerhalb des Fensters, liegt es
  neben dem Spieler.
- Jeder Druck nimmt die nächste ID aus den Definitionen (alphabetisch, reihum), damit sich Stapeln
  und Nicht-Stapeln testen lassen.
- Noch keine Definitionen geladen: Die Taste tut nichts.

## Code

Drei Plugins, damit die Tests ohne Asset-System und Eingabe auskommen:

- `src/items/mod.rs`: `Item`, `Stack`, `HeldBy`/`Inventory`, `ItemDefinitions`, `Effect`, die
  Messages `PickUp`, `DropItem`, `UseItem` und `ItemUsed` und die Systeme dazu (`items::Plugin`).
- `src/combat/mod.rs`: Heilen bei `ItemUsed`.
- `src/items/definitions.rs`: Asset-Loader für `items.ron` und Übernahme in die Ressource
  (`items::definitions::Plugin`).
- `src/items/input.rs`: Aufheben per Klick, Hervorhebung, Debug-Taste (`items::input::Plugin`).
- `src/ui/hud.rs`: Inventarfenster. `src/ui/mod.rs`: Taste I, Zustand und `Action::DropItem`.
- `tests/items.rs`: Aufheben mit und ohne Stapeln, Ablegen eines ganzen Stapels, Abspalten beim
  Ablegen, keine Stapelung nicht stapelbarer Items, Benutzen (Stapel, letztes Item, ohne Wirkung,
  am Boden) und ob sich `assets/items.ron` lesen lässt.
- `tests/combat.rs`: Heilen durch `ItemUsed` bis zum Maximum.

## Danach

- Gewicht als Masse des Spielers. Überladung macht auch langsamer (siehe [ideen.md](ideen/ideen.md)).
- Gegner-Drops mit Loot-Tabellen.
- Tooltip mit Namen beim Hover.
