use std::{
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap},
};

use anyhow::bail;

pub struct Dag<'a> {
    nodes: Vec<&'a str>,
    edges: Vec<(&'a str, &'a str)>,
}

impl<'a> Dag<'a> {
    /// `edges` carries `(from, to)` pairs: `from` must come before `to`
    /// in the topological order. Nodes that aren't named in `nodes` and
    /// edges referencing such nodes are rejected at sort time.
    pub fn new(nodes: Vec<&'a str>, edges: Vec<(&'a str, &'a str)>) -> Self {
        Self { nodes, edges }
    }

    pub fn topological_order(&self) -> anyhow::Result<Vec<&'a str>> {
        let index: BTreeMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (*n, i))
            .collect();
        let n = self.nodes.len();

        let mut in_deg = vec![0usize; n];
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (from, to) in &self.edges {
            let from_idx = *index
                .get(from)
                .ok_or_else(|| anyhow::anyhow!("edge references unknown node `{from}`"))?;
            let to_idx = *index
                .get(to)
                .ok_or_else(|| anyhow::anyhow!("edge references unknown node `{to}`"))?;
            adj[from_idx].push(to_idx);
            in_deg[to_idx] += 1;
        }

        // Lexicographic (Kahn's algorithm with a min-heap keyed on
        // declaration index). Among all valid topological orders we
        // emit the one closest to declaration order: at each step we
        // pop the lowest-indexed node whose predecessors are already
        // emitted. The motivation is that synthesised ordering edges
        // (from rule analysis) should perturb the read order
        // minimally, only deferring fields whose gates genuinely
        // require an earlier read.
        let mut heap: BinaryHeap<Reverse<usize>> =
            (0..n).filter(|&i| in_deg[i] == 0).map(Reverse).collect();
        let mut out = Vec::with_capacity(n);

        while let Some(Reverse(node)) = heap.pop() {
            out.push(self.nodes[node]);
            for &next in &adj[node] {
                in_deg[next] -= 1;
                if in_deg[next] == 0 {
                    heap.push(Reverse(next));
                }
            }
        }

        if out.len() != n {
            let stuck: Vec<&str> = in_deg
                .iter()
                .enumerate()
                .filter(|(_, d)| **d > 0)
                .map(|(i, _)| self.nodes[i])
                .collect();
            bail!(
                "ordering cycle detected among settings: {:?} (cannot produce a stable read/write order)",
                stuck
            );
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_edges_keeps_declaration_order() {
        let dag = Dag::new(vec!["a", "b", "c"], vec![]);
        assert_eq!(dag.topological_order().unwrap(), vec!["a", "b", "c"]);
    }

    #[test]
    fn edge_pulls_target_after_source() {
        // Edge b -> a means b must come first. Once b is emitted,
        // a (index 0) is the lowest free node -> it goes next under
        // lex order, beating c (index 2).
        let dag = Dag::new(vec!["a", "b", "c"], vec![("b", "a")]);
        assert_eq!(dag.topological_order().unwrap(), vec!["b", "a", "c"]);
    }

    #[test]
    fn cycle_is_reported() {
        let dag = Dag::new(vec!["a", "b"], vec![("a", "b"), ("b", "a")]);
        let err = dag.topological_order().unwrap_err().to_string();
        assert!(err.contains("ordering cycle"), "got: {err}");
    }

    #[test]
    fn unknown_node_in_edge_is_reported() {
        let dag = Dag::new(vec!["a", "b"], vec![("a", "ghost")]);
        let err = dag.topological_order().unwrap_err().to_string();
        assert!(err.contains("unknown node"), "got: {err}");
    }

    #[test]
    fn neighbour_order_stable_within_indegree_zero() {
        // c has an edge a -> c. b and a are both in-degree zero;
        // declaration order should be preserved.
        let dag = Dag::new(vec!["a", "b", "c"], vec![("a", "c")]);
        assert_eq!(dag.topological_order().unwrap(), vec!["a", "b", "c"]);
    }

    #[test]
    fn lex_toposort_emits_freed_node_immediately_when_index_is_low() {
        let dag = Dag::new(vec!["a", "x", "b", "c"], vec![("a", "x")]);
        assert_eq!(
            dag.topological_order().unwrap(),
            vec!["a", "x", "b", "c"],
            "freed node with lowest index must come before later already-root nodes"
        );
    }

    #[test]
    fn lex_toposort_only_defers_when_edge_forces() {
        let dag = Dag::new(vec!["a", "b", "c", "d", "e"], vec![("d", "b")]);
        assert_eq!(
            dag.topological_order().unwrap(),
            vec!["a", "c", "d", "b", "e"]
        );
    }
}
