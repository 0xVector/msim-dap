#[derive(Copy, Clone, Debug)]
pub enum State {
    New,
    Init,
    Config,
    Running,
}
