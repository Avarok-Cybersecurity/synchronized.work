//! Moving from one layout to the next. `Full` is a damped spring; `Lite` (reduced motion, or a
//! low-power host) swaps straight to the target keyframe. Motion is never "zeroed" into an
//! instant transition by tweaking spring constants — the mode is explicit.

use crate::layout::Pos;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    /// Per second squared.
    pub stiffness: f32,
    /// Per second.
    pub damping: f32,
    /// Distance and speed below which a node is at rest.
    pub rest: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Motion {
    Full(Spring),
    Lite,
}

/// Current positions easing toward a target layout. Nodes are matched by index; the caller
/// rebuilds the tween when the set of nodes changes, seeding new nodes at their parent.
#[derive(Debug, Clone, PartialEq)]
pub struct Tween {
    current: Vec<Pos>,
    velocity: Vec<Pos>,
    target: Vec<Pos>,
}

impl Tween {
    pub fn new(from: Vec<Pos>, target: Vec<Pos>) -> Option<Self> {
        (from.len() == target.len()).then(|| Self {
            velocity: vec![Pos { x: 0.0, y: 0.0 }; from.len()],
            current: from,
            target,
        })
    }

    pub fn positions(&self) -> &[Pos] {
        &self.current
    }

    /// Advances by `dt_ms`; returns whether anything is still moving (the App's
    /// `events.RequestFrames`, the site's next `requestAnimationFrame`).
    pub fn step(&mut self, dt_ms: f32, motion: Motion) -> bool {
        let spring = match motion {
            Motion::Lite => {
                self.current.clone_from(&self.target);
                self.velocity
                    .iter_mut()
                    .for_each(|v| *v = Pos { x: 0.0, y: 0.0 });
                return false;
            }
            Motion::Full(spring) => spring,
        };
        let dt = (dt_ms / 1000.0).clamp(0.0, 0.064);
        let mut moving = false;
        for ((p, v), t) in self
            .current
            .iter_mut()
            .zip(&mut self.velocity)
            .zip(&self.target)
        {
            for (pos, vel, tgt) in [(&mut p.x, &mut v.x, t.x), (&mut p.y, &mut v.y, t.y)] {
                let accel = spring.stiffness * (tgt - *pos) - spring.damping * *vel;
                *vel += accel * dt;
                *pos += *vel * dt;
                if (tgt - *pos).abs() < spring.rest && vel.abs() < spring.rest {
                    *pos = tgt;
                    *vel = 0.0;
                } else {
                    moving = true;
                }
            }
        }
        moving
    }
}
