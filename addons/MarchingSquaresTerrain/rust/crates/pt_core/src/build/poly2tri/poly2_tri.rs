//! Port of `Poly2TriSwift/Poly2Tri.swift`.
//! Source: `03-ghidra/by-source/first-party/Sources/Poly2TriSwift/Poly2Tri.c`.
//!
//! Entry point of the triangulator: input validation, normalisation into the unit box, the sweep
//! (`sweep.rs`), and orientation correction of the output triangles.

use std::collections::HashSet;
use std::fmt;

use super::sweep::Sweep;

/// A 2D point (Swift `SIMD2<Double>`).
pub type Point = [f64; 2];

/// Errors thrown by [`Poly2Tri::triangulate`] (Swift `TriangulationError: LocalizedError`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TriangulationError {
    /// Invalid input, with the user-facing message.
    Invalid(String),
    /// The sweep could not produce a valid triangulation.
    Failed,
}

const FAILED_MESSAGE: &str = "The polygon could not be triangulated.";
const NEEDS_THREE_POINTS: &str = "Each contour must contain at least three finite, non-collinear points.";
const REPEATED_POINTS: &str = "Repeated points are not supported.";
const EDGES_OVERLAP: &str = "Contour edges overlap.";
const INTERSECTS_ITSELF: &str = "A contour intersects itself.";
const CONTOURS_TOUCH: &str = "Contours touch or intersect.";
const HOLES_NEST: &str = "Holes must not overlap or nest.";
const HOLE_OUTSIDE: &str = "A hole is outside the contour.";
const BOUNDS_NOT_VALID: &str = "The polygon bounds are not valid.";
const POINTS_TOO_CLOSE: &str = "Polygon points are too close at this coordinate scale.";

impl TriangulationError {
    /// The localized description (Swift `errorDescription`).
    pub fn error_description(&self) -> String {
        match self {
            TriangulationError::Invalid(message) => message.clone(),
            TriangulationError::Failed => FAILED_MESSAGE.to_owned(),
        }
    }

    fn invalid(message: &str) -> Self {
        TriangulationError::Invalid(message.to_owned())
    }
}

impl fmt::Display for TriangulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.error_description())
    }
}

impl std::error::Error for TriangulationError {}

/// Constrained Delaunay triangulation of a polygon with holes (Swift `enum Poly2Tri`).
pub struct Poly2Tri;

impl Poly2Tri {
    /// Triangulates `contour` with `holes`. Returns triangle corner indices into the concatenation
    /// `contour + holes[0] + holes[1] + …`, three per triangle, wound like the contour.
    pub fn triangulate(contour: &[Point], holes: &[Vec<Point>]) -> Result<Vec<usize>, TriangulationError> {
        let mut loops: Vec<&[Point]> = Vec::with_capacity(holes.len() + 1);
        loops.push(contour);
        loops.extend(holes.iter().map(Vec::as_slice));
        Self::validate(&loops)?;

        let all: Vec<Point> = loops.iter().flat_map(|l| l.iter().copied()).collect();
        let mut minimum = all[0];
        let mut maximum = all[0];
        for p in &all {
            minimum = [p[0].min(minimum[0]), p[1].min(minimum[1])];
            maximum = [if maximum[0] <= p[0] { p[0] } else { maximum[0] }, if maximum[1] <= p[1] { p[1] } else { maximum[1] }];
        }
        let extent = [maximum[0] - minimum[0], maximum[1] - minimum[1]];
        let scale = if extent[0] <= extent[1] { extent[1] } else { extent[0] };
        if !(scale.is_finite() && scale > 0.0) {
            return Err(TriangulationError::invalid(BOUNDS_NOT_VALID));
        }
        let normalized: Vec<Point> =
            all.iter().map(|p| [(p[0] - minimum[0]) / scale, (p[1] - minimum[1]) / scale]).collect();
        let distinct: HashSet<(u64, u64)> = normalized.iter().map(|p| point_key(*p)).collect();
        if distinct.len() != normalized.len() {
            return Err(TriangulationError::invalid(POINTS_TOO_CLOSE));
        }

        let counts: Vec<usize> = loops.iter().map(|l| l.len()).collect();
        let triangles = Sweep::new(normalized, &counts).run()?;
        let positive = Self::area(contour) > 0.0;
        let mut result = Vec::with_capacity(triangles.len() * 3);
        for t in &triangles {
            let (a, b, c) = (all[t[0]], all[t[1]], all[t[2]]);
            let counter_clockwise = (b[0] - c[0]) * (a[1] - c[1]) < (b[1] - c[1]) * (a[0] - c[0]);
            if counter_clockwise == positive {
                result.extend_from_slice(t);
            } else {
                result.extend_from_slice(&[t[0], t[2], t[1]]);
            }
        }
        let expected = (all.len() + holes.len() * 2 - 2) * 3;
        if result.len() != expected {
            return Err(TriangulationError::Failed);
        }
        Ok(result)
    }

