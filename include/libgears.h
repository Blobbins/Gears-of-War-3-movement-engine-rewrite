/* libgears — Gears of War 3 style third-person movement and weapon rules, for any game.
 *
 * Clean reimplementation from notes on Gears 3's own script; no Gears code or data.
 * Units: Unreal units (uu), seconds, radians. Axes: X forward, Y right, Z up; yaw 0 =
 * +X, positive yaw turns toward +Y. Convert to your engine's units/axes in your host.
 *
 * Per frame: fill a GearsInput, call gears_character_tick (with your world callbacks)
 * or gears_character_tick_walls (with a built-in wall list), read gears_character_state
 * and put your player / camera / animations there. Fixed steps of 1/60 s are best.
 * Build: cargo build --release -p gears_ffi  → gears.dll / libgears.so / libgears.a
 */
#ifndef LIBGEARS_H
#define LIBGEARS_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

typedef struct { float x, y, z; } GearsVec3;

typedef struct {
    float stick_x, stick_y;  /* left stick, -1..1 (y = forward) */
    float cam_yaw;           /* camera yaw: the stick is camera-relative */
    int32_t a;               /* A held (roadie run / cover / evade / vault) */
    int32_t aim;             /* left trigger held (walk at half speed, no roadie) */
} GearsInput;

typedef struct {
    GearsVec3 location;      /* where the character's centre ends up */
    GearsVec3 normal;        /* away from the wall, toward the character */
    int32_t standing;        /* 0 = waist-high (vaultable), 1 = full height */
    float dist_to_left_edge, dist_to_right_edge;   /* facing the wall */
    int32_t can_slip_left, can_slip_right;
} GearsCoverSpot;

/* World queries your engine answers (traces against your level). Any may be NULL. */
typedef struct {
    void *user;
    /* best cover reachable from `from` roughly along `dir` within max_dist; 1 = found */
    int32_t (*find_cover)(void *user, GearsVec3 from, GearsVec3 dir, float max_dist, GearsCoverSpot *out);
    /* is the straight path clear for a character of `radius`? 1 = clear */
    int32_t (*is_clear)(void *user, GearsVec3 from, GearsVec3 to, float radius);
    /* swat turn: cover across the gap from `edge` going along `side`; 1 = found */
    int32_t (*find_swat_target)(void *user, const GearsCoverSpot *spot, GearsVec3 edge, GearsVec3 side, float max_dist, GearsCoverSpot *out);
    /* vault: distance along `dir` to clear the waist-high cover ahead (far face + radius); 1 = can vault */
    int32_t (*mantle_distance)(void *user, GearsVec3 from, GearsVec3 dir, float max_dist, float radius, float *out);
} GearsWorld;

enum {
    GEARS_STATE_WALK = 0, GEARS_STATE_IN_COVER = 1, GEARS_STATE_ROADIE_RUN = 2,
    GEARS_STATE_RUN2COVER = 3, GEARS_STATE_COVER_RUN = 4, GEARS_STATE_COVER_SLIP = 5,
    GEARS_STATE_EVADE_FWD = 6, GEARS_STATE_EVADE_BWD = 7, GEARS_STATE_EVADE_LT = 8,
    GEARS_STATE_EVADE_RT = 9, GEARS_STATE_PUSH_OUT = 10, GEARS_STATE_SWAT_TURN = 11,
    GEARS_STATE_VAULT = 12
};

typedef struct {
    GearsVec3 pos, vel;
    float yaw;
    float height;            /* above the start ground (vault) */
    int32_t state;           /* GEARS_STATE_* */
    int32_t in_cover, mirrored;
    GearsCoverSpot cover;    /* valid when in_cover */
    float time, time_since_evade;
} GearsState;

typedef struct GearsCharacter GearsCharacter;
GearsCharacter *gears_character_new(GearsVec3 pos, float yaw);
void gears_character_free(GearsCharacter *c);
void gears_character_set_pos(GearsCharacter *c, GearsVec3 pos);
void gears_character_tick(GearsCharacter *c, const GearsInput *in, float dt, const GearsWorld *world);
void gears_character_state(const GearsCharacter *c, GearsState *out);
/* state-change log: "[GEARS] t=1.234 Walk -> Run2Cover (run to cover)" */
uint32_t gears_character_event_count(const GearsCharacter *c);
int32_t gears_character_event(const GearsCharacter *c, uint32_t index, char *buf, uint32_t len);

