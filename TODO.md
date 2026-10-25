# TODO

## Performance

- Compute the flow field less often or with less work: larger cells (8 instead of 4 px would close
  corridors one tile wide), only the grids used by the enemies alive, or only after the player moved
  a few cells. Measure first with the HUD's "Performance" tree.
- Repaint the HUD only when the hover state changes, not on every mouse move over the game.

## Gameplay

- Balance the combat values in `player::combat()` and `Enemy::combat`: five enemies touching the
  player kill it within a few seconds.

## Map

- Give the tileset's buildings, gravestones and bridge railings colliders before they are used on
  the map.

## Code

- Let combat trigger a `Died` event instead of handling the deaths of players and enemies itself, so
  `combat` doesn't need to know about the objects. The combat tests would need the observers then.
- Optionally check the line of sight with a margin of a cell: the straight way to the player may
  pass up to about 2.5 px closer to a corner than the clearance. Since bodies have no friction,
  enemies slide off instead of getting stuck, though.
