# Konzept: Schwarm-Gegner

Umgesetzt (`src/objects/enemy/swarm/`, `src/combat/tokens.rs`). Alle Zahlen sind Startwerte fürs
Ausbalancieren.

## Ziel

- Kleine Gegner greifen als Schwarm an: kurz zuschlagen, dann zurückziehen, dann neu anlaufen.
  Sie kleben nicht dauerhaft am Spieler.
- Nicht alle Gegner schlagen gleichzeitig zu. Der Schaden pro Sekunde hängt an einer festen
  Obergrenze, nicht an der Zahl der Gegner. Fünf Gegner wirken bedrohlich, ohne den Spieler sofort
  zu töten.
- Der Schwarm wirkt lebendig: Die Gegner kreisen um den Spieler und kommen aus wechselnden
  Richtungen.
- Gegenstück zu großen Gegnern: Die holen sichtbar aus und schlagen hart zu (siehe „Abgrenzung“).

## Zustände

Ein Zustandsautomat pro Gegner, als Komponente `SwarmState`. Jeder Zustand setzt nur die
`DesiredVelocity`, die kraftbasierte Bewegung sorgt für Abbremsen und Umdrehen.

- **`Circle { since }`:** Startzustand nach dem Aufwachen. Der Gegner umkreist den Spieler im
  Kreisabstand und wartet auf ein Angriffs-Token, sobald er sein letztes zurückgegeben hat.
- **`Approach`:** Mit Token läuft er per Flowfield auf den Spieler zu, schneller als normal.
- **`Strike`:** Bei Kontakt ein Treffer, danach sofort `Retreat`. Kein eigener Zustand: Im Anlauf
  ist der Gegner `combat::Striking`, `combat` trifft dann wie bei allen Gegnern, sobald der Spieler
  in Reichweite ist, und meldet den Treffer (`combat::Hit`).
- **`Retreat { target, timer, stuck }`:** Er läuft zu einem Rückzugspunkt (siehe unten). Danach geht es
  zurück in `Circle`.

Übergänge:

- `Circle` → `Approach`: Token bekommen.
- `Approach` → `Strike`: Der Spieler ist in Reichweite.
- `Approach` → `Retreat`: Der Spieler trifft ihn während des Anlaufs (siehe „Zusammenspiel“) oder
  der Anlauf dauert zu lange (`APPROACH_TIMEOUT`).
- `Strike` → `Retreat`: immer.
- `Retreat` → `Circle`: Timer abgelaufen, Ziel erreicht oder festgefahren.

Das Token wird erst `token_time` nach Beginn des Anlaufs zurückgegeben, egal was der Gegner bis
dahin tut. Wird es früher frei, etwa beim Wechsel nach `Retreat`, wäre es nach dem kurzen Anlauf
(etwa 0,3 s) schon wieder frei: Mit genug kreisenden Gegnern käme so mehr als ein Treffer pro
Token alle paar Sekunden, und der Schaden pro Sekunde wüchse doch mit der Zahl der Gegner. So
erlaubt jedes Token höchstens einen Treffer pro `token_time`.

## Rückzug

Direkt weg vom Spieler läuft ein Gegner oft in Wände. Stattdessen peilt er einen Punkt an:

- im Kreisabstand um den Spieler,
- um 90–150° vom Winkel des Angriffs gedreht, zufällig links oder rechts.

So weicht er seitlich aus, und der nächste Anlauf kommt aus einer neuen Richtung.

- Der Punkt wird beim Wechsel nach `Retreat` einmal festgelegt, relativ zum Spieler. Bewegt sich
  der Spieler, wandert der Punkt mit.
- Liegt der Punkt in einer blockierten Zelle, wird der Winkel in Schritten verkleinert, bis eine
  freie Zelle gefunden ist. Sonst gilt die Richtung direkt weg vom Spieler.
- **Festgefahren:** Ist die Geschwindigkeit eine Weile fast null, obwohl er sich bewegen will,
  bricht er den Rückzug ab und geht nach `Circle`.
- Dauer und Winkel werden pro Rückzug zufällig gewählt, damit der Schwarm nicht im Gleichtakt
  angreift.

## Kreisen

- Der Gegner hält den Kreisabstand zum Spieler und bewegt sich tangential, in einer pro Gegner
  zufällig gewählten Richtung.
- Zu nah: Anteil weg vom Spieler. Zu weit: Anteil zum Spieler hin, über das Flowfield, falls eine
  Wand dazwischen liegt.
- Die vorhandene Trennung zwischen Gegnern verteilt sie von selbst auf dem Kreis.

## Angriffs-Tokens

Die Idee aus „Verhalten“ in [spawnen.md](spawnen.md):

- Ressource `AttackTokens` pro Ziel (vorerst nur der Spieler) mit einer Obergrenze `MAX_TOKENS`.
- Nur ein Gegner in `Circle` mit Token darf nach `Approach`. Wer schon am längsten kreist, kommt
  zuerst dran, damit niemand ewig wartet.
- Gegner, die einschlafen, despawnen oder sterben, geben ihr Token zurück.
- Andere Gegnerarten können dieselbe Ressource nutzen. Dann zählt ein großer Gegner etwa als zwei
  Tokens.

Damit ist `MAX_TOKENS` der wichtigste Regler für den Schaden pro Sekunde.

## Werte

In einer RON-Datei unter `assets/` mit Hot-Reload, wie `items.ron`:

