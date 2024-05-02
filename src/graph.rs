use crate::{
    addrlib::AddressBins,
    common::Id,
    diffs::DiffLists,
    OffsetLists,
};
use anyhow::Context as _;
use nonmax::NonMaxU32;
use petgraph::{
    graph::{
        self,
        IndexType,
        NodeIndex,
    },
    visit::{
        Bfs,
        IntoNodeIdentifiers as _,
    },
    Undirected,
};

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

type Node = Option<Id>;

#[derive(Default)]
pub struct Graph(graph::Graph<Node, (), Undirected, Ix>);

impl Graph {
    pub fn add_node(&mut self) -> NodeIndex<Ix> {
        self.0.add_node(None)
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
                        if let Some(id) = node {
                            if id != offset_id {
                                anyhow::bail!("attempted to assign id '{offset_id}' from bin '{version}' to offset '{offset}', but an id is already assigned ({id})",);
                            }
                        } else {
                            *node = Some(*offset_id);
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
            if self.0[node_id].is_none() {
                let id = initial_id;
                initial_id = initial_id.next();
                let mut bfs = Bfs::new(&self.0, node_id);
                while let Some(node_id) = bfs.next(&self.0) {
                    let node = &mut self.0[node_id];
                    if node.is_some() {
                        anyhow::bail!(
                            "attempted to assign an id to an offset, but an id is already assigned"
                        );
                    } else {
                        *node = Some(id);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get(&self, key: NodeIndex<Ix>) -> Id {
        self.0[key].expect("expected id to already be initialized upon access")
    }
}