    /// Signed area of a closed loop (positive when counter-clockwise).
    fn area(points: &[Point]) -> f64 {
        let o = points[0];
        let n = points.len();
        let mut sum = 0.0;
        for i in 0..n {
            let a = points[i];
            let b = points[if i + 1 == n { 0 } else { i + 1 }];
            sum += (o[0] - b[0]) * (a[1] - b[1]) - (a[0] - b[0]) * (o[1] - b[1]);
        }
        sum * 0.5
    }

    /// Rejects degenerate loops, repeated points, self-intersections, touching loops, nested
    /// holes and holes outside the contour (loop 0 is the contour).
    fn validate(loops: &[&[Point]]) -> Result<(), TriangulationError> {
        let mut seen: HashSet<(u64, u64)> = HashSet::new();
        for points in loops {
            let n = points.len();
            if n < 3 || points.iter().any(|p| !p[0].is_finite() || !p[1].is_finite()) {
                return Err(TriangulationError::invalid(NEEDS_THREE_POINTS));
            }
            let area = Self::area(points);
            if !area.is_finite() || area == 0.0 {
                return Err(TriangulationError::invalid(NEEDS_THREE_POINTS));
            }
            for p in points.iter() {
                if !seen.insert(point_key(*p)) {
                    return Err(TriangulationError::invalid(REPEATED_POINTS));
                }
            }
            for i in 0..n {
                let a = points[i];
                let b = points[(i + 1) % n];
                let c = points[(i + 2) % n];
                let cross = (b[1] - c[1]) * (a[0] - c[0]) - (b[0] - c[0]) * (a[1] - c[1]);
                let dot = (c[1] - b[1]) * (b[1] - a[1]) + (c[0] - b[0]) * (b[0] - a[0]);
                if cross == 0.0 && dot < 0.0 {
                    return Err(TriangulationError::invalid(EDGES_OVERLAP));
                }
                for j in (i + 2)..n {
                    if j == n - 1 && i == 0 {
                        continue;
                    }
                    if segments_intersect(a, b, points[j], points[(j + 1) % n]) {
                        return Err(TriangulationError::invalid(INTERSECTS_ITSELF));
                    }
                }
            }
        }
        for i in 0..loops.len() {
            for j in (i + 1)..loops.len() {
                let (first, second) = (loops[i], loops[j]);
                for ai in 0..first.len() {
                    let (a, b) = (first[ai], first[(ai + 1) % first.len()]);
                    for bi in 0..second.len() {
                        if segments_intersect(a, b, second[bi], second[(bi + 1) % second.len()]) {
                            return Err(TriangulationError::invalid(CONTOURS_TOUCH));
                        }
                    }
                }
                if i != 0 && (contains(first, second[0]) || contains(second, first[0])) {
                    return Err(TriangulationError::invalid(HOLES_NEST));
                }
            }
            if i != 0 && !contains(loops[0], loops[i][0]) {
                return Err(TriangulationError::invalid(HOLE_OUTSIDE));
            }
        }
        Ok(())
    }
}

