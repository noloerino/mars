use super::datatypes::*;
use super::memory::Memory;
use super::registers::{IRegister, RegFile};
use crate::arch::*;
use crate::assembler::SectionStore;
use crate::instruction::ConcreteInst;

pub trait Program<F, T>
where
    F: ArchFamily<T>,
    T: MachineDataWidth,
{
    fn new(insts: Vec<F::Instruction>, sections: SectionStore) -> Self;
    /// Prints out all the instructions that this program contains.
    fn dump_insts(&self);
    /// Runs the program to completion, returning an exit code.
    fn run(&mut self) -> i32;

    /// Returns the instructions provided to this program.
    fn get_inst_vec(&self) -> &[F::Instruction];

    /// Returns the current state of this program.
    fn get_state(self) -> ProgramState<F, T>;
}

pub struct ProgramState<F: ArchFamily<T>, T: MachineDataWidth> {
    pub(crate) priv_state: PrivProgState,
    pub(crate) user_state: UserProgState<F, T>,
}

impl<F: ArchFamily<T>, T: MachineDataWidth> Default for ProgramState<F, T> {
    fn default() -> Self {
        ProgramState::new()
    }
}

/// TODO put custom types for syscall args
/// TODO put errno on user state at a thread-local statically known location
impl<F: ArchFamily<T>, T: MachineDataWidth> ProgramState<F, T> {
    pub fn get_stdout(&self) -> &[u8] {
        self.priv_state.stdout.as_slice()
    }

    pub fn get_user_pc(&self) -> T::ByteAddr {
        self.user_state.pc
    }

    pub fn set_user_pc(&mut self, addr: <T>::ByteAddr) {
        self.user_state.pc = addr
    }

    pub fn regfile_read(&self, reg: F::Register) -> <T>::RegData {
        self.user_state.regfile.read(reg)
    }

    pub fn regfile_set(&mut self, reg: F::Register, val: <T>::RegData) {
        self.user_state.regfile.set(reg, val);
    }

    pub fn memory_get_word(&self, addr: <<T>::ByteAddr as ByteAddress>::WordAddress) -> DataWord {
        self.user_state.memory.get_word(addr)
    }

    pub fn memory_set_word(
        &mut self,
        addr: <<T>::ByteAddr as ByteAddress>::WordAddress,
        val: DataWord,
    ) {
        self.user_state.memory.set_word(addr, val);
    }

    pub fn handle_trap(&self, trap_kind: &TrapKind) -> PrivStateChange<T> {
        use TrapKind::*;
        match trap_kind {
            Ecall => self.dispatch_syscall(),
            _ => unimplemented!(),
        }
    }

    pub fn dispatch_syscall(&self) -> PrivStateChange<T> {
        let rf = &self.user_state.regfile;
        let syscall_number_reg = <F::Syscalls as SyscallConvention<F, T>>::syscall_number_reg();
        let arg_regs = <F::Syscalls as SyscallConvention<F, T>>::syscall_arg_regs();
        let a0 = rf.read(arg_regs[0]);
        let a1 = rf.read(arg_regs[1]);
        let a2 = rf.read(arg_regs[2]);
        if let Some(nr) = <F::Syscalls as SyscallConvention<F, T>>::number_to_syscall(
            self.user_state.regfile.read(syscall_number_reg).into(),
        ) {
            match nr {
                Syscall::Write => self.syscall_write(a0, a1.into(), a2),
                _ => self.syscall_unknown(),
            }
        } else {
            self.syscall_unknown()
        }
    }

    /// Writes contents to a specified file descriptor.
    /// TODO for now, this is hardcoded to print to stdout regardless of the provided FD.
    /// * fd - file descriptor
    /// * buf - pointer to the buffer to be written
    /// * len - the number of bytes to write
    fn syscall_write(
        &self,
        fd: T::RegData,
        buf: T::ByteAddr,
        len: T::RegData,
    ) -> PrivStateChange<T> {
        PrivStateChange::FileWrite { fd, buf, len }
    }

    /// Handles an unknown syscall.
    fn syscall_unknown(&self) -> PrivStateChange<T> {
        panic!("Unknown syscall")
    }

    pub fn new() -> ProgramState<F, T> {
        ProgramState {
            priv_state: PrivProgState::new(),
            user_state: UserProgState::new(),
        }
    }

    pub fn apply_inst(&mut self, inst: &F::Instruction) {
        self.apply_diff(&inst.apply(self));
    }

    /// Performs the described operation.
    /// The privileged operation is applied first, followed by the user operation.
    pub fn apply_diff(&mut self, diff: &InstResult<F, T>) {
        match diff {
            InstResult::Trap(trap_kind) => {
                let priv_diff = &self.handle_trap(trap_kind);
                let user_diff = self.priv_state.apply_diff(&self.user_state, priv_diff);
                self.user_state.apply_diff(&user_diff);
            }
            InstResult::UserStateChange(user_diff) => self.user_state.apply_diff(&user_diff),
        };
    }

