#[derive(Debug)]
pub struct RunStats {
    pub build_ms: f64,
    pub force_ms: f64,
    pub integrate_ms: f64,
    pub peak_node_count: usize,
    pub node_capacity: usize,
    pub particle_count: usize,
    pub particle_bytes: usize,
    pub node_pool_bytes: usize,
    pub traversal_stack_bytes: usize,
}

impl RunStats {
    pub const fn zero() -> Self {
        Self {
            build_ms: 0.0,
            force_ms: 0.0,
            integrate_ms: 0.0,
            peak_node_count: 0,
            node_capacity: 0,
            particle_count: 0,
            particle_bytes: 0,
            node_pool_bytes: 0,
            traversal_stack_bytes: 0,
        }
    }

    pub fn node_utilization(&self) -> f64 {
        if self.node_capacity == 0 {
            return 0.0;
        }
        self.peak_node_count as f64 / self.node_capacity as f64
    }

    pub fn workspace_bytes(&self) -> usize {
        self.particle_bytes + self.node_pool_bytes + self.traversal_stack_bytes
    }

    pub fn bytes_per_particle(&self) -> f64 {
        if self.particle_count == 0 {
            return 0.0;
        }
        self.workspace_bytes() as f64 / self.particle_count as f64
    }

}
