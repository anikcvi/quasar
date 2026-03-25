pub mod registers;
pub mod memory;
pub mod instructions;
pub mod parser;

pub use registers::RegisterFile;
pub use memory::Memory;
pub use instructions::MipsInstruction;
pub use parser::Parser;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SimulatorState {
    pub registers: [u32; 32],
    pub pc: u32,
    pub memory_sample: Vec<u8>,
    pub message: String,
}

pub struct MipsEngine {
    registers: RegisterFile,
    memory: Memory,
    pc: u32,
    program: Vec<MipsInstruction>,
    labels: HashMap<String, u32>,
    is_halted: bool,
    output_buffer: String,
}

impl MipsEngine {
    pub fn new() -> Self {
        MipsEngine {
            registers: RegisterFile::new(),
            memory: Memory::new(1024 * 64),
            pc: 0,
            program: Vec::new(),
            labels: HashMap::new(),
            is_halted: false,
            output_buffer: String::new(),
        }
    }

    pub fn load_program(&mut self, assembly: &str) -> Result<(), String> {
        self.reset();
        let mut instructions = Vec::new();
        let mut temp_labels = HashMap::new();
        let mut instruction_index = 0;

        for line in assembly.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() { continue; }

            if line.ends_with(':') {
                let label = line[..line.len()-1].to_string();
                temp_labels.insert(label, instruction_index);
            } else {
                if let Some(inst) = Parser::parse_line(line)? {
                    instructions.push(inst);
                    instruction_index += 1;
                }
            }
        }

        self.program = instructions;
        self.labels = temp_labels;
        Ok(())
    }

    pub fn step(&mut self) -> Result<bool, String> {
        if self.is_halted || self.pc as usize >= self.program.len() {
            return Ok(false);
        }

        let inst = self.program[self.pc as usize].clone();
        let mut next_pc = self.pc + 1;

        match inst {
            // R-Type
            MipsInstruction::Add { rd, rs, rt } => {
                let val = self.registers.read(rs).wrapping_add(self.registers.read(rt));
                self.registers.write(rd, val);
            },
            MipsInstruction::Sub { rd, rs, rt } => {
                let val = self.registers.read(rs).wrapping_sub(self.registers.read(rt));
                self.registers.write(rd, val);
            },
            MipsInstruction::And { rd, rs, rt } => {
                let val = self.registers.read(rs) & self.registers.read(rt);
                self.registers.write(rd, val);
            },
            MipsInstruction::Or { rd, rs, rt } => {
                let val = self.registers.read(rs) | self.registers.read(rt);
                self.registers.write(rd, val);
            },
            MipsInstruction::Xor { rd, rs, rt } => {
                let val = self.registers.read(rs) ^ self.registers.read(rt);
                self.registers.write(rd, val);
            },
            MipsInstruction::Nor { rd, rs, rt } => {
                let val = !(self.registers.read(rs) | self.registers.read(rt));
                self.registers.write(rd, val);
            },
            MipsInstruction::Slt { rd, rs, rt } => {
                let val = if (self.registers.read(rs) as i32) < (self.registers.read(rt) as i32) { 1 } else { 0 };
                self.registers.write(rd, val);
            },

            // I-Type
            MipsInstruction::Addi { rt, rs, imm } => {
                let val = self.registers.read(rs).wrapping_add(imm as u32);
                self.registers.write(rt, val);
            },
            MipsInstruction::Andi { rt, rs, imm } => {
                let val = self.registers.read(rs) & (imm as u32);
                self.registers.write(rt, val);
            },
            MipsInstruction::Ori { rt, rs, imm } => {
                let val = self.registers.read(rs) | (imm as u32);
                self.registers.write(rt, val);
            },
            MipsInstruction::Xori { rt, rs, imm } => {
                let val = self.registers.read(rs) ^ (imm as u32);
                self.registers.write(rt, val);
            },
            MipsInstruction::Lui { rt, imm } => {
                let val = (imm as u32) << 16;
                self.registers.write(rt, val);
            },
            MipsInstruction::Lw { rt, rs, offset } => {
                let addr = (self.registers.read(rs) as i32).wrapping_add(offset) as u32;
                let val = self.memory.read_word(addr)?;
                self.registers.write(rt, val);
            },
            MipsInstruction::Sw { rt, rs, offset } => {
                let addr = (self.registers.read(rs) as i32).wrapping_add(offset) as u32;
                let val = self.registers.read(rt);
                self.memory.write_word(addr, val)?;
            },
            MipsInstruction::Beq { rs, rt, label } => {
                if self.registers.read(rs) == self.registers.read(rt) {
                    next_pc = *self.labels.get(&label).ok_or(format!("Undefined label: {}", label))?;
                }
            },
            MipsInstruction::Bne { rs, rt, label } => {
                if self.registers.read(rs) != self.registers.read(rt) {
                    next_pc = *self.labels.get(&label).ok_or(format!("Undefined label: {}", label))?;
                }
            },

            // J-Type
            MipsInstruction::J { label } => {
                next_pc = *self.labels.get(&label).ok_or(format!("Undefined label: {}", label))?;
            },
            MipsInstruction::Jal { label } => {
                self.registers.write(31, (self.pc + 1) * 4); // Store return address (simplified)
                next_pc = *self.labels.get(&label).ok_or(format!("Undefined label: {}", label))?;
            },
            MipsInstruction::Jr { rs } => {
                // High-level simplification: if $ra, try to return to next instruction
                // Real JR requires mapping absolute addresses back to instruction indices.
                if rs == 31 {
                    self.is_halted = true; // For now, treat returning from main as halt
                    self.output_buffer.push_str("\n[Program Halted via JR $ra]");
                } else {
                    return Err("Complex JR not supported in this simplified engine".to_string());
                }
            },

            MipsInstruction::Syscall => {
                let v0 = self.registers.read(2);
                match v0 {
                    1 => { // print_int
                        let a0 = self.registers.read(4);
                        self.output_buffer.push_str(&format!("{}", a0 as i32));
                    },
                    10 => { // exit
                        self.is_halted = true;
                    },
                    _ => self.output_buffer.push_str(&format!("\n[Warning: Syscall {} not implemented]", v0)),
                }
            },
            MipsInstruction::Break => {
                self.is_halted = true;
                self.output_buffer.push_str("\n[Break encountered]");
            },
            MipsInstruction::Noop => {},
        }

        self.pc = next_pc;
        Ok(true)
    }

    pub fn run_all(&mut self) -> Result<String, String> {
        let mut count = 0;
        while count < 5000 && !self.is_halted && self.step()? {
            count += 1;
        }
        
        let mut result = self.output_buffer.clone();
        if count >= 5000 {
            result.push_str("\n[Error: Program timed out - possible infinite loop]");
        } else {
            result.push_str(&format!("\n[Completed in {} steps]", count));
        }
        Ok(result)
    }

    pub fn get_state(&self, message: String) -> SimulatorState {
        SimulatorState {
            registers: self.registers.get_all(),
            pc: self.pc * 4, // Show PC as byte offset for user
            memory_sample: self.memory.get_sample(256),
            message,
        }
    }

    pub fn reset(&mut self) {
        self.registers.reset();
        self.memory.reset();
        self.pc = 0;
        self.is_halted = false;
        self.output_buffer.clear();
    }
}
