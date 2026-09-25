//! A value that changes over a layer's length, through a few points.

use serde::{Deserialize, Serialize};

/// Most points a curve can have.
pub const MAX_POINTS: usize = 8;

/// A value over time: points of (time, value), with time as a fraction of
/// the layer's length (0..1), joined by straight lines. Before the first
/// point and after the last, the value stays level. Fixed capacity, so
/// generators stay `Copy` and cheap to hand to the audio thread.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "Vec<[f32; 2]>", into = "Vec<[f32; 2]>")]
pub struct Curve {
    points: [[f32; 2]; MAX_POINTS],
    len: u8,
}

impl Curve {
    /// The same value all along.
    pub fn flat(v: f32) -> Curve {
        Curve::new(&[(0.0, v)])
    }

    /// From (time, value) points, in any order. Extra points past
    /// [`MAX_POINTS`] are dropped; an empty list gives a flat 0.
    pub fn new(points: &[(f32, f32)]) -> Curve {
        let mut c = Curve {
            points: [[0.0; 2]; MAX_POINTS],
            len: 0,
        };
        for &(t, v) in points.iter().take(MAX_POINTS) {
            c.points[c.len as usize] = [t.clamp(0.0, 1.0), v];
            c.len += 1;
        }
        if c.len == 0 {
            c.len = 1;
        }
        c.sort();
        c
    }

    fn sort(&mut self) {
        let n = self.len as usize;
        self.points[..n].sort_by(|a, b| a[0].total_cmp(&b[0]));
    }

    pub fn points(&self) -> &[[f32; 2]] {
        &self.points[..self.len as usize]
    }

    /// Move point `i`; it stays between its neighbors in time.
    pub fn set(&mut self, i: usize, t: f32, v: f32) {
        let n = self.len as usize;
        if i >= n {
            return;
        }
        let lo = if i == 0 { 0.0 } else { self.points[i - 1][0] };
        let hi = if i + 1 == n {
            1.0
        } else {
            self.points[i + 1][0]
        };
        self.points[i] = [t.clamp(lo, hi), v];
    }

    /// Add a point; returns its index, or `None` when the curve is full.
    pub fn insert(&mut self, t: f32, v: f32) -> Option<usize> {
        let n = self.len as usize;
        if n >= MAX_POINTS {
            return None;
        }
        let t = t.clamp(0.0, 1.0);
        let i = self.points[..n].partition_point(|p| p[0] <= t);
        self.points.copy_within(i..n, i + 1);
        self.points[i] = [t, v];
        self.len += 1;
        Some(i)
    }

    /// Remove a point, keeping at least one.
    pub fn remove(&mut self, i: usize) {
        let n = self.len as usize;
        if n <= 1 || i >= n {
            return;
        }
        self.points.copy_within(i + 1..n, i);
        self.len -= 1;
    }

    /// The highest point: with straight lines between points, nothing in
    /// between goes above it.
    pub fn max(&self) -> f32 {
        self.points().iter().map(|p| p[1]).fold(f32::MIN, f32::max)
    }

    /// The value at time `t` (0..1).
    pub fn at(&self, t: f32) -> f32 {
        self.interpolate(t, |a, b, f| a + (b - a) * f)
    }

    /// Like [`at`](Self::at), but moving evenly in ratio rather than in
    /// value: right for frequencies, where 100 → 400 passes 200 halfway.
    pub fn at_log(&self, t: f32) -> f32 {
        self.interpolate(t, |a, b, f| {
            let (a, b) = (a.max(1e-3), b.max(1e-3));
            a * (b / a).powf(f)
        })
    }

    fn interpolate(&self, t: f32, mix: impl Fn(f32, f32, f32) -> f32) -> f32 {
        let p = self.points();
        let i = p.partition_point(|q| q[0] <= t);
        if i == 0 {
            return p[0][1];
        }
        if i == p.len() {
            return p[p.len() - 1][1];
        }
        let (a, b) = (p[i - 1], p[i]);
        let span = b[0] - a[0];
        let f = if span > 0.0 { (t - a[0]) / span } else { 1.0 };
        mix(a[1], b[1], f)
    }

    /// The same curve with every value changed by `f`.
    pub fn map(&self, f: impl Fn(f32) -> f32) -> Curve {
        let mut c = *self;
        for p in &mut c.points[..c.len as usize] {
            p[1] = f(p[1]);
        }
        c
    }

    /// Smallest and largest values.
    pub fn range(&self) -> (f32, f32) {
        self.points()
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), p| {
                (lo.min(p[1]), hi.max(p[1]))
            })
    }
}

impl From<Vec<[f32; 2]>> for Curve {
    fn from(v: Vec<[f32; 2]>) -> Curve {
        let points: Vec<(f32, f32)> = v.into_iter().map(|[t, v]| (t, v)).collect();
        Curve::new(&points)
    }
}

impl From<Curve> for Vec<[f32; 2]> {
    fn from(c: Curve) -> Vec<[f32; 2]> {
        c.points().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_and_holds_at_the_ends() {
        let c = Curve::new(&[(1.0, 10.0), (0.5, 20.0), (0.25, 0.0)]);
        assert_eq!(c.points(), [[0.25, 0.0], [0.5, 20.0], [1.0, 10.0]]);
        assert_eq!(c.at(0.0), 0.0);
        assert_eq!(c.at(0.375), 10.0);
        assert_eq!(c.at(0.75), 15.0);
        assert_eq!(c.at(2.0), 10.0);
        let hz = Curve::new(&[(0.0, 100.0), (1.0, 400.0)]);
        assert!((hz.at_log(0.5) - 200.0).abs() < 1e-3);
        assert_eq!(Curve::flat(3.0).at(0.7), 3.0);
        assert_eq!(Curve::new(&[]).at(0.5), 0.0);
    }

    #[test]
    fn editing_keeps_points_in_order() {
        let mut c = Curve::new(&[(0.0, 1.0), (1.0, 2.0)]);
        assert_eq!(c.insert(0.5, 5.0), Some(1));
        c.set(1, 2.0, 6.0);
        assert_eq!(c.points()[1], [1.0, 6.0], "clamped before the next point");
        c.set(1, -1.0, 6.0);
        assert_eq!(c.points()[1], [0.0, 6.0]);
        c.remove(1);
        assert_eq!(c.points().len(), 2);
        c.remove(0);
        c.remove(0);
        assert_eq!(c.points().len(), 1, "never empty");
        for i in 0..MAX_POINTS {
            c.insert(i as f32 / 10.0, 0.0);
        }
        assert_eq!(c.points().len(), MAX_POINTS);
        assert_eq!(c.insert(0.5, 0.0), None);
        assert_eq!(c.range(), (0.0, 2.0));
    }

    #[test]
    fn serializes_as_a_list_of_points() {
        let c = Curve::new(&[(0.0, 1.0), (0.5, 2.0)]);
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, "[[0.0,1.0],[0.5,2.0]]");
        assert_eq!(serde_json::from_str::<Curve>(&json).unwrap(), c);
    }
}
