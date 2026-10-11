//! What the host game must answer. libgears never owns the host's world.

use crate::math::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverHeight {
    /// Waist-high cover (Gears `CT_MidLevel`).
    Mid,
    /// Full-height wall (Gears `CT_Standing`).
    Standing,
}

/// A spot the character can take cover at. Gears gets these from hand-placed
/// CoverLinks; hosts can do the same or use a runtime cover finder (milestone L3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverSpot {
    /// Where the character's centre ends up (already offset from the wall).
    pub location: Vec3,
    /// Wall normal, pointing away from the wall toward the character.
    pub normal: Vec3,
    pub height: CoverHeight,
    /// Distance from `location` to the cover's left / right end, along the wall.
    /// Left/right are as seen by a character facing the wall (-normal).
    pub dist_to_left_edge: f32,
    pub dist_to_right_edge: f32,
    pub can_slip_left: bool,
    pub can_slip_right: bool,
}

impl CoverSpot {
    /// Direction along the wall toward the character's right when facing the wall.
    pub fn right(&self) -> Vec3 {
        // facing = -normal; right of facing (UE: X fwd, Y right) = (-f.y, f.x) rotated
        let f = -self.normal;
        Vec3::new(-f.y, f.x, 0.0).normalized()
    }
}

pub trait World {
    /// Gears' native `FindCoverFromLocAndDir`: best cover reachable from `from`
    /// going roughly along `dir` (unit, flat) within `max_dist`.
    fn find_cover(&self, from: Vec3, dir: Vec3, max_dist: f32) -> Option<CoverSpot>;

    /// Is the straight path clear for the character (used by evade's 128 uu check)?
    fn is_clear(&self, from: Vec3, to: Vec3, radius: f32) -> bool;

    /// Gears' native `CoverLink.GetSwatTurnTarget`: the cover across the gap from
    /// `edge` (the end of the current cover `spot`), going along `side`, within
    /// `max_dist` (`CoverLink.TurnDist`). Hosts without swat turns can keep the default.
    fn find_swat_target(
        &self,
        _spot: &CoverSpot,
        _edge: Vec3,
        _side: Vec3,
        _max_dist: f32,
    ) -> Option<CoverSpot> {
        None
    }

    /// Gears' native `GearPawn.FindMantleDistance`: how far along `dir` (unit, flat)
    /// from `from` the character must travel to clear the waist-high cover in front
    /// (its far face plus `radius`), at most `max_dist`. `None` = can't vault here
    /// (no mid cover, too thick, or a full-height wall in the way). The landing
    /// beyond is checked separately with `is_clear`. Default: no vaulting.
    fn mantle_distance(
        &self,
        _from: Vec3,
        _dir: Vec3,
        _max_dist: f32,
        _radius: f32,
    ) -> Option<f32> {
        None
    }
}

/// A world with no geometry at all.
pub struct EmptyWorld;

impl World for EmptyWorld {
    fn find_cover(&self, _: Vec3, _: Vec3, _: f32) -> Option<CoverSpot> {
        None
    }
    fn is_clear(&self, _: Vec3, _: Vec3, _: f32) -> bool {
        true
    }
}
