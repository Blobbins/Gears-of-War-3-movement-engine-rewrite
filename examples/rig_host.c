/* libgears rig from C: load your marcus.glb, run forward for a second, print the rig.
 * cc -Iinclude examples/rig_host.c target/release/libgears.a -lm -lpthread -ldl -o rig_host
 * ./rig_host _local/exports/marcus/pack/marcus.glb */
#include <stdio.h>
#include <stdlib.h>
#include "libgears.h"

int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "usage: rig_host marcus.glb\n"); return 1; }
    FILE *f = fopen(argv[1], "rb");
    if (!f) { perror(argv[1]); return 1; }
    fseek(f, 0, SEEK_END); long n = ftell(f); fseek(f, 0, SEEK_SET);
    uint8_t *b = malloc(n); fread(b, 1, n, f); fclose(f);
    GearsAnim *a = gears_anim_new(b, (size_t)n, NULL);
    free(b);
    if (!a) { fprintf(stderr, "not a usable export\n"); return 1; }
    GearsCharacter *c = gears_character_new((GearsVec3){ 0, 0, 0 }, 0);
    GearsWeapon *w = gears_weapon_new(GEARS_LANCER);
    GearsWalls *walls = gears_walls_new(24);
    GearsInput in = { 0, 1, 0, 0, 0 };
    const GearsRigPose *p = NULL;
    for (int i = 0; i < 60; i++) {
        gears_character_tick_walls(c, &in, 1.0f / 60, walls);
        GearsAnimInput ai = { 0, 0, 0, 0, GEARS_LANCER, 0, w };
        p = gears_anim_update(a, c, &ai, 1.0f / 60);
    }
    for (uint32_t i = 0; i < gears_rig_bone_count(); i++)
        printf("%-11s parent %2d  at (%6.1f %6.1f %6.1f)\n", gears_rig_bone_name(i), gears_rig_bone_parent(i),
               p->pos[i].x, p->pos[i].y, p->pos[i].z);
    gears_weapon_free(w); gears_character_free(c); gears_anim_free(a); gears_walls_free(walls);
    return 0;
}
