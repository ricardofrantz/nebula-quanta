#[derive(Debug)]
pub struct RunStats {
    pub build_ms: f64,
    pub force_ms: f64,
    pub integrate_ms: f64,
    pub peak_node_count: usize,
    pub node_capacity: usize,
}

impl RunStats {
    pub const fn zero() -> Self {
        Self {
            build_ms: 0.0,
            force_ms: 0.0,
            integrate_ms: 0.0,
            peak_node_count: 0,
            node_capacity: 0,
        }
    }
}
