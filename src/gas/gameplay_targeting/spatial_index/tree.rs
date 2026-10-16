//! A balanced, mutable AABB hierarchy with stable leaf handles.

use bevy::math::{DVec3, Vec3};
use bevy::prelude::Entity;

const FAT_MARGIN: f64 = 0.5;

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: DVec3,
    max: DVec3,
}

impl Bounds {
    fn around(center: Vec3, radius: f64) -> Self {
        let center = center.as_dvec3();
        let extent = DVec3::splat(radius);
        let min = center - extent;
        let max = center + extent;

        // Outward rounding keeps broad-phase bounds conservative even when a
        // small extent is added to a very large, finite coordinate.
        Self {
            min: DVec3::new(min.x.next_down(), min.y.next_down(), min.z.next_down()),
            max: DVec3::new(max.x.next_up(), max.y.next_up(), max.z.next_up()),
        }
    }

    fn contains_point(self, point: Vec3) -> bool {
        let point = point.as_dvec3();
        point.cmpge(self.min).all() && point.cmple(self.max).all()
    }

    fn overlaps(self, other: Self) -> bool {
        self.min.cmple(other.max).all() && self.max.cmpge(other.min).all()
    }

    fn union(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    fn area(self) -> f64 {
        let extent = self.max - self.min;
        2.0 * (extent.x * extent.y + extent.y * extent.z + extent.z * extent.x)
    }
}

#[derive(Clone, Copy, Debug)]
enum NodeKind {
    Leaf(Entity),
    Branch { left: usize, right: usize },
    Free { next: Option<usize> },
}

#[derive(Debug)]
struct Node {
    bounds: Bounds,
    parent: Option<usize>,
    height: usize,
    kind: NodeKind,
}

/// A point-target broad phase whose leaves keep their handles when moved.
///
/// Insertion follows the smallest increase in bounding-box surface area. AVL
/// rotations bound tree height independently of insertion order. Every branch
/// stores the exact union of its children's bounds, while leaves retain fat
/// bounds until a point moves outside them.
#[derive(Debug, Default)]
pub(super) struct DynamicAabbTree {
    nodes: Vec<Node>,
    root: Option<usize>,
    free: Option<usize>,
}

impl DynamicAabbTree {
    /// Inserts a finite point and returns its stable leaf handle.
    pub(super) fn insert(&mut self, entity: Entity, position: Vec3) -> usize {
        debug_assert!(position.is_finite());
        let leaf = self.allocate(Node {
            bounds: Bounds::around(position, FAT_MARGIN),
            parent: None,
            height: 0,
            kind: NodeKind::Leaf(entity),
        });
        self.attach(leaf);
        leaf
    }

    /// Removes a live leaf. Its handle must not be used again after this call.
    pub(super) fn remove(&mut self, leaf: usize) {
        debug_assert!(matches!(self.nodes[leaf].kind, NodeKind::Leaf(_)));
        self.detach(leaf);
        self.release(leaf);
    }

    /// Updates a live leaf, reinserting only after it leaves its fat bounds.
    pub(super) fn update(&mut self, leaf: usize, position: Vec3) {
        debug_assert!(position.is_finite());
        debug_assert!(matches!(self.nodes[leaf].kind, NodeKind::Leaf(_)));
        if self.nodes[leaf].bounds.contains_point(position) {
            return;
        }

        self.detach(leaf);
        self.nodes[leaf].bounds = Bounds::around(position, FAT_MARGIN);
        self.attach(leaf);
    }

    /// Appends broad-phase candidates for a finite sphere to `output`.
    ///
    /// The query uses the sphere's enclosing AABB. Callers perform precise
    /// geometry checks and establish gameplay ordering after this traversal.
    pub(super) fn query(&self, origin: Vec3, radius: f32, output: &mut Vec<Entity>) {
        debug_assert!(origin.is_finite());
        debug_assert!(radius.is_finite() && radius >= 0.0);
        let query = Bounds::around(origin, f64::from(radius));
        let mut current = self.root;
        let mut previous = None;

        // Parent links encode the traversal stack without allocating scratch
        // memory. Arriving from the parent visits or prunes a subtree; returning
        // from the left child visits the right; returning from the right ascends.
        while let Some(index) = current {
            let node = &self.nodes[index];
            let next = if previous == node.parent {
                if node.bounds.overlaps(query) {
                    match node.kind {
                        NodeKind::Leaf(entity) => {
                            output.push(entity);
                            node.parent
                        }
                        NodeKind::Branch { left, .. } => Some(left),
                        NodeKind::Free { .. } => {
                            debug_assert!(false, "free nodes cannot be reachable from the root");
                            node.parent
                        }
                    }
                } else {
                    node.parent
                }
            } else if let NodeKind::Branch { left, right } = node.kind
                && previous == Some(left)
            {
                Some(right)
            } else {
                node.parent
            };
            previous = Some(index);
            current = next;
        }
    }

    fn allocate(&mut self, node: Node) -> usize {
        if let Some(index) = self.free {
            self.free = match self.nodes[index].kind {
                NodeKind::Free { next } => next,
                _ => {
                    debug_assert!(false, "the free list only contains free nodes");
                    None
                }
            };
            self.nodes[index] = node;
            index
        } else {
            let index = self.nodes.len();
            self.nodes.push(node);
            index
        }
    }

    fn release(&mut self, index: usize) {
        self.nodes[index].kind = NodeKind::Free { next: self.free };
        self.nodes[index].parent = None;
        self.free = Some(index);
    }