- Schaden pro Treffer: 4 % der maximalen Lebenspunkte
- Anlauf: 1,5-fache Normalgeschwindigkeit
- `APPROACH_TIMEOUT`: 2 s
- `token_time`: 2 s
- Rückzug: 1,0–2,0 s
- Drehung des Rückzugspunkts: 90–150°
- Kreisabstand: 15–20 px
- `MAX_TOKENS`: 2

Rechnung: Mit zwei Tokens und je 2 s pro Token kommt höchstens ein Treffer pro Sekunde. Bei 4 % pro
Treffer überlebt ein untätiger Spieler 25 s, egal ob zwei oder acht Gegner da sind (so gemessen;
ein einzelner Gegner braucht 49 s, da sein Zyklus länger ist als `token_time`).

## Zusammenspiel

- **Rückstoß:** Trifft der Spieler einen Gegner in `Approach`, bricht der Angriff ab und er geht in
  `Retreat`. Gutes Timing wird so belohnt.
- **Schlafen:** `enemy::Asleep` hat Vorrang. Ein schlafender Gegner hat keinen Zustand und kein
  Token. Beim Aufwachen startet er in `Circle`.
- **Flowfield:** `Approach` und das Annähern beim Kreisen nutzen das Feld. Außerhalb des
  Flowfield-Radius gilt die direkte Richtung, wie bei allen Gegnern.
- **Hinterhalte:** Gegner aus Hinterhalten starten wach in `Circle`. Ein Hinterhalt mit vielen
  kleinen Gegnern wird so nicht automatisch tödlich.
- **Sicht:** Unsichtbare Gegner verhalten sich gleich. Kreisende Gegner tauchen am Rand der Sicht
  auf und verschwinden wieder, das ist gewollt.

## Abgrenzung

Große Gegner bekommen kein Schwarmverhalten, sondern einen angekündigten Angriff:

- **Ausholen** (0,3–0,5 s): Der Gegner bleibt stehen und zeigt den Schlag an.
- **Treffer:** Erst jetzt wird geprüft, ob der Spieler noch in Reichweite ist.
- **Erholung:** Kurz verwundbar, das Fenster für den Gegenangriff.

Das kommt in ein eigenes Konzept. Gemeinsam ist beiden nur die Ressource `AttackTokens`.

## Code

- `enemy::swarm`: `spawn_enemy` gibt Gegnern bis `swarm::MAX_RADIUS` (5 px, die kleinen) den
  Marker `Swarm`. Wach bekommen sie `SwarmState` und `Orbit` (Kreisabstand und Richtung, beim
  Aufwachen gewählt), schlafend verlieren sie beides samt Token (`start_and_stop`).
  `update_states` wechselt die Zustände und verteilt freie Tokens, `move_swarm` setzt die
  `DesiredVelocity`. `move_towards_player` lässt Schwarm-Gegner aus; die Trennung teilen sich beide
  über `enemy::Crowd`.
- `combat::tokens`: `AttackTokens` (Obergrenze), `AttackToken` als Komponente am Halter, so gibt
  ein despawnter oder sterbender Gegner es von selbst zurück. Wer ein freies Token bekommt und wann
  er es zurückgibt, entscheidet die Gegnerart; beim Schwarm hält `swarm::TokenHold` die Zeit.
- `combat::Restrained` (alle `Swarm`): Solche Einheiten greifen nur an, solange sie
  `combat::Striking` sind, beim Schwarm während des Anlaufs.
- `combat::Hit` meldet jeden Treffer mit Angreifer und Ziel. Ein Treffer in `Approach`, eigener oder
  des Spielers, beendet den Anlauf.
- `assets/swarm.ron` (`SwarmValues`, hot-reloaded über `swarm::values`), das auch `AttackTokens`
  setzt. Der Zufall kommt aus `SwarmRng` (`fastrand`), Tests setzen einen festen Seed.

## Tests

- Zustandswechsel: `Circle` → `Approach` nur mit Token, `Strike` → `Retreat` immer.
- Tokens: Nie mehr als `MAX_TOKENS` gleichzeitig vergeben, nur mit Token in `Approach`. Ein Token
  wird `token_time` nach Beginn des Anlaufs frei. Ein schlafender oder despawnter Gegner gibt sein
  Token zurück.
- Rückzugspunkt in einer blockierten Zelle: Es wird ein freier gewählt.
- Festgefahren im Rückzug: Gegner wechselt nach `Circle`.
- Zeit bis zum Tod: Fünf Schwarm-Gegner gegen einen untätigen Spieler, headless. Die Zeit liegt im
  Zielbereich (20–30 s) und ändert sich mit acht Gegnern kaum.

## Offen

- **Angriff des Spielers:** Der Spieler schlägt noch automatisch alle 0,5 s zu und reicht weiter
  als ein kleiner Gegner, trifft einen anlaufenden Gegner also fast immer zuerst und bricht den
  Anlauf ab. „Gutes Timing“ gibt es erst mit einem Angriff per Eingabe.
- **Tokens bei mehreren Zielen:** Später greifen Gegner vielleicht auch Begleiter oder Brunnen an.
  Dann braucht jedes Ziel eigene Tokens.
- **Kreisen in engen Gängen:** Ohne Platz für einen Kreis stauen sich die Gegner. Erst beobachten.
- **Rückzug sichtbar machen:** Vielleicht eine kurze Pose oder ein Laut, damit der Spieler merkt,
  dass jetzt ein Fenster für den Angriff ist.
- **Gruppenangriff:** Ab und zu dürfen alle gleichzeitig angreifen, als kurzer Höhepunkt. Passt
  zum Director in [spawnen.md](spawnen.md).
