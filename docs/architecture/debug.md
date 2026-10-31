# Debug controls

- WASD: move the player.
- E / R / T: spawn a small / medium / large enemy at the mouse cursor, asleep until the player sees it.
- G: freeze all enemies or let them go again (also a HUD checkbox).
- F: spawn an item at the mouse cursor, cycling through the item types.
- N: move the clock on by an hour (also a HUD button).
- Left click on an item within reach: pick it up.
- Tab: open/close the inventory above the hotbar. Tiles and slots then work alike: right click to use; click picks one up (selected, with a popup with use/drop buttons) and the next click puts it down, exactly like dragging it: on a slot it goes there (slot onto slot swaps), a slot put down elsewhere is emptied. 1–5 put the hovered or selected one on their slot.
- 1–5: use the item type in that hotbar slot (also by clicking the slot), while the inventory is closed.
- Alt+Enter: toggle borderless fullscreen (a setting, so it is saved).
- Escape: close the topmost window (settings menu, selection popup, inventory), or else open the settings menu, which pauses the game.

The HUD shows the day and time of day with its phase; next to it, "Nächste Phase" moves the clock on to the start of the next phase (dusk, night, dawn, day), "Zeitraffer" cycles the clock's speed through ×10, ×60 and normal (by shortening `DayNight::day_length`; a change to `assets/light.ron` ends it), and the checkbox "Uhr anhalten" stops it.

The HUD button "Gegner spawnen" spawns a small awake enemy out of sight, "Flowfield" cycles the flow field overlay through the grids (5, 8, 12, 16 px clearance) and off.

The HUD checkbox "Performance" (on by default) shows the frames per second, the frame time and the time of the physics in it, averaged over half a second. The measuring runs either way; the checkbox only decides whether the HUD shows it, which makes the HUD paint again twice a second.

`PhysicsDebugPlugin` draws collider outlines. They start hidden; the HUD checkbox "Kollider" shows them.

The HUD checkbox "Gegner einfrieren (G)" freezes all enemies (`enemy::Frozen`, run condition `enemy::active`): they neither walk, turn nor attack, but can still be hit and pushed.
