//! Port of `Poly2TriSwift/Sweep.swift`.
//! Source: `03-ghidra/by-source/first-party/Sources/Poly2TriSwift/Sweep.c`.
//!
//! The sweep-line constrained Delaunay triangulation of poly2tri (Domiter & Žalik), as the Swift
//! port wrote it: triangles and advancing-front nodes live in arenas and are referred to by index
//! (Swift used class references; node links were `weak`). Left/right variants of the edge-fill
//! steps are merged behind a `right` flag, recursion is bounded by [`Sweep::enter`], and every
//! internal inconsistency fails with [`TriangulationError::Failed`].

use std::collections::HashSet;

use super::poly2_tri::{Point, TriangulationError};

type Result<T> = std::result::Result<T, TriangulationError>;

/// Distance below which a new point counts as lying on the front node's x (Swift `1e-12`).
const EPSILON: f64 = 1e-12;
/// Margin of the artificial head/tail points, relative to the bounds.
const ALPHA: f64 = 0.3;
/// Maximum recursion depth of the sweep steps.
const MAXIMUM_DEPTH: i64 = 256;
/// Minimum step budget; the budget is `max(points * 1000, 100000)`.
const MINIMUM_STEPS: i64 = 100_000;

use std::f64::consts::FRAC_PI_2;
const FRAC_3PI_4: f64 = 2.356194490192345;

fn failed() -> TriangulationError {
    TriangulationError::Failed
}

/// A triangle of the sweep (Swift `Sweep.Triangle`).
#[derive(Clone, Debug)]
struct Triangle {
    points: [usize; 3],
    /// `neighbors[i]` is across the edge opposite `points[i]`.
    neighbors: [Option<usize>; 3],
    constrained: [bool; 3],
    delaunay: [bool; 3],
}

impl Triangle {
    fn new(points: [usize; 3]) -> Self {
        Triangle { points, neighbors: [None; 3], constrained: [false; 3], delaunay: [false; 3] }
    }

    fn index(&self, p: usize) -> Result<usize> {
        self.points.iter().position(|&q| q == p).ok_or_else(failed)
    }

    /// Index of the edge `p`–`q` (the index of the third point), in either direction.
    fn edge_index(&self, p: usize, q: usize) -> Option<usize> {
        (0..3).find(|&i| {
            let (a, b) = (self.points[(i + 1) % 3], self.points[(i + 2) % 3]);
            (a == p && b == q) || (a == q && b == p)
        })
    }

    /// Point after `p` (counter-clockwise).
    fn point_ccw(&self, p: usize) -> Result<usize> {
        Ok(self.points[(self.index(p)? + 1) % 3])
    }

    /// Point before `p` (clockwise).
    fn point_cw(&self, p: usize) -> Result<usize> {
        Ok(self.points[(self.index(p)? + 2) % 3])
    }

    /// Swift `ncw(_:)`: the neighbour clockwise of `p`.
    fn ncw(&self, p: usize) -> Result<Option<usize>> {
        Ok(self.neighbors[(self.index(p)? + 1) % 3])
    }

    /// Swift `nccw(_:)`: the neighbour counter-clockwise of `p`.
    fn nccw(&self, p: usize) -> Result<Option<usize>> {
        Ok(self.neighbors[(self.index(p)? + 2) % 3])
    }

    /// Swift `rotate(_:_:)`: turns the triangle so `old` moves one place and `new` replaces the
    /// point clockwise of it.
    fn rotate(&mut self, old: usize, new: usize) -> Result<()> {
        let i = self.index(old)?;
        let previous = self.points[(i + 2) % 3];
        self.points[(i + 1) % 3] = old;
        self.points[i] = previous;
        self.points[(i + 2) % 3] = new;
        Ok(())
    }
}

/// A node of the advancing front (Swift `Sweep.Node`).
#[derive(Clone, Debug)]
struct Node {
    point: usize,
    next: Option<usize>,
    prev: Option<usize>,
    triangle: Option<usize>,
}

