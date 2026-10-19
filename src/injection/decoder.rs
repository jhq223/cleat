//! Bounded instruction decoding. No byte-pattern search through executable code.
use crate::{Error, Result};
use yaxpeax_arch::{Decoder as _, LengthedInstruction, U8Reader};

#[derive(Clone, Copy)]
pub enum Architecture {
    X86(u32),
    Arm,
    Thumb,
    Arm64,
}

pub fn branches(code: &[u8], address: usize, arch: Architecture) -> Result<Vec<usize>> {
    let mut targets = Vec::new();
    match arch {
        Architecture::X86(bits) => {
            use iced_x86::{Decoder, DecoderOptions, Mnemonic, OpKind};
            let decoder = Decoder::try_with_ip(bits, code, address as u64, DecoderOptions::NONE)
                .map_err(|e| Error::Bridge(e.to_string()))?;
            for instruction in decoder {
                if instruction.is_invalid() || instruction.mnemonic() == Mnemonic::Ret {
                    break;
                }
                if matches!(instruction.mnemonic(), Mnemonic::Call | Mnemonic::Jmp)
                    && matches!(
                        instruction.op0_kind(),
                        OpKind::NearBranch16 | OpKind::NearBranch32 | OpKind::NearBranch64
                    )
                {
                    targets.push(instruction.near_branch_target() as usize);
                    if instruction.mnemonic() == Mnemonic::Jmp {
                        break;
                    }
                }
            }
        }
        Architecture::Arm64 => {
            use yaxpeax_arm::armv8::a64::{InstDecoder, Opcode, Operand};
            let decoder = InstDecoder::default();
            for (offset, bytes) in code.as_chunks::<4>().0.iter().enumerate() {
                let Ok(instruction) = decoder.decode(&mut U8Reader::new(bytes)) else {
                    break;
                };
                if instruction.opcode == Opcode::RET {
                    break;
                }
                if matches!(instruction.opcode, Opcode::B | Opcode::BL) {
                    if let Operand::PCOffset(displacement) = instruction.operands[0] {
                        targets.push(
                            address
                                .wrapping_add(offset * 4)
                                .wrapping_add_signed(displacement as isize),
                        );
                    }
                    if instruction.opcode == Opcode::B {
                        break;
                    }
                }
            }
        }
        Architecture::Arm | Architecture::Thumb => {
            use yaxpeax_arm::armv7::{InstDecoder, Opcode, Operand};
            let thumb = matches!(arch, Architecture::Thumb);
            let decoder = if thumb {
                InstDecoder::default_thumb()
            } else {
                InstDecoder::default()
            };
            let mut offset = 0;
            while offset < code.len() {
                let Ok(instruction) = decoder.decode(&mut U8Reader::new(&code[offset..])) else {
                    break;
                };
                if matches!(instruction.opcode, Opcode::B | Opcode::BL | Opcode::BLX) {
                    let displacement = match instruction.operands[0] {
                        Operand::BranchOffset(value) => Some(value.wrapping_mul(4)),
                        Operand::BranchThumbOffset(value) => Some(value.wrapping_mul(2)),
                        _ => None,
                    };
                    if let Some(displacement) = displacement {
                        let mut pc = (address & !1).wrapping_add(offset);
                        if thumb {
                            pc += 4;
                            if instruction.opcode == Opcode::BLX {
                                pc &= !3;
                            }
                        }
                        let mut target = pc.wrapping_add_signed(displacement as isize);
                        if thumb != (instruction.opcode == Opcode::BLX) {
                            target |= 1;
                        } else {
                            target &= !1;
                        }
                        targets.push(target);
                    }
                    if instruction.opcode == Opcode::B {
                        break;
                    }
                }
                // Branch-to-register commonly marks a return; don't decode the next function.
                if instruction.opcode == Opcode::BX {
                    break;
                }
                offset += instruction.len().to_const() as usize;
            }
        }
    }
    Ok(targets)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn x86_ignores_call_bytes_inside_immediates_and_sign_extends_targets() {
        let bytes = [0xb8, 0xe8, 0, 0, 0, 0xe8, 0xf6, 0xff, 0xff, 0xff, 0xc3];
        assert_eq!(
            branches(&bytes, 0x1000, Architecture::X86(64)).unwrap(),
            [0x1000]
        );
    }
    #[test]
    fn arm_branches_use_architectural_pc_and_instruction_width() {
        assert_eq!(
            branches(&[0, 0, 0, 0xeb], 0x1000, Architecture::Arm).unwrap(),
            [0x1008]
        );
        assert_eq!(
            branches(&[0, 0, 0, 0x94], 0x1000, Architecture::Arm64).unwrap(),
            [0x1000]
        );
        assert_eq!(
            branches(&[0, 0xf0, 0, 0xf8], 0x1001, Architecture::Thumb).unwrap(),
            [0x1005]
        );
    }
}
