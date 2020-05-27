# duna
A 32-bit RISCV emulator built in Rust. Inspired by [Venus](https://github.com/ThaumicMekanism/venus).

## Current functionality
- Supports most of the base ISA (haven't implemented a few arithmetic operations yet since I'm still
figuring out the proper abstractions)
- Supports the print ecall :P
- Run by CLI with `cargo run <INPUT_FILE>`

## Roadmap - RV32I
- [ ] Implement paged memory
- [ ] Support assembler [relocation functions](https://github.com/riscv/riscv-asm-manual/blob/master/riscv-asm.md#assembler-relocation-functions)
- [ ] Run [compliance tests](https://github.com/riscv/riscv-compliance)

## Roadmap - Other ISAs?
- [ ] RISCV-64
- [ ] MIPS 32/64bit? whatever they're called?
- [ ] x86-64
- [ ] ARM
- [ ] wasm
- [ ] some LLVM IR

## Roadmap - General
- [ ] Make interface for stepping through instructions
- [ ] Display regfile, memory, and cache info
- [ ] Provide debugger support a la GDB, possibly valgrind-like tools as well?
- [ ] Implement peephole optimizations + visualizations
