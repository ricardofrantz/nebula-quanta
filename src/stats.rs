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
    pub tree_build_transient_bytes: usize,
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
            tree_build_transient_bytes: 0,
        }
    }

    pub fn node_utilization(&self) -> f64 {
        if self.node_capacity == 0 {
            return 0.0;
        }
        self.peak_node_count as f64 / self.node_capacity as f64
    }

    pub fn workspace_bytes(&self) -> usize {
        self.particle_bytes
            + self.node_pool_bytes
            + self.traversal_stack_bytes
            + self.tree_build_transient_bytes
    }

    pub fn bytes_per_particle(&self) -> f64 {
        if self.particle_count == 0 {
            return 0.0;
        }
        self.workspace_bytes() as f64 / self.particle_count as f64
    }

    pub fn total_ms(&self) -> f64 {
        self.build_ms + self.force_ms + self.integrate_ms
    }

    pub fn avg_step_ms(&self, steps: usize) -> f64 {
        if steps == 0 {
            return 0.0;
        }
        self.total_ms() / steps as f64
    }

    pub fn steps_per_sec(&self, steps: usize) -> f64 {
        let total = self.total_ms();
        if steps == 0 || total <= 0.0 {
            return 0.0;
        }
        1000.0 * steps as f64 / total
    }

    pub fn force_ns_per_particle_step(&self, steps: usize, particles: usize) -> f64 {
        let denom = (steps.saturating_mul(particles)) as f64;
        if denom == 0.0 {
            return 0.0;
        }
        self.force_ms * 1_000_000.0 / denom
    }
}
