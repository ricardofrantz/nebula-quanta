#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub mass: f64,
    pub com_x: f64,
    pub com_y: f64,
    pub body_idx: i32,
    pub children: [i32; 4],
}

impl Node {
    pub fn empty() -> Self {
        Self {
            x_min: 0.0,
            x_max: 0.0,
            y_min: 0.0,
            y_max: 0.0,
            mass: 0.0,
            com_x: 0.0,
            com_y: 0.0,
            body_idx: -1,
            children: [-1; 4],
        }
    }

    pub fn with_bounds(x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Self {
        Self {
            x_min,
            x_max,
            y_min,
            y_max,
            mass: 0.0,
            com_x: 0.0,
            com_y: 0.0,
            body_idx: -1,
            children: [-1, -1, -1, -1],
        }
    }

    pub fn is_leaf(&self) -> bool {
        self.children.iter().all(|idx| *idx < 0)
    }

    pub fn size(&self) -> f64 {
        self.x_max - self.x_min
    }
}

pub struct QuadTree {
    pub nodes: Vec<Node>,
}

impl QuadTree {
    pub fn with_capacity(max_nodes: usize) -> Self {
        let mut nodes = Vec::with_capacity(max_nodes);
        nodes.push(Node::empty());
        Self { nodes }
    }

    pub fn reset(&mut self, x_min: f64, x_max: f64, y_min: f64, y_max: f64) {
        self.nodes.clear();
        self.nodes.push(Node::with_bounds(x_min, x_max, y_min, y_max));
    }

    pub fn root_bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let node = self.nodes.first()?;
        Some((node.x_min, node.x_max, node.y_min, node.y_max))
    }
}