    fn attach(&mut self, leaf: usize) {
        let Some(mut sibling) = self.root else {
            self.root = Some(leaf);
            return;
        };
        let bounds = self.nodes[leaf].bounds;
        while let NodeKind::Branch { left, right } = self.nodes[sibling].kind {
            let left_node = &self.nodes[left];
            let right_node = &self.nodes[right];
            let left_cost = left_node.bounds.union(bounds).area() - left_node.bounds.area();
            let right_cost = right_node.bounds.union(bounds).area() - right_node.bounds.area();
            let comparison = left_cost
                .total_cmp(&right_cost)
                .then_with(|| left_node.height.cmp(&right_node.height))
                .then_with(|| left.cmp(&right));
            sibling = if comparison.is_le() { left } else { right };
        }
        debug_assert!(matches!(self.nodes[sibling].kind, NodeKind::Leaf(_)));

        let parent = self.nodes[sibling].parent;
        let branch = self.allocate(Node {
            bounds: bounds.union(self.nodes[sibling].bounds),
            parent,
            height: 1,
            kind: NodeKind::Branch {
                left: sibling,
                right: leaf,
            },
        });
        self.replace_child(parent, sibling, branch);
        self.nodes[sibling].parent = Some(branch);
        self.nodes[leaf].parent = Some(branch);
        self.refit_upwards(Some(branch));
    }

    fn detach(&mut self, leaf: usize) {
        let Some(parent) = self.nodes[leaf].parent else {
            debug_assert_eq!(self.root, Some(leaf));
            self.root = None;
            return;
        };
        let NodeKind::Branch { left, right } = self.nodes[parent].kind else {
            debug_assert!(false, "a leaf's parent must be a branch");
            return;
        };
        let sibling = if left == leaf {
            right
        } else {
            debug_assert_eq!(right, leaf);
            left
        };
        let grandparent = self.nodes[parent].parent;
        self.replace_child(grandparent, parent, sibling);
        self.nodes[leaf].parent = None;
        self.release(parent);
        self.refit_upwards(grandparent);
    }

    fn replace_child(&mut self, parent: Option<usize>, old: usize, new: usize) {
        if let Some(parent) = parent {
            let NodeKind::Branch { left, right } = self.nodes[parent].kind else {
                debug_assert!(false, "only branches can contain children");
                return;
            };
            self.nodes[parent].kind = if left == old {
                NodeKind::Branch { left: new, right }
            } else {
                debug_assert_eq!(right, old);
                NodeKind::Branch { left, right: new }
            };
        } else {
            debug_assert_eq!(self.root, Some(old));
            self.root = Some(new);
        }
        self.nodes[new].parent = parent;
    }

    fn refit(&mut self, index: usize) {
        if let NodeKind::Branch { left, right } = self.nodes[index].kind {
            self.nodes[index].bounds = self.nodes[left].bounds.union(self.nodes[right].bounds);
            self.nodes[index].height = 1 + self.nodes[left].height.max(self.nodes[right].height);
        }
    }

    fn refit_upwards(&mut self, mut current: Option<usize>) {
        while let Some(index) = current {
            self.refit(index);
            let subtree_root = self.balance(index);
            current = self.nodes[subtree_root].parent;
        }
    }

    fn balance(&mut self, index: usize) -> usize {
        let NodeKind::Branch { left, right } = self.nodes[index].kind else {
            return index;
        };
        let left_height = self.nodes[left].height;
        let right_height = self.nodes[right].height;
        if right_height > left_height + 1 {
            let NodeKind::Branch {
                left: inner,
                right: outer,
            } = self.nodes[right].kind
            else {
                debug_assert!(false, "a taller subtree must be a branch");
                return index;
            };
            if self.nodes[inner].height > self.nodes[outer].height {
                self.rotate_right(right);
            }
            self.rotate_left(index)
        } else if left_height > right_height + 1 {
            let NodeKind::Branch {
                left: outer,
                right: inner,
            } = self.nodes[left].kind
            else {
                debug_assert!(false, "a taller subtree must be a branch");
                return index;
            };
            if self.nodes[inner].height > self.nodes[outer].height {
                self.rotate_left(left);
            }
            self.rotate_right(index)
        } else {
            index
        }
    }

    fn rotate_left(&mut self, index: usize) -> usize {
        let NodeKind::Branch { left, right } = self.nodes[index].kind else {
            debug_assert!(false, "rotation requires a branch");
            return index;
        };
        let NodeKind::Branch {
            left: inner,
            right: outer,
        } = self.nodes[right].kind
        else {
            debug_assert!(false, "rotation requires a branch child");
            return index;
        };
        let parent = self.nodes[index].parent;
        self.replace_child(parent, index, right);
        self.nodes[right].kind = NodeKind::Branch {
            left: index,
            right: outer,
        };
        self.nodes[index].parent = Some(right);
        self.nodes[index].kind = NodeKind::Branch { left, right: inner };
        self.nodes[inner].parent = Some(index);
        self.refit(index);
        self.refit(right);
        right
    }

    fn rotate_right(&mut self, index: usize) -> usize {
        let NodeKind::Branch { left, right } = self.nodes[index].kind else {
            debug_assert!(false, "rotation requires a branch");
            return index;
        };
        let NodeKind::Branch {
            left: outer,
            right: inner,
        } = self.nodes[left].kind
        else {
            debug_assert!(false, "rotation requires a branch child");
            return index;
        };
        let parent = self.nodes[index].parent;
        self.replace_child(parent, index, left);
        self.nodes[left].kind = NodeKind::Branch {
            left: outer,
            right: index,
        };
        self.nodes[index].parent = Some(left);
        self.nodes[index].kind = NodeKind::Branch { left: inner, right };
        self.nodes[inner].parent = Some(index);
        self.refit(index);
        self.refit(left);
        left
    }
}
