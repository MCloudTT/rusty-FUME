#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum State {
    S0,
    CONNECT,
    ADDING,
    MUTATION,
    SEND,
    Sf,
}