/// Hash key of a point with Swift `SIMD2<Double>` equality (`-0.0 == 0.0`).
fn point_key(p: Point) -> (u64, u64) {
    ((p[0] + 0.0).to_bits(), (p[1] + 0.0).to_bits())
}

/// `(q - r) × (p - r)` with the operand order of the original.
fn orient(p: Point, q: Point, r: Point) -> f64 {
    (q[1] - r[1]) * (p[0] - r[0]) - (q[0] - r[0]) * (p[1] - r[1])
}

/// Whether `r` lies in the bounding box of segment `p`–`q`.
fn within_box(p: Point, q: Point, r: Point) -> bool {
    p[0].min(q[0]) <= r[0] && r[0] <= p[0].max(q[0]) && p[1].min(q[1]) <= r[1] && r[1] <= p[1].max(q[1])
}

/// Whether segments `a`–`b` and `c`–`d` cross or touch.
fn segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool {
    let d1 = orient(a, b, c);
    let d2 = orient(a, b, d);
    let d3 = orient(c, d, a);
    let d4 = orient(c, d, b);
    let crosses = ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0)) && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0));
    crosses
        || (d1 == 0.0 && within_box(a, b, c))
        || (d2 == 0.0 && within_box(a, b, d))
        || (d3 == 0.0 && within_box(c, d, a))
        || (d4 == 0.0 && within_box(c, d, b))
}