/* Built-in world made of straight walls (thickness > 0: cover on both faces). */
typedef struct GearsWalls GearsWalls;
GearsWalls *gears_walls_new(float thickness);
void gears_walls_free(GearsWalls *w);
void gears_walls_add(GearsWalls *w, float ax, float ay, float bx, float by, float nx, float ny, int32_t standing);
void gears_character_tick_walls(GearsCharacter *c, const GearsInput *in, float dt, GearsWalls *w);

/* Weapons: 0 = Lancer, 1 = Gnasher. You trace the shots and apply the damage. */
enum { GEARS_LANCER = 0, GEARS_GNASHER = 1 };
typedef struct GearsWeapon GearsWeapon;
enum { GEARS_AR_NONE = 0, GEARS_AR_GOOD = 1, GEARS_AR_PERFECT = 2, GEARS_AR_FAILED = 3 };
/* reload pressed while reloading = an active reload attempt (one per reload) */
typedef struct { uint32_t shots; int32_t reload_started, reload_finished, active_reload; } GearsWeaponTick;
typedef struct { float elapsed, total, perfect_start, perfect_end, good_end; int32_t result; } GearsReloadBar;
typedef struct { uint32_t mag, spare; float reloading, reload_duration; } GearsAmmo;
GearsWeapon *gears_weapon_new(int32_t kind);
void gears_weapon_free(GearsWeapon *w);
GearsWeaponTick gears_weapon_tick(GearsWeapon *w, int32_t fire, int32_t reload, int32_t steady, float dt);
GearsAmmo gears_weapon_ammo(const GearsWeapon *w);
float gears_weapon_aim_error(const GearsWeapon *w);     /* cone half-angle, radians */
float gears_weapon_damage(int32_t kind, uint32_t hits, float dist);
uint32_t gears_weapon_pellets(int32_t kind, int32_t aiming, float time_since_evade, float speed, float *out_yaw_pitch, uint32_t cap);
float gears_weapon_recoil(int32_t kind);                 /* camera kick per shot, radians */
float gears_weapon_damage_mult(const GearsWeapon *w);    /* perfect-reload bonus (×1.3 / ×1.2) */
GearsReloadBar gears_weapon_reload_bar(const GearsWeapon *w);

/* Animation + the libgears standard rig (24 bones, our own skeleton: shareable).
 * The motion comes from the player's own export (marcus.glb, tools/setup.py).
 * Rig frame: Y up, +Z forward, +X the character's left, Unreal units, relative to the
 * pawn's feet. Rotations are from the rig's T-pose (gears_rig_bone_rest). */
#define GEARS_RIG_BONES 24
typedef struct { float x, y, z, w; } GearsQuat;
typedef struct {
    int32_t aiming;            /* 1 while aiming */
    float cam_pitch, cam_yaw;  /* radians, Gears convention (X forward, Y right) */
    float stick_x;             /* -1..1 */
    int32_t weapon_kind;       /* GEARS_LANCER / GEARS_GNASHER */
    uint32_t shots;            /* GearsWeaponTick.shots this frame */
    const GearsWeapon *weapon; /* for reload animations; may be NULL */
} GearsAnimInput;
typedef struct {
    uint32_t bone_count;
    GearsVec3 pos[GEARS_RIG_BONES];   /* joint positions */
    GearsQuat local[GEARS_RIG_BONES]; /* relative to the parent bone */
    GearsQuat model[GEARS_RIG_BONES]; /* in the rig frame */
    GearsVec3 gun_at, gun_dir;        /* grip point, barrel direction */
    int32_t mirrored;                 /* cover facing left (already applied) */
    int32_t has_yaw_override;         /* face yaw_override instead of the pawn's yaw */
    float yaw_override;
    float height, forward_offset;     /* vault root motion: up, and along the pawn's yaw */
} GearsRigPose;
typedef struct GearsAnim GearsAnim;
GearsAnim *gears_anim_new(const uint8_t *glb, size_t len, const char *aim_json); /* NULL if unusable */
void gears_anim_free(GearsAnim *a);
/* after gears_character_tick; the result stays valid until the next call */
const GearsRigPose *gears_anim_update(GearsAnim *a, const GearsCharacter *c, const GearsAnimInput *in, float dt);
uint32_t gears_rig_bone_count(void);
const char *gears_rig_bone_name(uint32_t i);  /* "pelvis", "upperarm_l", ... */
int32_t gears_rig_bone_parent(uint32_t i);    /* -1 for the root */
GearsVec3 gears_rig_bone_rest(uint32_t i);    /* T-pose joint position */

#ifdef __cplusplus
}
#endif
#endif
