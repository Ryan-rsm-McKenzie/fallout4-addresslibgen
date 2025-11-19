use crate::{OffsetLists, addrlib::AddressBins, common::Id, diffs::DiffLists};
use anyhow::Context as _;
use nonmax::NonMaxU32;
use petgraph::{
    Undirected,
    graph::{self, IndexType, NodeIndex},
    visit::{Bfs, IntoNodeIdentifiers as _},
};
use smallvec::SmallVec;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Ix(NonMaxU32);

unsafe impl IndexType for Ix {
    fn new(x: usize) -> Self {
        // SAFETY: IndexType::max ensures the maximum value is never present here
        #[allow(clippy::cast_possible_truncation)]
        let inner = unsafe { NonMaxU32::new_unchecked(x as _) };
        Self(inner)
    }

    fn index(&self) -> usize {
        self.0.get() as _
    }

    fn max() -> Self {
        Self(NonMaxU32::MAX)
    }
}

// Different versions/runs/inputs to the diff calculator could end up producing different offset
// matches such that we end up in a situation where 2 offsets that didn't previously match (and
// thus had different ids assigned) end up matching in the future. In order to handle this
// situation, we use an array where the last element is the latest id assigned to those offset
// connections, from the most recent address bin.
type Node = SmallVec<[Id; 1]>;

#[derive(Default)]
pub struct Graph(graph::Graph<Node, (), Undirected, Ix>);

impl Graph {
    pub fn add_node(&mut self) -> NodeIndex<Ix> {
        self.0.add_node(Default::default())
    }

    pub fn add_edges(
        &mut self,
        offset_lists: &OffsetLists,
        diff_lists: &DiffLists,
    ) -> anyhow::Result<()> {
        println!("adding graph edges...");

        macro_rules! get_offsets {
            ($version:expr) => {
                offset_lists.get($version).with_context(|| {
                    format!(
                        "found diff for version '{}', but no corresponding offset info",
                        $version
                    )
                })
            };
        }

        macro_rules! get_ix {
            ($offsets:expr, $offset:expr, $version:expr) => {
                $offsets.get($offset).map(|x| x.ix)
            };
        }

        for diff_list in diff_lists.iter() {
            let left_offsets = get_offsets!(diff_list.left)?;
            let right_offsets = get_offsets!(diff_list.right)?;
            for diff in diff_list.iter() {
                if let Some(left_node) = get_ix!(left_offsets, diff.left, diff_list.left) {
                    if let Some(right_node) = get_ix!(right_offsets, diff.right, diff_list.right) {
                        self.0.add_edge(left_node, right_node, ());
                    }
                }
            }
        }

        Ok(())
    }

    pub fn seed_ids(
        &mut self,
        offset_lists: &OffsetLists,
        address_bins: &AddressBins,
    ) -> anyhow::Result<()> {
        println!("seeding ids...");

        for (version, address_bin) in address_bins.iter() {
            let offset_list = offset_lists.get(*version).with_context(|| {
                format!(
                    "found address bin for version '{version}', but no corressponding offset info"
                )
            })?;
            for (offset_id, offset) in address_bin.iter() {
                if let Some(root_id) = offset_list.get(*offset).map(|x| x.ix) {
                    let mut bfs = Bfs::new(&self.0, root_id);
                    while let Some(node_id) = bfs.next(&self.0) {
                        let node = &mut self.0[node_id];
                        if node.last().is_none_or(|id| id != offset_id) {
                            node.push(*offset_id);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn assign_all_ids(&mut self, mut initial_id: Id) -> anyhow::Result<()> {
        println!("assigning ids to all offsets...");

        for node_id in self.0.node_identifiers() {
            if self.0[node_id].is_empty() {
                let id = initial_id;
                initial_id = initial_id.next();
                let mut bfs = Bfs::new(&self.0, node_id);
                while let Some(node_id) = bfs.next(&self.0) {
                    let node = &mut self.0[node_id];
                    if let Some(current_id) = node.last() {
                        anyhow::bail!(
                            "attempted to assign an id '{id}' to an offset, but an id '{current_id}' is already assigned"
                        );
                    } else {
                        node.push(id);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get(&self, key: NodeIndex<Ix>) -> Id {
        self.0[key]
            .last()
            .copied()
            .expect("expected id to already be initialized upon access")
    }
}
