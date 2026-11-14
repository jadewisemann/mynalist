//! A batch of ops applied to a tree, with their inverses. Port of the `Tx` class in `packages/core/src/commands.ts`.

use crate::ids::new_id;
use crate::tree::{NodeId, Op, Props, Tree};

pub struct Tx<'a> {
    pub tree: &'a mut Tree,
    pub ts: i64,
    pub ops: Vec<Op>,
    /// Inverse ops in undo order.
    pub inverse: Vec<Op>,
}

impl<'a> Tx<'a> {
    pub fn new(tree: &'a mut Tree, ts: i64) -> Self {
        Tx { tree, ts, ops: Vec::new(), inverse: Vec::new() }
    }

    pub fn apply(&mut self, op: Op) -> bool {
        match self.tree.apply(&op) {
            Some(inv) => {
                self.ops.push(op);
                self.inverse.insert(0, inv);
                true
            }
            None => false,
        }
    }

    pub fn create(&mut self, parent: &str, after: Option<&str>, props: Props) -> NodeId {
        self.create_with_id(parent, after, props, new_id())
    }

    pub fn create_with_id(&mut self, parent: &str, after: Option<&str>, props: Props, id: NodeId) -> NodeId {
        self.apply(Op::Create {
            id: id.clone(),
            parent: parent.to_string(),
            after: after.map(str::to_string),
            props,
            ts: self.ts,
        });
        id
    }

    pub fn set(&mut self, id: &str, props: Props) -> bool {
        self.apply(Op::Set { id: id.to_string(), props, ts: self.ts })
    }

    pub fn move_to(&mut self, id: &str, parent: Option<&str>, after: Option<&str>) -> bool {
        self.apply(Op::Move {
            id: id.to_string(),
            parent: parent.map(str::to_string),
            after: after.map(str::to_string),
            ts: self.ts,
        })
    }

    pub fn collapse(&mut self, id: &str, v: bool) -> bool {
        self.apply(Op::Collapse { id: id.to_string(), v })
    }

    /// Undo everything done in this tx (used when a command bails out midway).
    pub fn rollback(&mut self) {
        for op in std::mem::take(&mut self.inverse) {
            self.tree.apply(&op);
        }
        self.ops.clear();
    }
}
