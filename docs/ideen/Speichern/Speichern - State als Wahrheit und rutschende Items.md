# Speichern: State als Wahrheit und rutschende Items

Design-Notiz, Stand Oktober 2026. Ergänzt „Speicherstand und große Welt“. Noch nicht umgesetzt.

## State statt ECS als Wahrheit

`GroupState` und `CellState` bleiben die Wahrheit, Entities sind nur die Darstellung.

Alternative ECS als Wahrheit verworfen:

- Entladene Karten haben keine Entities. Ihr Zustand braucht trotzdem eine Map, also gäbe es zwei Wahrheiten.
- Vor jedem Snapshot müssten alle geladenen Entities in den State zurückgeschrieben werden. Das läuft im Hauptthread und wächst mit der Zahl der Objekte.
- `Arc::ptr_eq` reicht dann nicht. Geänderte Chunks müssten über `Changed` und `RemovedComponents` erkannt werden.
- Despawnte Objekte sind einfach weg. `Removed` müsste extra festgehalten werden.
- Items, die in eine nicht geladene Zelle rutschen, landen trotzdem im State.
- Einziger Vorteil: Systeme ändern nur Komponenten und können den State nicht vergessen. Bei uns kommen Änderungen ohnehin als einzelne Events, das Mitschreiben kostet fast nichts.

## Änderungen anwenden

- Gameplay-Systeme ändern den State nie direkt. Sie schicken nur Events.
- Ein zentrales System pro Objekttyp verarbeitet das Event: zuerst State per `Arc::make_mut`, dann die Entity.
- Beide werden geändert, aber nur an einer Stelle im Code. So laufen sie nicht auseinander.

Beispiel Item aufheben:

1. `ItemPickedUp`-Event kommt rein.
2. Zelle per `make_mut` holen, Item über seine ID entfernen, ins Inventar legen.
3. Im selben System die Entity despawnen.

Beispiel Brunnen: `used = true` im State, dann Sprite oder Komponente anpassen.

## Rutschende Items

Ausnahme vom Prinzip: Die Physik bewegt zuerst die Entity. Die Position kommt erst in den State, wenn das Item liegen bleibt.

### Item existiert sofort im State

- Beim Ablegen oder Droppen schreibt das Event das Item sofort mit ID und Startposition in die passende Zelle. Erst danach rutscht es los.
- Speichern während es rutscht: Im State steht die Startposition. Nach dem Laden liegt es dort. Es ist nie weg und nie doppelt.
- Beim Liegenbleiben werden Position und ggf. Zellwechsel nachgetragen. Zellwechsel: beide Zellen per `make_mut` ändern.

Optional, falls die alte Position stört: Direkt vor dem Snapshot alle wachen Items durchgehen und ihre aktuelle Position in den State schreiben. Läuft im selben Frame wie der Snapshot, bleibt also atomar. Nach dem Laden hat das Item keinen Schwung mehr.

### Liegenbleiben erkennen: Avian-Sleeping

- Avian fügt keine Static-Komponente hinzu. Es gibt aber die Marker-Komponente `Sleeping`.
- Reagieren per Observer auf das Hinzufügen von `Sleeping` oder per Query mit `Added<Sleeping>`. Dort Position holen, Zielzelle berechnen, per `make_mut` in den State schreiben.
- Items dürfen kein `SleepingDisabled` haben.
- Alternative, falls Sleeping zu träge ist: selbst prüfen, wann `LinearVelocity` unter einen Schwellwert fällt.

So funktioniert Sleeping:

- **Einschlafen:** Liegen lineare und Drehgeschwindigkeit unter `SleepingThreshold`, läuft ein Timer. Bleibt das länger als `DeactivationTime` so, bekommt der Körper `Sleeping`. Bewegt er sich vorher, startet der Timer neu.
- **Schlafend:** Keine Integration und keine Kontaktauflösung mehr.
- **Aufwachen:** Durch Berührung eines wachen Körpers, durch Setzen von Geschwindigkeit, Kraft oder Impuls oder durch einen Joint.
- **Inseln:** Sich berührende Körper bilden Simulationsinseln. Eine Insel schläft nur gemeinsam und wacht gemeinsam auf.
- Folge: Ein Gegner, der durch einen Item-Haufen läuft, weckt alle auf. Beim erneuten Einschlafen werden die neuen Positionen geschrieben.

Dauer und Einstellungen:

- `DeactivationTime` ist standardmäßig etwa eine halbe Sekunde.
- `SleepingThreshold` ist sehr klein und wird mit `PhysicsLengthUnit` skaliert. Bei Pixel-Einheiten die Length Unit passend setzen, sonst schlafen Items zu früh oder zu spät ein.
- Körper haben keine Reibung. Items brauchen deshalb etwas `LinearDamping`, sonst rutschen sie lange und schlafen kaum ein.

Unsicher: Avian hat Sleeping in den letzten Versionen umgebaut. Namen und Defaults für 0.8 in der Doku prüfen.

## Tests (zusätzlich)

- Speichern während ein Item rutscht: Nach dem Laden existiert es genau einmal, an Start- bzw. letzter geschriebener Position.
- Item schläft in anderer Zelle ein: Beide Zellen geändert, Item genau einmal vorhanden.
- Gegner weckt schlafende Items: Neue Positionen landen nach dem Einschlafen im State.
