use super::arch::*;
use super::instruction::RiscVInst;
use super::registers::RiscVRegister;
use crate::arch::*;
use crate::assembler::{Linker, ParseErrorReport, SectionStore};
use crate::instruction::*;
use crate::program_state::*;
use std::str;

pub struct RiscVProgram<T: MachineDataWidth> {
    pub insts: Vec<RiscVInst<T>>,
    pub state: ProgramState<RiscVRegister, T>,
}

impl RiscVProgram<Width32b> {
    pub const TEXT_START_32: u32 = 0x1000_0000;
    pub const STACK_START_32: u32 = 0x7FFF_FFF0;
    pub const DATA_START_32: u32 = 0x2000_0000;
}

impl Program<RiscVRegister, RiscVInst<Width32b>, Width32b> for RiscVProgram<Width32b> {
    /// Initializes a new program instance from the provided instructions.
    ///
    /// The instructions are loaded into memory at the start of the instruction section,
    /// which defaults to TEXT_START to avoid any accidental null pointer derefs.
    ///
    /// The stack pointer is initialized to STACK_START.
    ///
    /// The data given in SectionStore is used to initialize the data and rodata sections.
    ///
    /// Until paged memory is implemented, rodata is placed sequentially with data, and
    /// no guarantees on read-onliness are enforced.
    fn new(insts: Vec<RiscVInst<Width32b>>, sections: SectionStore) -> RiscVProgram<Width32b> {
        let mut state = ProgramState::new();
        let mut user_state = &mut state.user_state;
        let text_start: ByteAddr32 = RiscVProgram::TEXT_START_32.into();
        let stack_start: ByteAddr32 = RiscVProgram::STACK_START_32.into();
        user_state
            .regfile
            .set(RiscVRegister::SP, stack_start.into());
        user_state.pc = text_start;
        // store instructions
        let mut next_addr: ByteAddr32 = user_state.pc;
        for inst in &insts {
            user_state.memory.set_word(
                next_addr.to_word_address(),
                DataWord::from(inst.to_machine_code()),
            );
            next_addr = next_addr.plus_4()
        }
        // store data
        let all_data = sections.data.into_iter().chain(sections.rodata.into_iter());
        for (_offs, byte) in all_data.enumerate() {
            user_state
                .memory
                .set_byte(RiscVProgram::DATA_START_32.into(), byte.into())
        }
        RiscVProgram { insts, state }
    }

    /// Prints out all the instructions that this program contains.
    fn dump_insts(&self) {
        for inst in &self.insts {
            println!("{:?}", inst);
        }
    }

    /// Runs the program to completion, returning the value in register a0.
    fn run(&mut self) -> i32 {
        // for now, just use the instruction vec to determine the next instruction
        let pc_start: u32 = RiscVProgram::TEXT_START_32;
        // for now, if we're out of instructions just call it a day
        // if pc dipped below pc_start, panic for now is also fine
        while let Some(inst) = self.insts.get(
            ByteAddr32::from(u32::from(self.state.user_state.pc).wrapping_sub(pc_start))
                .to_word_address() as usize,
        ) {
            self.state.apply_inst(inst);
        }
        self.state.user_state.regfile.read(RiscVRegister::A0).into()
    }
    fn get_inst_vec(&self) -> &[RiscVInst<Width32b>] {
        self.insts.as_slice()
    }
    fn get_state(self) -> ProgramState<RiscVRegister, Width32b> {
        self.state
    }
}

impl str::FromStr for RiscVProgram<Width32b> {
    type Err = ParseErrorReport;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Linker::with_main_str(s).link::<RV32>()
    }
}
