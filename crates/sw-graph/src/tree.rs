/// The five kinds of point the synchronized.work tree draws (design §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Project,
    Task,
    Person,
    Agent,
    Progress,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeSpec {
    /// Stable across redraws (the point id); React Flow's node id.
    pub key: String,
    pub parent: Option<usize>,
    pub kind: NodeKind,
    pub label: String,
    /// Width and height in view units, measured by the caller's painter.
    pub size: (f32, f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphError {
    /// A parent index is out of range or not earlier in the list.
    BadParent {
        node: usize,
    },
    Empty,
    /// A size is negative or not finite.
    BadSize {
        node: usize,
    },
}

/// A forest in topological order: every node's parent comes before it, so a cycle cannot be
/// expressed at all.
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    nodes: Vec<NodeSpec>,
    children: Vec<Vec<usize>>,
    roots: Vec<usize>,
}

impl Tree {
    pub fn new(nodes: Vec<NodeSpec>) -> Result<Self, GraphError> {
        if nodes.is_empty() {
            return Err(GraphError::Empty);
        }
        let mut children = vec![Vec::new(); nodes.len()];
        let mut roots = Vec::new();
        for (i, node) in nodes.iter().enumerate() {
            let (w, h) = node.size;
            if !(w.is_finite() && h.is_finite() && w >= 0.0 && h >= 0.0) {
                return Err(GraphError::BadSize { node: i });
            }
            match node.parent {
                None => roots.push(i),
                Some(p) if p < i => children[p].push(i),
                Some(_) => return Err(GraphError::BadParent { node: i }),
            }
        }
        Ok(Self {
            nodes,
            children,
            roots,
        })
    }

    pub fn nodes(&self) -> &[NodeSpec] {
        &self.nodes
    }

    pub fn children(&self, node: usize) -> &[usize] {
        &self.children[node]
    }

    pub fn roots(&self) -> &[usize] {
        &self.roots
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