/// Even-odd point-in-polygon test.
fn contains(polygon: &[Point], p: Point) -> bool {
    let n = polygon.len();
    let mut inside = false;
    for k in 0..n {
        let v = polygon[k];
        let w = polygon[if k + 1 == n { 0 } else { k + 1 }];
        if (p[1] < v[1]) != (p[1] < w[1]) && p[0] < (p[1] - v[1]) * (w[0] - v[0]) / (w[1] - v[1]) + v[0] {
            inside = !inside;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, size: f64) -> Vec<Point> {
        vec![[x, y], [x + size, y], [x + size, y + size], [x, y + size]]
    }

    fn signed_area(points: &[Point], t: &[usize]) -> f64 {
        let (a, b, c) = (points[t[0]], points[t[1]], points[t[2]]);
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    }

    fn total_area(points: &[Point], indices: &[usize]) -> f64 {
        indices.chunks(3).map(|t| signed_area(points, t) * 0.5).sum()
    }

    #[test]
    fn square_gives_two_counter_clockwise_triangles() {
        let contour = square(0.0, 0.0, 2.0);
        let indices = Poly2Tri::triangulate(&contour, &[]).unwrap();
        assert_eq!(indices.len(), 6);
        assert!(indices.chunks(3).all(|t| signed_area(&contour, t) > 0.0));
        assert!((total_area(&contour, &indices) - 4.0).abs() < 1e-12);
    }

    #[test]
    fn clockwise_contour_keeps_clockwise_winding() {
        let mut contour = square(0.0, 0.0, 1.0);
        contour.reverse();
        let indices = Poly2Tri::triangulate(&contour, &[]).unwrap();
        assert!(indices.chunks(3).all(|t| signed_area(&contour, t) < 0.0));
    }

    #[test]
    fn square_with_square_hole() {
        let contour = square(0.0, 0.0, 4.0);
        let mut hole = square(1.0, 1.0, 2.0);
        hole.reverse();
        let indices = Poly2Tri::triangulate(&contour, &[hole.clone()]).unwrap();
        assert_eq!(indices.len(), (8 + 2 - 2) * 3);
        let all: Vec<Point> = contour.iter().chain(hole.iter()).copied().collect();
        assert!(indices.chunks(3).all(|t| signed_area(&all, t) > 0.0));
        assert!((total_area(&all, &indices) - 12.0).abs() < 1e-9);
    }

    #[test]
    fn concave_polygon() {
        let contour = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [2.0, 1.0], [0.0, 4.0]];
        let indices = Poly2Tri::triangulate(&contour, &[]).unwrap();
        assert_eq!(indices.len(), 9);
        assert!((total_area(&contour, &indices) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn collinear_points_on_an_edge_are_kept() {
        let contour = vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]];
        let indices = Poly2Tri::triangulate(&contour, &[]).unwrap();
        assert_eq!(indices.len(), 9);
        assert!((total_area(&contour, &indices) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn degenerate_input_errors() {
        let err = |c: Vec<Point>, h: Vec<Vec<Point>>| Poly2Tri::triangulate(&c, &h).unwrap_err().error_description();
        assert_eq!(err(vec![[0.0, 0.0], [1.0, 0.0]], vec![]), NEEDS_THREE_POINTS);
        assert_eq!(err(vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]], vec![]), NEEDS_THREE_POINTS);
        assert_eq!(err(vec![[0.0, 0.0], [1.0, 0.0], [f64::NAN, 1.0]], vec![]), NEEDS_THREE_POINTS);
        assert_eq!(err(square(0.0, 0.0, 4.0), vec![vec![[0.0, 0.0], [1.0, 1.0], [1.0, 2.0]]]), REPEATED_POINTS);
        assert_eq!(err(vec![[0.0, 0.0], [2.0, 0.0], [1.0, 0.0], [1.0, 1.0]], vec![]), EDGES_OVERLAP);
        assert_eq!(err(vec![[0.0, 0.0], [4.0, 4.0], [4.0, 0.0], [0.0, 1.0]], vec![]), INTERSECTS_ITSELF);
        assert_eq!(err(square(0.0, 0.0, 4.0), vec![square(3.0, 1.0, 2.0)]), CONTOURS_TOUCH);
        assert_eq!(err(square(0.0, 0.0, 4.0), vec![square(5.0, 5.0, 1.0)]), HOLE_OUTSIDE);
        assert_eq!(err(square(0.0, 0.0, 8.0), vec![square(1.0, 1.0, 4.0), square(2.0, 2.0, 1.0)]), HOLES_NEST);
    }

    #[test]
    fn ring_and_comb_cover_their_area() {
        let circle = |r: f64, n: usize| -> Vec<Point> {
            (0..n).map(|i| { let a = i as f64 * std::f64::consts::TAU / n as f64; [r * a.cos(), r * a.sin()] }).collect()
        };
        let contour = circle(10.0, 48);
        let mut hole = circle(4.0, 24);
        hole.reverse();
        let indices = Poly2Tri::triangulate(&contour, &[hole.clone()]).unwrap();
        let all: Vec<Point> = contour.iter().chain(hole.iter()).copied().collect();
        let expected = Poly2Tri::area(&contour) + Poly2Tri::area(&hole);
        assert!((total_area(&all, &indices) - expected).abs() < 1e-9);
        assert!(indices.chunks(3).all(|t| signed_area(&all, t) > 0.0));

        let mut comb = vec![[0.0, 0.0], [20.0, 0.0]];
        for i in (0..10).rev() {
            let x = i as f64 * 2.0;
            comb.extend_from_slice(&[[x + 2.0, 5.0], [x + 1.0, 1.0 + i as f64 * 0.1]]);
        }
        comb.pop();
        let indices = Poly2Tri::triangulate(&comb, &[]).unwrap();
        assert!((total_area(&comb, &indices) - Poly2Tri::area(&comb)).abs() < 1e-9);
        assert!(indices.chunks(3).all(|t| signed_area(&comb, t) > 0.0));
    }

    #[test]
    fn failed_error_has_default_description() {
        assert_eq!(TriangulationError::Failed.to_string(), FAILED_MESSAGE);
    }
}
