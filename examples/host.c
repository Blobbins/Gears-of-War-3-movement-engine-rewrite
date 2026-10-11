/* Minimal libgears host: one waist-high wall, slide into cover, vault it, fire the Gnasher.
 * cc -Iinclude examples/host.c target/release/libgears.a -lm -lpthread -ldl -o host */
#include <stdio.h>
#include "libgears.h"

static void step(GearsCharacter *c, GearsWalls *w, float sy, int a, int frames) {
    GearsInput in = { 0, sy, 0, a, 0 };
    for (int i = 0; i < frames; i++) gears_character_tick_walls(c, &in, 1.0f / 60, w);
}

int main(void) {
    GearsWalls *w = gears_walls_new(24);
    gears_walls_add(w, 300, -200, 300, 200, -1, 0, 0);        /* waist-high wall at x = 300 */
    GearsCharacter *c = gears_character_new((GearsVec3){ 0, 0, 0 }, 0);
    step(c, w, 1, 1, 3);  step(c, w, 1, 0, 60);                /* tap A toward it: slide in */
    step(c, w, 1, 1, 3);  step(c, w, 0, 0, 120);               /* stick to the wall + A: vault */
    GearsState s; gears_character_state(c, &s);
    printf("after vault: x=%.1f state=%d\n", s.pos.x, s.state);
    char buf[160];
    for (uint32_t i = 0; i < gears_character_event_count(c); i++) { gears_character_event(c, i, buf, sizeof buf); puts(buf); }
    GearsWeapon *g = gears_weapon_new(GEARS_GNASHER);
    GearsWeaponTick t = gears_weapon_tick(g, 1, 0, 1, 1.0f / 60);
    float p[20]; uint32_t n = gears_weapon_pellets(GEARS_GNASHER, 1, s.time_since_evade, 0, p, 10);
    printf("gnasher: %u shot(s), %u pellets, 10 hits at 200 uu = %.0f, 5 hits at 1200 uu = %.0f, mag %u\n",
           t.shots, n, gears_weapon_damage(GEARS_GNASHER, 10, 200), gears_weapon_damage(GEARS_GNASHER, 5, 1200), gears_weapon_ammo(g).mag);
    gears_weapon_free(g); gears_character_free(c); gears_walls_free(w);
    return 0;
}