/// One triangulation run over normalised points (Swift `final class Sweep`).
pub struct Sweep {
    points: Vec<Point>,
    /// Constrained edges `(p, q)` stored at their upper point `q`.
    edges: Vec<Vec<(usize, usize)>>,
    triangles: Vec<Triangle>,
    nodes: Vec<Node>,
    head: Option<usize>,
    edge_p: usize,
    edge_q: usize,
    depth: i64,
    steps: i64,
    count: usize,
}

impl Sweep {
    /// Prepares a sweep over `points`, made of consecutive closed loops of `counts` points each.
    pub fn new(points: Vec<Point>, counts: &[usize]) -> Self {
        let count = points.len();
        let mut sweep = Sweep {
            points,
            edges: vec![Vec::new(); count],
            triangles: Vec::new(),
            nodes: Vec::new(),
            head: None,
            edge_p: 0,
            edge_q: 0,
            depth: 0,
            steps: 0,
            count,
        };
        let mut start = 0;
        for &n in counts {
            for i in 0..n {
                let a = start + i;
                let b = start + (i + 1) % n;
                let (p, q) = if sweep.less(a, b) { (a, b) } else { (b, a) };
                sweep.edges[q].push((p, q));
            }
            start += n;
        }
        sweep
    }

    /// Runs the sweep. Returns the interior triangles as point-index triples.
    pub fn run(mut self) -> Result<Vec<[usize; 3]>> {
        let n = self.count;
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| {
            if self.less(a, b) {
                std::cmp::Ordering::Less
            } else if self.less(b, a) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
        let min_x = fold(&self.points, 0, |v, m| if m <= v { m } else { v });
        let max_x = fold(&self.points, 0, |v, m| if v <= m { m } else { v });
        let min_y = fold(&self.points, 1, |v, m| if m <= v { m } else { v });
        let max_y = fold(&self.points, 1, |v, m| if v <= m { m } else { v });
        let dx = (max_x - min_x) * ALPHA;
        let y = min_y - (max_y - min_y) * ALPHA;
        self.points.push([min_x - dx, y]);
        self.points.push([max_x + dx, y]);

        let first = self.add_triangle([order[0], n, n + 1]);
        let left = self.make_node(n, Some(first));
        let middle = self.make_node(order[0], Some(first));
        let right = self.make_node(n + 1, None);
        self.head = Some(left);
        self.nodes[left].next = Some(middle);
        self.nodes[middle].prev = Some(left);
        self.nodes[middle].next = Some(right);
        self.nodes[right].prev = Some(middle);

        for &p in &order[1..] {
            let new = self.point_event(p)?;
            for (ep, eq) in self.edges[p].clone() {
                self.edge_p = ep;
                self.edge_q = eq;
                let t = self.nodes[new].triangle.ok_or_else(failed)?;
                if !self.side(t, ep, eq) {
                    self.fill_edge(ep, eq, new)?;
                    let t = self.nodes[new].triangle.ok_or_else(failed)?;
                    self.edge_event(ep, eq, t, eq)?;
                }
            }
        }
        self.finalize()
    }

    /// Adds point `p` to the front above the node it projects onto; returns the new node.
    fn point_event(&mut self, p: usize) -> Result<usize> {
        let mut node = self.head.ok_or_else(failed)?;
        let mut next = self.nodes[node].next;
        while let Some(candidate) = next {
            if self.points[p][0] < self.points[self.nodes[candidate].point][0] {
                break;
            }
            node = candidate;
            next = self.nodes[node].next;
        }
        let next = self.nodes[node].next.ok_or_else(failed)?;
        let below = self.nodes[node].triangle.ok_or_else(failed)?;
        let t = self.add_triangle([p, self.nodes[node].point, self.nodes[next].point]);
        self.mark(t, Some(below));
        let new = self.make_node(p, Some(t));
        self.nodes[new].next = Some(next);
        self.nodes[new].prev = Some(node);
        self.nodes[next].prev = Some(new);
        self.nodes[node].next = Some(new);
        if !self.legalize(t)? {
            self.map(t)?;
        }
        if self.points[p][0] <= self.points[self.nodes[node].point][0] + EPSILON {
            self.fill(node)?;
        }
        self.fill_front(new)?;
        Ok(new)
    }

    /// Walks to a triangle on a constrained edge and collects everything inside the boundary.
    fn finalize(&self) -> Result<Vec<[usize; 3]>> {
        let head = self.head.ok_or_else(failed)?;
        let first = self.nodes[head].next.ok_or_else(failed)?;
        let mut t = self.nodes[first].triangle.ok_or_else(failed)?;
        let p = self.nodes[first].point;
        let mut seen = HashSet::new();
        loop {
            let i = self.triangles[t].index(p)?;
            if self.triangles[t].constrained[(i + 1) % 3] {
                break;
            }
            if !seen.insert(t) {
                return Err(failed());
            }
            t = self.triangles[t].nccw(p)?.ok_or_else(failed)?;
        }
        let mut stack = vec![t];
        let mut visited = HashSet::new();
        let mut result = Vec::new();
        while let Some(t) = stack.pop() {
            if !visited.insert(t) {
                continue;
            }
            let triangle = &self.triangles[t];
            let [a, b, c] = triangle.points;
            if triangle.points.iter().any(|&q| q >= self.count) || self.orient(a, b, c) == 0 {
                return Err(failed());
            }
            result.push(triangle.points);
            for i in 0..3 {
                if !triangle.constrained[i]
                    && let Some(neighbor) = triangle.neighbors[i] {
                        stack.push(neighbor);
                    }
            }
        }
        Ok(result)
    }

    /// Recursion guard: bounds the depth and the total number of steps.
    fn enter(&mut self) -> Result<()> {
        let budget = (self.count as i64) * 1000;
        let within = if budget < MINIMUM_STEPS { self.steps < MINIMUM_STEPS } else { self.steps < budget };
        if self.depth < MAXIMUM_DEPTH && within {
            self.depth += 1;
            self.steps += 1;
            Ok(())
        } else {
            Err(failed())
        }
    }

    /// Runs `body` inside [`Sweep::enter`], leaving the depth balanced on every path.
    fn guarded<T>(&mut self, body: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.enter()?;
        let result = body(self);
        self.depth -= 1;
        result
    }

    /// Point order of the sweep: by y, then by x.
    fn less(&self, a: usize, b: usize) -> bool {
        let (pa, pb) = (self.points[a], self.points[b]);
        if pa[1] != pb[1] {
            pa[1] < pb[1]
        } else {
            pa[0] < pb[0]
        }
    }

    /// Orientation of `a`, `b`, `c`: 1 counter-clockwise, -1 clockwise (or NaN), 0 collinear.
    fn orient(&self, a: usize, b: usize, c: usize) -> i64 {
        let (pa, pb, pc) = (self.points[a], self.points[b], self.points[c]);
        let v = (pa[0] - pc[0]) * (pb[1] - pc[1]) - (pa[1] - pc[1]) * (pb[0] - pc[0]);
        if v > 0.0 {
            1
        } else if v == 0.0 {
            0
        } else {
            -1
        }
    }

    /// Signed angle at `origin` from `a` to `b`.
    fn angle(&self, origin: usize, a: usize, b: usize) -> f64 {
        let o = self.points[origin];
        let (ax, ay) = (self.points[a][0] - o[0], self.points[a][1] - o[1]);
        let (bx, by) = (self.points[b][0] - o[0], self.points[b][1] - o[1]);
        (by * ax - bx * ay).atan2(by * ay + bx * ax)
    }

    /// Whether `d` lies strictly inside the circumcircle of `a`, `b`, `c`.
    fn incircle(&self, a: usize, b: usize, c: usize, d: usize) -> bool {
        let pd = self.points[d];
        let (adx, ady) = (self.points[a][0] - pd[0], self.points[a][1] - pd[1]);
        let (bdx, bdy) = (self.points[b][0] - pd[0], self.points[b][1] - pd[1]);
        let (cdx, cdy) = (self.points[c][0] - pd[0], self.points[c][1] - pd[1]);
        let oabd = bdy * adx - ady * bdx;
        let ocad = cdx * ady - adx * cdy;
        if oabd <= 0.0 || ocad <= 0.0 {
            return false;
        }
        let alift = ady * ady + adx * adx;
        let blift = bdy * bdy + bdx * bdx;
        let clift = cdy * cdy + cdx * cdx;
        ocad * blift + alift * (bdx * cdy - cdx * bdy) + oabd * clift > 0.0
    }

    fn add_triangle(&mut self, points: [usize; 3]) -> usize {
        self.triangles.push(Triangle::new(points));
        self.triangles.len() - 1
    }

    fn make_node(&mut self, point: usize, triangle: Option<usize>) -> usize {
        self.nodes.push(Node { point, next: None, prev: None, triangle });
        self.nodes.len() - 1
    }

    /// Swift `Triangle.mark(_:)`: links `t` and `other` across their shared edge, if any.
    fn mark(&mut self, t: usize, other: Option<usize>) {
        let Some(other) = other else { return };
        let points = self.triangles[t].points;
        let others = self.triangles[other].points;
        let Some(j) = (0..3).find(|&j| others.contains(&points[(j + 1) % 3]) && others.contains(&points[(j + 2) % 3]))
        else {
            return;
        };
        let Some(k) = (0..3).find(|&k| others[k] != points[(j + 1) % 3] && others[k] != points[(j + 2) % 3]) else {
            return;
        };
        self.triangles[t].neighbors[j] = Some(other);
        self.triangles[other].neighbors[k] = Some(t);
    }

    /// Point of `ot` across the edge it shares with `t` opposite `p`.
    fn opposite_point(&self, ot: usize, t: usize, p: usize) -> Result<usize> {
        let cw = self.triangles[t].point_cw(p)?;
        self.triangles[ot].point_cw(cw)
    }

    /// Swift `map(_:)`: points front nodes at `t` for each of its edges without a neighbour.
    fn map(&mut self, t: usize) -> Result<()> {
        for i in 0..3 {
            if self.triangles[t].neighbors[i].is_none() {
                let target = self.triangles[t].point_cw(self.triangles[t].points[i])?;
                let mut node = self.head;
                while let Some(n) = node {
                    if self.nodes[n].point == target {
                        self.nodes[n].triangle = Some(t);
                        break;
                    }
                    node = self.nodes[n].next;
                }
            }
        }
        Ok(())
    }

    /// Swift `side(_:_:_:)`: if `p`–`q` is an edge of `t`, marks it constrained on both sides.
    fn side(&mut self, t: usize, p: usize, q: usize) -> bool {
        let Some(i) = self.triangles[t].edge_index(p, q) else { return false };
        self.triangles[t].constrained[i] = true;
        if let Some(n) = self.triangles[t].neighbors[i]
            && let Some(j) = self.triangles[n].edge_index(p, q) {
                self.triangles[n].constrained[j] = true;
            }
        true
    }

    /// Restores the Delaunay property around `t`; returns whether any edge was flipped.
    fn legalize(&mut self, t: usize) -> Result<bool> {
        self.guarded(|s| {
            for i in 0..3 {
                if s.triangles[t].delaunay[i] {
                    continue;
                }
                let Some(ot) = s.triangles[t].neighbors[i] else { continue };
                let p = s.triangles[t].points[i];
                let op = s.opposite_point(ot, t, p)?;
                let oi = s.triangles[ot].index(op)?;
                if s.triangles[ot].constrained[oi] || s.triangles[ot].delaunay[oi] {
                    s.triangles[t].constrained[i] = s.triangles[ot].constrained[oi];
                    continue;
                }
                let ccw = s.triangles[t].point_ccw(p)?;
                let cw = s.triangles[t].point_cw(p)?;
                if s.incircle(p, ccw, cw, op) {
                    s.triangles[t].delaunay[i] = true;
                    s.triangles[ot].delaunay[oi] = true;
                    s.rotate(t, p, ot, op)?;
                    if !s.legalize(t)? {
                        s.map(t)?;
                    }
                    if !s.legalize(ot)? {
                        s.map(ot)?;
                    }
                    s.triangles[t].delaunay[i] = false;
                    s.triangles[ot].delaunay[oi] = false;
                    return Ok(true);
                }
            }
            Ok(false)
        })
    }

    /// Swift `rotate(_:_:_:_:)`: flips the edge shared by `t` (at `p`) and `ot` (at `op`),
    /// carrying edge flags and neighbours over.
    fn rotate(&mut self, t: usize, p: usize, ot: usize, op: usize) -> Result<()> {
        let n1 = self.triangles[t].nccw(p)?;
        let n2 = self.triangles[t].ncw(p)?;
        let n3 = self.triangles[ot].nccw(op)?;
        let n4 = self.triangles[ot].ncw(op)?;
        let (ti, oi) = (self.triangles[t].index(p)?, self.triangles[ot].index(op)?);
        let flags = |tri: &Triangle, i: usize, step: usize| (tri.constrained[(i + step) % 3], tri.delaunay[(i + step) % 3]);
        let e1 = flags(&self.triangles[t], ti, 2);
        let e2 = flags(&self.triangles[t], ti, 1);
        let e3 = flags(&self.triangles[ot], oi, 2);
        let e4 = flags(&self.triangles[ot], oi, 1);
        self.triangles[t].rotate(p, op)?;
        self.triangles[ot].rotate(op, p)?;
        let targets = [(ot, p, 2, e1), (t, p, 1, e2), (t, op, 2, e3), (ot, op, 1, e4)];
        for (tri, point, step, (constrained, delaunay)) in targets {
            let k = (self.triangles[tri].index(point)? + step) % 3;
            self.triangles[tri].constrained[k] = constrained;
            self.triangles[tri].delaunay[k] = delaunay;
        }
        self.triangles[t].neighbors = [None; 3];
        self.triangles[ot].neighbors = [None; 3];
        self.mark(ot, n1);
        self.mark(t, n2);
        self.mark(t, n3);
        self.mark(ot, n4);
        self.mark(t, Some(ot));
        Ok(())
    }

    /// Fills the hole above `node` with a triangle and removes the node from the front.
    fn fill(&mut self, node: usize) -> Result<()> {
        let prev = self.nodes[node].prev.ok_or_else(failed)?;
        let next = self.nodes[node].next.ok_or_else(failed)?;
        let t = self.add_triangle([self.nodes[prev].point, self.nodes[node].point, self.nodes[next].point]);
        let prev_triangle = self.nodes[prev].triangle.ok_or_else(failed)?;
        self.mark(t, Some(prev_triangle));
        let node_triangle = self.nodes[node].triangle.ok_or_else(failed)?;
        self.mark(t, Some(node_triangle));
        self.nodes[prev].next = Some(next);
        self.nodes[next].prev = Some(prev);
        if !self.legalize(t)? {
            self.map(t)?;
        }
        Ok(())
    }

    /// Swift `largeHole(_:)`: whether the front angle at `node` is too wide to fill.
    fn large_hole(&self, node: usize) -> Result<bool> {
        let next = self.nodes[node].next.ok_or_else(failed)?;
        let prev = self.nodes[node].prev.ok_or_else(failed)?;
        let origin = self.nodes[node].point;
        let a = self.angle(origin, self.nodes[next].point, self.nodes[prev].point);
        if a.abs() <= FRAC_PI_2 {
            return Ok(false);
        }
        if a < 0.0 {
            return Ok(true);
        }
        if let Some(next2) = self.nodes[next].next {
            let a = self.angle(origin, self.nodes[next2].point, self.nodes[prev].point);
            if (0.0..=FRAC_PI_2).contains(&a) {
                return Ok(false);
            }
        }
        if let Some(prev2) = self.nodes[prev].prev {
            let a = self.angle(origin, self.nodes[next].point, self.nodes[prev2].point);
            return Ok(!(0.0..=FRAC_PI_2).contains(&a));
        }
        Ok(true)
    }

    /// Fills the front on both sides of `n`, then any basin to its right.
    fn fill_front(&mut self, n: usize) -> Result<()> {
        let mut node = self.nodes[n].next;
        while let Some(current) = node {
            if self.nodes[current].next.is_none() || self.large_hole(current)? {
                break;
            }
            self.fill(current)?;
            node = self.nodes[current].next;
        }
        let mut node = self.nodes[n].prev;
        while let Some(current) = node {
            if self.nodes[current].prev.is_none() || self.large_hole(current)? {
                break;
            }
            self.fill(current)?;
            node = self.nodes[current].prev;
        }
        if let Some(next) = self.nodes[n].next
            && let Some(next2) = self.nodes[next].next {
                let (p, q) = (self.points[self.nodes[n].point], self.points[self.nodes[next2].point]);
                if (p[1] - q[1]).atan2(p[0] - q[0]) < FRAC_3PI_4 {
                    self.fill_basin(n)?;
                }
            }
        Ok(())
    }

    /// Fills the basin (a dip in the front) to the right of `node`.
    fn fill_basin(&mut self, node: usize) -> Result<()> {
        let next = self.nodes[node].next.ok_or_else(failed)?;
        let next2 = self.nodes[next].next.ok_or_else(failed)?;
        let y = |s: &Self, n: usize| s.points[s.nodes[n].point][1];
        let left = if self.orient(self.nodes[node].point, self.nodes[next].point, self.nodes[next2].point) == 1 {
            next2
        } else {
            next
        };
        let mut bottom = left;
        while let Some(n) = self.nodes[bottom].next {
            if y(self, bottom) < y(self, n) {
                break;
            }
            bottom = n;
        }
        if bottom == left {
            return Ok(());
        }
        let mut right = bottom;
        while let Some(n) = self.nodes[right].next {
            if y(self, n) <= y(self, right) {
                break;
            }
            right = n;
        }
        if right == bottom {
            return Ok(());
        }
        let width = self.points[self.nodes[right].point][0] - self.points[self.nodes[left].point][0];
        let top = if y(self, right) < y(self, left) { left } else { right };
        let mut current = bottom;
        loop {
            if y(self, top) - y(self, current) < width {
                return Ok(());
            }
            self.fill(current)?;
            let prev = self.nodes[current].prev.ok_or_else(failed)?;
            let next = self.nodes[current].next.ok_or_else(failed)?;
            if prev == left && next == right {
                return Ok(());
            }
            if prev == left {
                let next2 = self.nodes[next].next.ok_or_else(failed)?;
                if self.orient(self.nodes[current].point, self.nodes[next].point, self.nodes[next2].point) == -1 {
                    return Ok(());
                }
                current = next;
            } else if next == right {
                let prev2 = self.nodes[prev].prev.ok_or_else(failed)?;
                if self.orient(self.nodes[current].point, self.nodes[prev].point, self.nodes[prev2].point) == 1 {
                    return Ok(());
                }
                current = prev;
            } else {
                current = if y(self, prev) < y(self, next) { prev } else { next };
            }
        }
    }

    /// Fills the front above the constrained edge `ep`–`eq`, starting at `start`.
    fn fill_edge(&mut self, ep: usize, eq: usize, start: usize) -> Result<()> {
        let right = self.points[eq][0] < self.points[ep][0];
        let sign = if right { 1 } else { -1 };
        let mut node = start;
        loop {
            let next = self.step(node, right)?;
            let d = self.points[self.nodes[next].point][0] - self.points[ep][0];
            if 0.0 <= if right { d } else { -d } {
                return Ok(());
            }
            if self.orient(eq, self.nodes[next].point, ep) == sign {
                self.fill_below(ep, eq, node, right)?;
            } else {
                node = next;
            }
        }
    }

    /// The neighbour of `node` on the `right` (next) or left (prev) side.
    fn step(&self, node: usize, right: bool) -> Result<usize> {
        let n = &self.nodes[node];
        if right { n.next } else { n.prev }.ok_or_else(failed)
    }

    fn fill_below(&mut self, ep: usize, eq: usize, node: usize, right: bool) -> Result<()> {
        self.guarded(|s| {
            let sign = if right { 1 } else { -1 };
            if (s.points[s.nodes[node].point][0] - s.points[ep][0]) * sign as f64 >= 0.0 {
                return Ok(());
            }
            let n1 = s.step(node, right)?;
            let n2 = s.step(n1, right)?;
            if s.orient(s.nodes[node].point, s.nodes[n1].point, s.nodes[n2].point) == sign {
                s.fill_concave(ep, eq, node, right)
            } else {
                s.fill_convex(ep, eq, node, right)?;
                s.fill_below(ep, eq, node, right)
            }
        })
    }

    fn fill_concave(&mut self, ep: usize, eq: usize, node: usize, right: bool) -> Result<()> {
        self.guarded(|s| {
            let n = s.step(node, right)?;
            s.fill(n)?;
            let n = s.step(node, right)?;
            let c = s.nodes[n].point;
            if c == ep {
                return Ok(());
            }
            let sign = if right { 1 } else { -1 };
            if s.orient(eq, c, ep) != sign {
                return Ok(());
            }
            let n2 = s.step(n, right)?;
            if s.orient(s.nodes[node].point, c, s.nodes[n2].point) != sign {
                return Ok(());
            }
            s.fill_concave(ep, eq, node, right)
        })
    }

    fn fill_convex(&mut self, ep: usize, eq: usize, node: usize, right: bool) -> Result<()> {
        self.guarded(|s| {
            let n = s.step(node, right)?;
            let n2 = s.step(n, right)?;
            let n3 = s.step(n2, right)?;
            let sign = if right { 1 } else { -1 };
            if s.orient(s.nodes[n].point, s.nodes[n2].point, s.nodes[n3].point) == sign {
                s.fill_concave(ep, eq, n, right)
            } else if s.orient(eq, s.nodes[n2].point, ep) == sign {
                s.fill_convex(ep, eq, n, right)
            } else {
                Ok(())
            }
        })
    }

    /// Inserts the constrained edge `ep`–`eq` by walking from triangle `t` at `point`.
    fn edge_event(&mut self, ep: usize, eq: usize, t: usize, point: usize) -> Result<()> {
        self.guarded(|s| {
            if s.side(t, ep, eq) {
                return Ok(());
            }
            let p1 = s.triangles[t].point_ccw(point)?;
            let p2 = s.triangles[t].point_cw(point)?;
            let o1 = s.orient(eq, p1, ep);
            let o2 = s.orient(eq, p2, ep);
            if o1 != 0 && o2 != 0 {
                if o1 == o2 {
                    let next = if o1 == -1 { s.triangles[t].nccw(point)? } else { s.triangles[t].ncw(point)? };
                    let next = next.ok_or_else(failed)?;
                    s.edge_event(ep, eq, next, point)
                } else {
                    s.flip_edge(ep, eq, t, point)
                }
            } else {
                let collinear = if o1 == 0 { p1 } else { p2 };
                let index = s.triangles[t].edge_index(eq, collinear).ok_or_else(failed)?;
                s.triangles[t].constrained[index] = true;
                s.edge_q = collinear;
                let i = s.triangles[t].index(point)?;
                let across = s.triangles[t].neighbors[i].ok_or_else(failed)?;
                s.edge_event(ep, collinear, across, collinear)
            }
        })
    }

    /// Whether `d` is inside the scan area of `a` between `b` and `c`.
    fn in_scan_area(&self, a: usize, b: usize, c: usize, d: usize) -> bool {
        let (pa, pd) = (self.points[a], self.points[d]);
        let cross = |q: Point| (pa[0] - q[0]) * (pd[1] - q[1]) - (pa[1] - q[1]) * (pd[0] - q[0]);
        cross(self.points[b]) < -EPSILON && cross(self.points[c]) > EPSILON
    }

    /// Point of `ot` to continue a flip scan with, from the side of `op` relative to the edge.
    fn next_flip_point(&self, ep: usize, eq: usize, ot: usize, op: usize) -> Result<usize> {
        match self.orient(eq, op, ep) {
            -1 => self.triangles[ot].point_ccw(op),
            1 => self.triangles[ot].point_cw(op),
            _ => Err(failed()),
        }
    }

    fn flip_edge(&mut self, ep: usize, eq: usize, t: usize, p: usize) -> Result<()> {
        self.guarded(|s| {
            let i = s.triangles[t].index(p)?;
            let ot = s.triangles[t].neighbors[i].ok_or_else(failed)?;
            let op = s.opposite_point(ot, t, p)?;
            let ccw = s.triangles[t].point_ccw(p)?;
            let cw = s.triangles[t].point_cw(p)?;
            if s.in_scan_area(p, ccw, cw, op) {
                s.rotate(t, p, ot, op)?;
                s.map(t)?;
                s.map(ot)?;
                if p == eq && op == ep {
                    if eq == s.edge_q && ep == s.edge_p {
                        s.side(t, ep, eq);
                        s.side(ot, ep, eq);
                        s.legalize(t)?;
                        s.legalize(ot)?;
                    }
                    return Ok(());
                }
                let o = s.orient(eq, op, ep);
                let non_crossing = if o == 1 { ot } else { t };
                let index = s.triangles[non_crossing].edge_index(p, op).ok_or_else(failed)?;
                s.triangles[non_crossing].delaunay[index] = true;
                s.legalize(non_crossing)?;
                s.triangles[non_crossing].delaunay = [false; 3];
                let next = if non_crossing == ot { t } else { ot };
                s.flip_edge(ep, eq, next, p)
            } else {
                let new_p = s.next_flip_point(ep, eq, ot, op)?;
                s.flip_scan(ep, eq, t, ot, new_p)?;
                s.edge_event(ep, eq, t, p)
            }
        })
    }

    fn flip_scan(&mut self, ep: usize, eq: usize, flip: usize, t: usize, p: usize) -> Result<()> {
        self.guarded(|s| {
            let i = s.triangles[t].index(p)?;
            let ot = s.triangles[t].neighbors[i].ok_or_else(failed)?;
            let op = s.opposite_point(ot, t, p)?;
            let ccw = s.triangles[flip].point_ccw(eq)?;
            let cw = s.triangles[flip].point_cw(eq)?;
            if s.in_scan_area(eq, ccw, cw, op) {
                s.flip_edge(eq, op, ot, op)
            } else {
                let new_p = s.next_flip_point(ep, eq, ot, op)?;
                s.flip_scan(ep, eq, flip, ot, new_p)
            }
        })
    }
}

/// Swift `points.map(\.x).min()` and friends: folds one axis with `pick(value, best)`.
fn fold(points: &[Point], axis: usize, pick: impl Fn(f64, f64) -> f64) -> f64 {
    points.iter().skip(1).fold(points[0][axis], |best, p| pick(p[axis], best))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_are_stored_at_their_upper_point() {
        let sweep = Sweep::new(vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], &[3]);
        assert_eq!(sweep.edges[0], vec![]);
        assert_eq!(sweep.edges[1], vec![(0, 1)]);
        assert_eq!(sweep.edges[2], vec![(1, 2), (0, 2)]);
    }

    #[test]
    fn orient_signs() {
        let sweep = Sweep::new(vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [2.0, 0.0]], &[4]);
        assert_eq!(sweep.orient(0, 1, 2), 1);
        assert_eq!(sweep.orient(0, 2, 1), -1);
        assert_eq!(sweep.orient(0, 1, 3), 0);
    }

    #[test]
    fn triangle_rotate_matches_poly2tri_legalize() {
        let mut t = Triangle::new([0, 1, 2]);
        t.rotate(0, 9).unwrap();
        assert_eq!(t.points, [2, 0, 9]);
    }

    #[test]
    fn triangle_runs_to_interior_triangles() {
        let triangles = Sweep::new(vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], &[3]).run().unwrap();
        assert_eq!(triangles.len(), 1);
        let mut points = triangles[0].to_vec();
        points.sort();
        assert_eq!(points, vec![0, 1, 2]);
    }
}
