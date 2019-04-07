use bitvec::Bits;
use rand::Rng;

#[macro_use]
mod opcode;
use opcode::Opcode;

mod memory;

mod display;
use display::Sprite;

mod timer;

pub struct Cpu {
    gpr: [u8; 16],
    program_counter: usize,
    index: usize,
    stack_pointer: usize,
    memory: memory::Memory,
    display: display::Display,
    rng: rand::rngs::ThreadRng,
    delay_timer: timer::Timer,
}

impl Cpu {
    pub fn new(rom: Vec<u8>) -> Cpu {
        Cpu {
            gpr: [0; 16],
            program_counter: memory::PROGRAM_CODE_BASE,
            index: 0,
            stack_pointer: memory::STACK_BASE,
            memory: memory::Memory::new(&rom),
            display: display::Display::new(),
            rng: rand::thread_rng(),
            delay_timer: timer::Timer::new(),
        }
    }

    fn fetch_instruction(&mut self) -> Opcode {
        let instruction = Opcode(self.memory.read_u16_at(self.program_counter));
        self.program_counter += memory::WORD_SIZE;
        instruction
    }

    fn push(&mut self, value: u16) {
        self.stack_pointer -= memory::WORD_SIZE;
        self.memory.write_u16_at(value, self.stack_pointer);
    }

    fn pop(&mut self) -> u16 {
        let value = self.memory.read_u16_at(self.stack_pointer);
        self.stack_pointer += memory::WORD_SIZE;
        value
    }

    fn call(&mut self, addr: u16) {
        self.push(self.program_counter as u16);
        self.program_counter = addr as usize;
    }

    fn ret(&mut self) {
        self.program_counter = self.pop() as usize;
    }

    fn draw(&mut self, x: u8, y: u8, z: u8) {
        assert!(
            z <= 15,
            "Sprite size is {} when the maximum allowed sprite size is 15",
            z
        );

        let sprite = Sprite::new(&self.memory.0[self.index..self.index + z as usize]);
        self.gpr[0xf] = self.display.draw(&sprite, x as usize, y as usize) as u8;
    }

    fn skip_if(&mut self, predicate: bool) {
        if predicate {
            self.program_counter += memory::WORD_SIZE;
        }
    }

    fn bcd(&mut self, number: u8) {
        let bcd = [
            ((number / 100) % 10) as u8,
            ((number / 10) % 10) as u8,
            (number % 10) as u8,
        ];
        self.memory.write_at(&bcd, self.index);
    }

    fn load_regs(&mut self, reg_count: usize) {
        self.memory
            .read_at(&mut self.gpr[0..reg_count + 1], self.index);
    }

    fn store_regs(&mut self, reg_count: usize) {
        self.memory
            .write_at(&self.gpr[0..reg_count + 1], self.index);
    }

    pub fn execute(&mut self) {
        let opcode = self.fetch_instruction();

        match opcode.to_nibble_tuple() {
            opcode!("JMP addr") => {
                self.program_counter = opcode.tribble() as usize;
            }

            opcode!("JMP V0, addr") => {
                self.program_counter = (opcode.tribble() + self.gpr[0] as u16) as usize;
            }

            opcode!("CALL addr") => {
                self.call(opcode.tribble());
            }

            opcode!("RET") => {
                self.ret();
            }

            opcode!("MOV Vx, byte") => {
                self.gpr[opcode.reg1()] = opcode.byte();
            }

            opcode!("MOV Vx, Vy") => {
                self.gpr[opcode.reg1()] = self.gpr[opcode.reg2()];
            }

            opcode!("MOV I, addr") => {
                self.index = opcode.tribble() as usize;
            }

            opcode!("MOV DT, Vx") => {
                self.delay_timer.set(self.gpr[opcode.reg1()] as u64);
            }

            opcode!("MOV Vx, DT") => {
                self.gpr[opcode.reg1()] = self.delay_timer.get() as u8;
            }

            opcode!("ADD Vx, byte") => {
                self.gpr[opcode.reg1()].wrapping_add(opcode.byte());
            }

            opcode!("ADD Vx, Vy") => {
                self.gpr[opcode.reg1()].wrapping_add(self.gpr[opcode.reg2()]);
            }

            opcode!("ADD I, Vx") => {
                self.index.wrapping_add(self.gpr[opcode.reg1()] as usize);
            }

            opcode!("SUB Vx, Vy") => {
                self.gpr[opcode.reg1()].wrapping_sub(self.gpr[opcode.reg2()]);
            }

            opcode!("RSUB Vx, Vy") => {
                self.gpr[opcode.reg1()] =
                    self.gpr[opcode.reg2()].wrapping_sub(self.gpr[opcode.reg1()]);
            }

            opcode!("OR Vx, Vy") => {
                self.gpr[opcode.reg1()] |= self.gpr[opcode.reg2()];
            }

            opcode!("AND Vx, Vy") => {
                self.gpr[opcode.reg1()] &= self.gpr[opcode.reg2()];
            }

            opcode!("XOR Vx, Vy") => {
                self.gpr[opcode.reg1()] ^= self.gpr[opcode.reg2()];
            }

            opcode!("SHR Vx") => {
                self.gpr[opcode.reg1()].get::<bitvec::BigEndian>(0.into());
                self.gpr[opcode.reg1()] >>= 1;
            }

            opcode!("SHL Vx") => {
                self.gpr[opcode.reg1()].get::<bitvec::BigEndian>(7.into());
                self.gpr[opcode.reg1()] <<= 1;
            }

            opcode!("RND Vx, tribble") => {
                self.gpr[opcode.reg1()] = self.rng.gen_range(0, opcode.byte() + 1);
            }

            opcode!("SKE Vx, byte") => {
                self.skip_if(self.gpr[opcode.reg1()] == opcode.byte());
            }

            opcode!("SKE Vx, Vy") => {
                self.skip_if(self.gpr[opcode.reg1()] == self.gpr[opcode.reg2()])
            }

            opcode!("SKNE Vx, byte") => {
                self.skip_if(self.gpr[opcode.reg1()] != opcode.byte());
            }

            opcode!("SKNE Vx, Vy") => {
                self.skip_if(self.gpr[opcode.reg1()] != self.gpr[opcode.reg2()])
            }

            opcode!("CLS") => {
                self.display.clear()
            }

            opcode!("DRW Vx, Vy, nibble") => {
                self.draw(
                    self.gpr[opcode.reg1()],
                    self.gpr[opcode.reg2()],
                    opcode.nibble(),
                );
            }

            opcode!("BCD Vx") => {
                self.bcd(self.gpr[opcode.reg1()]);
            }

            opcode!("LD Vx, [I]") => {
                self.load_regs(opcode.reg1());
            }

            opcode!("STR [I], Vx") => {
                self.store_regs(opcode.reg1());
            }

            opcode!("FONT Vx") => {
                self.index = (self.gpr[opcode.reg1()] * 5) as usize;
            }
            _ => {
                println!(
                    "Unsupported opcode: 0x{:X} at 0x{:X}",
                    opcode.0,
                    self.program_counter - 2
                );
                std::process::exit(1);
            }
        }
    }
}