    /// Reverts the described operation.
    /// Since the privileged diff is applied first during execution, the user diff should
    /// be applied first during a revert.
    /// TODO figure out how to implement that...
    pub fn revert_diff(&mut self, diff: &ProgramDiff<F, T>) {
        match diff {
            ProgramDiff::UserOnly(user_only) => self.user_state.revert_diff(user_only),
            ProgramDiff::PrivOnly(priv_only) => {
                self.priv_state.revert_diff(&self.user_state, priv_only)
            }
        }
    }
}

pub trait SyscallConvention<F: ArchFamily<T>, T: MachineDataWidth> {
    /// Returns the syscall identified by number N, or none if no such syscall exists.
    fn number_to_syscall(n: T::Signed) -> Option<Syscall>;
    /// Returns the number corresponding to the syscall, or -1 if it is unimplemented.
    fn syscall_to_number(syscall: Syscall) -> T::RegData;
    /// Returns which register is used to pass the syscall number.
    fn syscall_number_reg() -> F::Register;
    /// Returns which registers are used to pass arguments to syscalls.
    fn syscall_arg_regs() -> Vec<F::Register>;
    /// Returns which registers are used to return arguments from syscalls.
    fn syscall_return_regs() -> Vec<F::Register>;
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Syscall {
    Read,
    Write,
    Open,
    Close,
}

/// Contains program state that is visited only to privileged entities, i.e. a kernel thread.
/// TODO add kernel thread information (tid, file descriptors, etc.)
pub struct PrivProgState {
    /// Holds the contents of all bytes that have been printed to stdout (used mostly for testing)
    stdout: Vec<u8>,
    // file_descriptors: Vec<Vec<u8>>
}

impl Default for PrivProgState {
    fn default() -> Self {
        PrivProgState::new()
    }
}

impl PrivProgState {
    pub fn new() -> PrivProgState {
        PrivProgState { stdout: Vec::new() }
    }

    pub fn apply_diff<F: ArchFamily<T>, T: MachineDataWidth>(
        &mut self,
        user_state: &UserProgState<F, T>,
        diff: &PrivStateChange<T>,
    ) -> UserDiff<F, T> {
        use PrivStateChange::*;
        match diff {
            NoChange => UserDiff::noop(user_state),
            FileWrite { fd: _, buf, len } => {
                let memory = &user_state.memory;
                let len_val: T::Unsigned = (*len).into();
                let count: usize = T::usgn_to_usize(len_val);
                let base_addr: T::Unsigned = (*buf).into();
                let bytes: Vec<u8> = (0..count)
                    .map(|i| u8::from(memory.get_byte((base_addr + T::usize_to_usgn(i)).into())))
                    .collect();
                // TODO impl for other files
                print!("{}", String::from_utf8_lossy(&bytes));
                self.stdout.extend(bytes);
                // TODO parameterize priv state over R as well
                let ret_reg = <F::Syscalls as SyscallConvention<F, T>>::syscall_return_regs()[0];
                UserDiff::reg_write_pc_p4(user_state, ret_reg, *len)
            }
            Exit => unimplemented!(),
        }
    }

    /// Reverts a privileged state change.
    /// The originally produced UserOnly diff MUST have already been applied.
    pub fn revert_diff<F: ArchFamily<T>, T: MachineDataWidth>(
        &mut self,
        _user_state: &UserProgState<F, T>,
        diff: &PrivStateChange<T>,
    ) {
        use PrivStateChange::*;
        match diff {
            NoChange => {}
            // TODO delete last len bytes from fd
            FileWrite {
                fd: _,
                buf: _,
                len: _,
            } => {}
            _ => unimplemented!(),
        }
    }
}

/// Contains program state that is visible to the user.
pub struct UserProgState<F: ArchFamily<T>, T: MachineDataWidth> {
    pub pc: T::ByteAddr,
    pub regfile: RegFile<F::Register, T>,
    pub memory: Memory<T>,
}

impl<F: ArchFamily<T>, T: MachineDataWidth> Default for UserProgState<F, T> {
    fn default() -> Self {
        UserProgState::new()
    }
}

impl<F: ArchFamily<T>, T: MachineDataWidth> UserProgState<F, T> {
    pub fn new() -> UserProgState<F, T> {
        UserProgState {
            pc: T::sgn_zero().into(),
            regfile: RegFile::new(),
            memory: Memory::new(),
        }
    }

    pub fn apply_diff(&mut self, diff: &UserDiff<F, T>) {
        self.pc = diff.pc.new_pc;
        if let Some(RegDiff {
            reg,
            val: RegDataChange { new_value, .. },
        }) = diff.reg
        {
            self.regfile.set(reg, new_value);
        }
        if let Some(MemDiff {
            addr,
            val: WordChange { new_value, .. },
        }) = diff.mem
        {
            self.memory.set_word(addr, new_value);
        }
    }

