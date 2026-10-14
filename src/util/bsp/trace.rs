use super::lumps::*;
use std::cell::RefCell;

// Stack entry for the iterative BSP traversal.
struct StackEntry {
    node_idx: i32,
    t_start: f32,
    t_end: f32,
}

/// Per-thread buffers reused by every trace, so a trace allocates nothing.
#[derive(Default)]
struct TraceScratch {
    stack: Vec<StackEntry>,
    // A brush is listed in every leaf it touches; brushes whose stamp equals the current trace's
    // stamp were already tested by this trace (testing one again cannot change the result).
    brush_stamps: Vec<u32>,
    stamp: u32,
}

thread_local! {
    static TRACE_SCRATCH: RefCell<TraceScratch> = RefCell::new(TraceScratch::default());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SightlineStatus {
    Clear,
    BlockedByOpaqueWall,
    BlockedByTransparentWindow,
    BlockedByGrate,
}

#[derive(Clone, Debug, Default)]
pub struct BspCollisionData {
    pub planes: Vec<Plane>,
    pub nodes: Vec<Node>,
    pub leafs: Vec<Leaf>,
    pub leafbrushes: Vec<u16>,
    pub brushes: Vec<Brush>,
    pub brushsides: Vec<BrushSide>,
    pub texinfo: Vec<TexInfo>,
    pub texdata: Vec<TexData>,
    pub material_names: Vec<String>,
}

impl BspCollisionData {
    /// Determines whether a brush represents a transparent window/glass, a grate, or an opaque solid.
    fn classify_brush(&self, brush: &Brush, hit_side_idx: Option<usize>) -> SightlineStatus {
        // 1. Check for wire fence / grate
        if (brush.contents & CONTENTS_GRATE) != 0 {
            return SightlineStatus::BlockedByGrate;
        }

        // 2. Check for glass window content flag
        if (brush.contents & CONTENTS_WINDOW) != 0 {
            return SightlineStatus::BlockedByTransparentWindow;
        }

        // 3. Inspect surface flags and texture name of the hit brushside
        if let Some(side_idx) = hit_side_idx {
            if side_idx < self.brushsides.len() {
                let side = &self.brushsides[side_idx];
                let texinfo_idx = side.texinfo as usize;
                if texinfo_idx < self.texinfo.len() {
                    let ti = &self.texinfo[texinfo_idx];
                    if (ti.flags & SURF_TRANS) != 0 {
                        return SightlineStatus::BlockedByTransparentWindow;
                    }

                    let texdata_idx = ti.texdata as usize;
                    if texdata_idx < self.texdata.len() {
                        let td = &self.texdata[texdata_idx];
                        let str_id = td.name_string_table_id as usize;
                        if str_id < self.material_names.len() {
                            let mat = self.material_names[str_id].to_ascii_lowercase();
                            if mat.contains("glass") || mat.contains("window") {
                                return SightlineStatus::BlockedByTransparentWindow;
                            }
                            if mat.contains("grate") || mat.contains("fence") {
                                return SightlineStatus::BlockedByGrate;
                            }
                        }
                    }
                }
            }
        }

        // 4. Solid or opaque
        if (brush.contents & (CONTENTS_SOLID | CONTENTS_OPAQUE | CONTENTS_PLAYERCLIP)) != 0 {
            return SightlineStatus::BlockedByOpaqueWall;
        }

        SightlineStatus::Clear
    }

    /// Traces a ray from start to end through the BSP tree, checking for wall occlusion.
    pub fn trace_sightline(&self, start: [f32; 3], end: [f32; 3]) -> SightlineStatus {
        if self.nodes.is_empty() {
            return SightlineStatus::Clear;
        }

        TRACE_SCRATCH.with(|scratch| self.trace_sightline_with(start, end, &mut scratch.borrow_mut()))
    }

    fn trace_sightline_with(&self, start: [f32; 3], end: [f32; 3], scratch: &mut TraceScratch) -> SightlineStatus {
        scratch.stamp = scratch.stamp.wrapping_add(1);
        if scratch.stamp == 0 {
            scratch.brush_stamps.fill(0);
            scratch.stamp = 1;
        }
        if scratch.brush_stamps.len() < self.brushes.len() {
            scratch.brush_stamps.resize(self.brushes.len(), 0);
        }
        let stamp = scratch.stamp;
        let brush_stamps = &mut scratch.brush_stamps;

        // Stack-based iterative traversal to avoid recursion overhead
        let stack = &mut scratch.stack;
        stack.clear();
        stack.push(StackEntry {
            node_idx: 0,
            t_start: 0.0,
            t_end: 1.0,
        });

        let mut earliest_hit_frac = 1.0f32;
        let mut earliest_hit_status = SightlineStatus::Clear;

        while let Some(entry) = stack.pop() {
            if entry.t_start >= earliest_hit_frac {
                continue;
            }

            if entry.node_idx < 0 {
                // Leaf reached
                let leaf_idx = (-entry.node_idx - 1) as usize;
                if leaf_idx < self.leafs.len() {
                    let leaf = &self.leafs[leaf_idx];

                    // Check all brushes in this leaf
                    let start_b = leaf.first_leafbrush as usize;
                    let end_b = start_b + leaf.num_leafbrushes as usize;

                    for lb_idx in start_b..end_b {
                        if lb_idx >= self.leafbrushes.len() {
                            break;
                        }
                        let b_idx = self.leafbrushes[lb_idx] as usize;
                        if b_idx >= self.brushes.len() {
                            continue;
                        }
                        if brush_stamps[b_idx] == stamp {
                            continue;
                        }
                        brush_stamps[b_idx] = stamp;

                        let brush = &self.brushes[b_idx];
                        if (brush.contents & (CONTENTS_SOLID | CONTENTS_WINDOW | CONTENTS_OPAQUE | CONTENTS_GRATE)) == 0 {
                            continue;
                        }

                        // Clip ray against convex brush planes
                        let mut enter_frac = -1.0f32;
                        let mut exit_frac = 1.0f32;
                        let mut hit_side_idx = None;
                        let mut starts_out = false;

                        let first_side = brush.first_side as usize;
                        let num_sides = brush.num_sides as usize;

                        for s in 0..num_sides {
                            let side_idx = first_side + s;
                            if side_idx >= self.brushsides.len() {
                                break;
                            }
                            let side = &self.brushsides[side_idx];
                            let plane_idx = side.planenum as usize;
                            if plane_idx >= self.planes.len() {
                                continue;
                            }
                            let plane = &self.planes[plane_idx];

                            let d0 = start[0] * plane.normal[0]
                                + start[1] * plane.normal[1]
                                + start[2] * plane.normal[2]
                                - plane.dist;
                            let d1 = end[0] * plane.normal[0]
                                + end[1] * plane.normal[1]
                                + end[2] * plane.normal[2]
                                - plane.dist;

                            if d0 > 0.0 {
                                starts_out = true;
                            }

                            // If both are in front of plane, ray misses brush
                            if d0 > 0.0 && d1 > 0.0 {
                                enter_frac = 1.0;
                                exit_frac = -1.0;
                                break;
                            }
                            // If both are behind plane, continue
                            if d0 <= 0.0 && d1 <= 0.0 {
                                continue;
                            }

                            if d0 > d1 {
                                // Entering brush
                                let f = (d0 - 0.03125) / (d0 - d1);
                                if f > enter_frac {
                                    enter_frac = f;
                                    hit_side_idx = Some(side_idx);
                                }
                            } else {
                                // Exiting brush
                                let f = (d0 + 0.03125) / (d0 - d1);
                                if f < exit_frac {
                                    exit_frac = f;
                                }
                            }
                        }

                        if starts_out && enter_frac < exit_frac && enter_frac >= 0.0 && enter_frac < earliest_hit_frac {
                            earliest_hit_frac = enter_frac;
                            earliest_hit_status = self.classify_brush(brush, hit_side_idx);
                            if earliest_hit_status == SightlineStatus::BlockedByOpaqueWall {
                                // Opaque wall stops sightline completely
                                return SightlineStatus::BlockedByOpaqueWall;
                            }
                        }
                    }
                }
                continue;
            }

            let node_idx = entry.node_idx as usize;
            if node_idx >= self.nodes.len() {
                continue;
            }
            let node = &self.nodes[node_idx];
            let plane_idx = node.plane_num as usize;
            if plane_idx >= self.planes.len() {
                continue;
            }
            let plane = &self.planes[plane_idx];

            let p_start = [
                start[0] + entry.t_start * (end[0] - start[0]),
                start[1] + entry.t_start * (end[1] - start[1]),
                start[2] + entry.t_start * (end[2] - start[2]),
            ];
            let p_end = [
                start[0] + entry.t_end * (end[0] - start[0]),
                start[1] + entry.t_end * (end[1] - start[1]),
                start[2] + entry.t_end * (end[2] - start[2]),
            ];

            let d0 = p_start[0] * plane.normal[0]
                + p_start[1] * plane.normal[1]
                + p_start[2] * plane.normal[2]
                - plane.dist;
            let d1 = p_end[0] * plane.normal[0]
                + p_end[1] * plane.normal[1]
                + p_end[2] * plane.normal[2]
                - plane.dist;

            if d0 >= 0.0 && d1 >= 0.0 {
                // Entire segment is in front of plane
                stack.push(StackEntry {
                    node_idx: node.children[0],
                    t_start: entry.t_start,
                    t_end: entry.t_end,
                });
            } else if d0 < 0.0 && d1 < 0.0 {
                // Entire segment is behind plane
                stack.push(StackEntry {
                    node_idx: node.children[1],
                    t_start: entry.t_start,
                    t_end: entry.t_end,
                });
            } else {
                // Segment crosses plane
                let (near_child, far_child) = if d0 >= 0.0 {
                    (node.children[0], node.children[1])
                } else {
                    (node.children[1], node.children[0])
                };

                let frac = (d0 / (d0 - d1)).clamp(0.0, 1.0);
                let t_mid = entry.t_start + frac * (entry.t_end - entry.t_start);

                // Push far child first, then near child (so near child pops and executes first)
                stack.push(StackEntry {
                    node_idx: far_child,
                    t_start: t_mid,
                    t_end: entry.t_end,
                });
                stack.push(StackEntry {
                    node_idx: near_child,
                    t_start: entry.t_start,
                    t_end: t_mid,
                });
            }
        }

        earliest_hit_status
    }
}
