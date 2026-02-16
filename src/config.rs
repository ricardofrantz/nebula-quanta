use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "nebula-quanta")]
pub struct Args {
    #[arg(long, default_value_t = 1000)]
    pub n: usize,
    #[arg(long, default_value_t = 100)]
    pub steps: usize,
    #[arg(long, default_value_t = 0.001)]
    pub dt: f64,
    #[arg(long, default_value_t = 0.7)]
    pub theta: f64,
    #[arg(long, default_value_t = 0.01)]
    pub epsilon: f64,
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
    #[arg(long, default_value = "barnes_hut")]
    pub mode: String,
    #[arg(long)]
    pub validate: bool,
}