    pub fn revert_diff(&mut self, diff: &UserDiff<F, T>) {
        self.pc = diff.pc.old_pc;
        if let Some(RegDiff {
            reg,
            val: RegDataChange { old_value, .. },
        }) = diff.reg
        {
            self.regfile.set(reg, old_value);
        }
        if let Some(MemDiff {
            addr,
            val: WordChange { old_value, .. },
        }) = diff.mem
        {
            self.memory.set_word(addr, old_value);
        }
    }
}

#[derive(Copy, Clone)]
struct RegDataChange<T: MachineDataWidth> {
    old_value: T::RegData,
    new_value: T::RegData,
}

#[derive(Copy, Clone)]
struct WordChange {
    old_value: DataWord,
    new_value: DataWord,
}

/// A change to the program counter.
struct PcDiff<T: MachineDataWidth> {
    old_pc: T::ByteAddr,
    new_pc: T::ByteAddr,
}

/// A change to a register.
struct RegDiff<R: IRegister, T: MachineDataWidth> {
    reg: R,
    val: RegDataChange<T>,
}

/// A change to memory.
struct MemDiff<T: MachineDataWidth> {
    addr: <T::ByteAddr as ByteAddress>::WordAddress,
    val: WordChange,
}

/// Represents the type of trap being raised from user mode.
/// See "Machine Cause Register" in the RISCV privileged spec for details.
pub enum TrapKind {
    /// Corresponds to an ecall instruction issued from user mode.
    Ecall,
    /// TODO implement paged memory
    PageFault,
}

/// Encodes a change that occurs within the user space of a program, which entails a write to the
/// PC and possibly a register or memory operation.
pub enum InstResult<F: ArchFamily<T>, T: MachineDataWidth> {
    Trap(TrapKind),
    UserStateChange(UserDiff<F, T>),
}

/// Represents a diff as it is applied to a program.
pub enum ProgramDiff<F: ArchFamily<T>, T: MachineDataWidth> {
    PrivOnly(PrivStateChange<T>),
    UserOnly(UserDiff<F, T>),
}

#[derive(Copy, Clone)]
/// Encodes a change that occurred to the state of the privileged aspects of a program,
/// such as a write to a file.
pub enum PrivStateChange<T: MachineDataWidth> {
    /// Indicates that the program should terminate.
    /// TODO implement exit codes
    Exit,
    NoChange,
    /// Represents a file write.
    /// * fd: the file descriptor
    /// * buf: user pointer to the data written
    /// * len: the number of bytes written
    FileWrite {
        fd: T::RegData,
        buf: T::ByteAddr,
        len: T::RegData,
    },
}

/// Represents a diff that is applied only to the user state of a program.
pub struct UserDiff<F: ArchFamily<T>, T: MachineDataWidth> {
    pc: PcDiff<T>,
    reg: Option<RegDiff<F::Register, T>>,
    mem: Option<MemDiff<T>>,
}

impl<F: ArchFamily<T>, T: MachineDataWidth> UserDiff<F, T> {
    pub fn into_inst_result(self) -> InstResult<F, T> {
        InstResult::UserStateChange(self)
    }

    fn new(
        state: &UserProgState<F, T>,
        new_pc: T::ByteAddr,
        reg_change: Option<RegDiff<F::Register, T>>,
        mem_change: Option<MemDiff<T>>,
    ) -> Self {
        UserDiff {
            pc: PcDiff {
                old_pc: state.pc,
                new_pc,
            },
            reg: reg_change,
            mem: mem_change,
        }
    }

    fn new_pc_p4(
        state: &UserProgState<F, T>,
        reg_change: Option<RegDiff<F::Register, T>>,
        mem_change: Option<MemDiff<T>>,
    ) -> Self {
        UserDiff::new(state, state.pc.plus_4(), reg_change, mem_change)
    }

    pub fn noop(state: &UserProgState<F, T>) -> Self {
        UserDiff::new_pc_p4(state, None, None)
    }

    pub fn pc_update_op(state: &UserProgState<F, T>, new_pc: T::ByteAddr) -> Self {
        UserDiff::new(state, new_pc, None, None)
    }

    pub fn reg_write_op(
        state: &UserProgState<F, T>,
        new_pc: T::ByteAddr,
        reg: F::Register,
        val: T::RegData,
    ) -> Self {
        UserDiff::new(
            state,
            new_pc,
            Some(RegDiff {
                reg,
                val: RegDataChange {
                    old_value: state.regfile.read(reg),
                    new_value: val,
                },
            }),
            None,
        )
    }

    pub fn reg_write_pc_p4(state: &UserProgState<F, T>, reg: F::Register, val: T::RegData) -> Self {
        UserDiff::reg_write_op(state, state.pc.plus_4(), reg, val)
    }

    /// Performs a memory write operation.
    /// This may trap to the OS in the event of exceptional events like a page fault.
    pub fn mem_write_op(
        state: &UserProgState<F, T>,
        addr: <T::ByteAddr as ByteAddress>::WordAddress,
        val: DataWord,
    ) -> Self {
        UserDiff::new_pc_p4(
            state,
            None,
            Some(MemDiff {
                addr,
                val: WordChange {
                    old_value: state.memory.get_word(addr),
                    new_value: val,
                },
            }),
        )
    }
}
