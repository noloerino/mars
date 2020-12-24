use crate::arch::*;
use crate::program_state::{Data, InstResult, ProgramState};
use std::fmt::Debug;

pub trait ConcreteInst<F, S>: Debug
where
    F: ArchFamily<S>,
    S: Data,
{
    fn to_machine_code(&self) -> u32;
    fn apply(&self, state: &ProgramState<F, S>) -> InstResult<F, S>;
}
