//! Implementation of [`DebugTarget`] for MSIM, using the MSIM debug protocol.
use super::{DebugTarget, Register, Result, TargetError};
use crate::dwarf::DebugIndex;
use crate::msim::{ArgType, Connection, CpuArch, CsrAddress, MsimError, RegisterId, Request};
use crate::{Address, CpuId, LineNo};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

/// Debug target implementation for MSIM.
pub struct MsimTarget<C: Connection, I: DebugIndex> {
    connection: C,
    index: I,
    bp_store: BpStore,
    config: Option<Config>,
}

#[derive(Default)]
struct BpStore {
    bps_per_file: HashMap<PathBuf, Vec<(LineNo, Address)>>,
    bps_addresses: HashSet<Address>,
}

struct Config {
    cpu_count: u64,
    cpu_to_arch: HashMap<CpuId, CpuArch>,
}

/// Static information about a CPU architecture.
struct ArchInfo {
    reg_count: u64,
}

/// Map of architecture register and CSR numbers to names
struct RegisterMapper {
    /// General register number to name mapping, in order of register number.
    gen_reg_names: &'static [&'static str],
    /// CSR address (index) to name mapping
    csr_map: &'static [(CsrAddress, &'static str)],
}

const PC_REG_NAME: &str = "pc";

impl<C: Connection, I: DebugIndex> MsimTarget<C, I> {
    pub fn new(session: C, index: I) -> Self {
        Self {
            connection: session,
            index,
            bp_store: BpStore::default(),
            config: None,
        }
    }

    fn get_config(&mut self) -> Result<&Config> {
        // Retrieve and cache config
        if self.config.is_none() {
            let conf_response = self.connection.send(Request::GetConfig)?;
            let cpu_count = conf_response.arg0;
            let mut cpu_to_arch = HashMap::new();

            for cpu_id in 0..cpu_count {
                let arch_response = self.connection.send(Request::GetCpuInfo(cpu_id))?;
                cpu_to_arch.insert(cpu_id, CpuArch::read(arch_response.arg0));
            }

            self.config = Some(Config {
                cpu_count,
                cpu_to_arch,
            });
        }

        self.config.as_ref().ok_or(TargetError::SessionLost) // Will never happen
    }
}

impl<C: Connection, I: DebugIndex> DebugTarget for MsimTarget<C, I> {
    fn cpu_count(&mut self) -> Result<u64> {
        Ok(self.get_config()?.cpu_count)
    }

    fn resume(&mut self) -> Result<()> {
        self.connection.send(Request::Resume)?.to_result()?;
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.connection.send(Request::Pause)?.to_result()?;
        Ok(())
    }

    fn terminate(&mut self) -> Result<()> {
        match self.connection.send(Request::Terminate) {
            // We treat these errors as success
            Ok(_)
            | Err(MsimError::ListenerDied | MsimError::ClosedError | MsimError::IOError(_)) => {
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    fn step_by(&mut self, cpu: CpuId, count: u64) -> Result<()> {
        self.connection
            .send(Request::Step(cpu, count))?
            .to_result()?;
        self.resume()?;
        Ok(())
    }

    // TODO: if a BP is changed while step is in progress, replace fn will overwrite the step BP. Solving needs a flag but prolly not realistic.
    fn set_code_bp(&mut self, source: &Path, line: LineNo) -> Result<Address> {
        let address = self.index.get_address(source, line).ok_or_else(|| {
            TargetError::AddressNotFound(source.to_string_lossy().into_owned(), line)
        })?;

        // BP already exists, just return the address
        if self.bp_store.bps_addresses.contains(&address) {
            return Ok(address);
        }

        self.connection
            .send(Request::SetCodeBreakpoint(address))?
            .to_result()?;
        self.bp_store.bps_addresses.insert(address);
        self.bp_store
            .bps_per_file
            .entry(source.to_path_buf())
            .or_default()
            .push((line, address));

        Ok(address)
    }

    fn remove_code_bp(&mut self, source: &Path, line: LineNo) -> Result<Address> {
        let address = self.index.get_address(source, line).ok_or_else(|| {
            TargetError::AddressNotFound(source.to_string_lossy().into_owned(), line)
        })?;

        if self.bp_store.bps_addresses.remove(&address) {
            self.connection
                .send(Request::RemoveCodeBreakpoint(address))?
                .to_result()?;
        }
        if let Some(bps) = self.bp_store.bps_per_file.get_mut(source)
            && let Some(i) = bps.iter().position(|&(_, addr)| addr == address)
        {
            bps.remove(i);
        }

        Ok(address)
    }

    fn replace_code_bps(&mut self, source: &Path, lines: &[LineNo]) -> Vec<Result<()>> {
        let old_by_file: HashMap<LineNo, Address> = self
            .bp_store
            .bps_per_file
            .remove(source)
            .unwrap_or_default()
            .into_iter()
            .collect();

        // Remove old BPs that are not in the new set
        for (&line, &_addr) in &old_by_file {
            if !lines.contains(&line) {
                self.remove_code_bp(source, line).ok(); // TODO: handle error somehow
            }
        }

        let mut new_by_file = Vec::with_capacity(lines.len());
        let mut results = Vec::with_capacity(lines.len());

        for &line in lines {
            let result: Result<()> = (|| {
                // Reuse old BP if it exists
                if let Some(&addr) = old_by_file.get(&line) {
                    new_by_file.push((line, addr));
                    return Ok(());
                }

                let address = self.set_code_bp(source, line)?;
                new_by_file.push((line, address));
                Ok(())
            })();

            results.push(result);
            if matches!(results.last(), Some(Err(TargetError::SessionLost))) {
                break;
            }
        }

        if !new_by_file.is_empty() {
            self.bp_store
                .bps_per_file
                .insert(source.to_path_buf(), new_by_file);
        }
        results
    }

    fn resolve_code_bp(&self, address: Address) -> Option<(&Path, LineNo)> {
        if self.bp_store.bps_addresses.contains(&address) {
            self.index.resolve_address(address)
        } else {
            None
        }
    }

    fn resolve_address(&self, address: Address) -> Option<(&Path, LineNo)> {
        self.index.resolve_address(address)
    }

    fn read_general_regs(&mut self, cpu: CpuId) -> Result<Vec<Register>> {
        let arch = self.get_config()?.cpu_arch(cpu)?;
        let reg_map = arch.reg_map()?;
        let reg_count = arch.info()?.reg_count;

        let mut registers = vec![];
        eprintln!("Reading {reg_count} registers for CPU {cpu} ({arch})");

        for reg in 0..reg_count {
            registers.push(Register {
                name: reg_map.reg_name(reg)?,
                value: self
                    .connection
                    .send(Request::ReadGeneralRegister { cpu, reg })?
                    .arg0,
            });
        }
        eprintln!("Done reading registers for CPU {cpu}");
        Ok(registers)
    }

    fn write_general_reg(&mut self, cpu: CpuId, name: &str, value: u64) -> Result<()> {
        let reg = self
            .get_config()?
            .cpu_arch(cpu)?
            .reg_map()?
            .name_to_reg_id(name)?;

        self.connection
            .send(Request::WriteGeneralRegister { cpu, reg, value })?
            .to_result()?;
        Ok(())
    }

    fn read_csrs(&mut self, cpu: CpuId) -> Result<Vec<Register>> {
        let arch = self.get_config()?.cpu_arch(cpu)?;
        let reg_map = arch.reg_map()?;
        let csr_ids = reg_map
            .csr_map
            .iter()
            .map(|(addr, _)| *addr)
            .collect::<Vec<CsrAddress>>();

        let mut registers = vec![];

        // PC first (not in CSR map)
        eprintln!("Reading PC for CPU {cpu} ({arch})");
        registers.push(Register {
            name: PC_REG_NAME,
            value: self.read_pc(cpu)?,
        });

        eprintln!("Reading {} CSRs for CPU {cpu} ({arch})", csr_ids.len());
        for reg in csr_ids {
            let value = self
                .connection
                .send(Request::ReadCsr { cpu, reg })?
                .to_result()?
                .arg0;
            registers.push(Register {
                name: reg_map.csr_name(reg)?,
                value,
            });
        }
        eprintln!("Done reading CSRs for CPU {cpu}");
        Ok(registers)
    }

    fn write_csr(&mut self, cpu: CpuId, name: &str, value: u64) -> Result<()> {
        // PC special case
        if name == PC_REG_NAME {
            return self.write_pc(cpu, value);
        }

        let reg = self
            .get_config()?
            .cpu_arch(cpu)?
            .reg_map()?
            .name_to_csr_addr(name)?;

        self.connection
            .send(Request::WriteCsr { cpu, reg, value })?
            .to_result()?;
        Ok(())
    }

    fn read_pc(&mut self, cpu: CpuId) -> Result<Address> {
        Ok(self
            .connection
            .send(Request::ReadPC(cpu))?
            .to_result()?
            .arg0)
    }

    fn write_pc(&mut self, cpu: CpuId, address: Address) -> Result<()> {
        self.connection
            .send(Request::WritePC {
                cpu,
                value: address,
            })?
            .to_result()?;
        Ok(())
    }

    fn read_phys_memory(&mut self, address: Address, length: usize) -> Result<Vec<u8>> {
        const STEP: usize = 3 * size_of::<ArgType>(); // 3 args per response
        let mut bytes = vec![];

        for read_addr in (address..address + length as Address).step_by(STEP) {
            let chunk = self
                .connection
                .send(Request::ReadPhysMemory(read_addr))?
                .to_result()?;
            bytes.extend_from_slice(&chunk.arg0.to_be_bytes());
            bytes.extend_from_slice(&chunk.arg1.to_be_bytes());
            bytes.extend_from_slice(&chunk.arg2.to_be_bytes());
        }

        bytes.truncate(length);
        Ok(bytes)
    }

    fn read_virt_memory(&mut self, cpu: CpuId, address: Address, length: usize) -> Result<Vec<u8>> {
        const STEP: usize = 3 * size_of::<ArgType>(); // 3 args per response
        let mut bytes = vec![];

        for read_addr in (address..address + length as Address).step_by(STEP) {
            let chunk = self
                .connection
                .send(Request::ReadVirtMemory {
                    cpu,
                    address: read_addr,
                })?
                .to_result()?;
            bytes.extend_from_slice(&chunk.arg0.to_be_bytes());
            bytes.extend_from_slice(&chunk.arg1.to_be_bytes());
            bytes.extend_from_slice(&chunk.arg2.to_be_bytes());
        }

        bytes.truncate(length);
        Ok(bytes)
    }
}

impl Config {
    fn cpu_arch(&self, cpu_id: CpuId) -> Result<CpuArch> {
        self.cpu_to_arch
            .get(&cpu_id)
            .copied()
            .ok_or(TargetError::UnknownCpu(cpu_id))
    }
}

impl CpuArch {
    const fn info(self) -> Result<ArchInfo> {
        #[allow(clippy::match_same_arms)]
        let info = match self {
            Self::Mips => ArchInfo { reg_count: 32 },
            Self::RiscV32 => ArchInfo { reg_count: 32 },
            Self::RiscV64 => ArchInfo { reg_count: 32 },
            Self::Unknown => return Err(TargetError::UnknownArch),
        };
        Ok(info)
    }

    #[allow(clippy::too_many_lines, clippy::trivially_copy_pass_by_ref)]
    const fn reg_map(&self) -> Result<RegisterMapper> {
        let mapper = match self {
            Self::Mips => RegisterMapper {
                gen_reg_names: &[
                    "0", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4",
                    "t5", "t6", "t7", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "t8", "t9",
                    "k0", "k1", "gp", "sp", "fp", "ra",
                ],
                csr_map: &[
                    (0, "index"),
                    (1, "random"),
                    (2, "entrylo0"),
                    (3, "entrylo1"),
                    (4, "context"),
                    (5, "pagemask"),
                    (6, "wired"),
                    (7, "res_7"),
                    (8, "badvaddr"),
                    (9, "count"),
                    (10, "entryhi"),
                    (11, "compare"),
                    (12, "status"),
                    (13, "cause"),
                    (14, "epc"),
                    (15, "prid"),
                    (16, "config"),
                    (17, "lladdr"),
                    (18, "watchlo"),
                    (19, "watchhi"),
                    (20, "xcontext"),
                    (21, "res_21"),
                    (22, "res_22"),
                    (23, "res_23"),
                    (24, "res_24"),
                    (25, "res_25"),
                    (26, "res_26"),
                    (27, "res_27"),
                    (28, "res_28"),
                    (29, "res_29"),
                    (30, "errorepc"),
                    (31, "res_31"),
                ],
            },

            Self::RiscV32 | Self::RiscV64 => RegisterMapper {
                gen_reg_names: &[
                    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0/fp", "s1", "a0", "a1",
                    "a2", "a3", "a4", "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8",
                    "s9", "s10", "s11", "t3", "t4", "t5", "t6",
                ],

                // From the RISC-V privileged spec, both 32 and 64 bit
                csr_map: &[
                    (0xC00, "cycle"),
                    (0xC01, "time"),
                    // Machine-level CSRs
                    (0x300, "mstatus"),
                    (0x302, "medeleg"),
                    (0x303, "mideleg"),
                    (0x304, "mie"),
                    (0x305, "mtvec"),
                    (0x306, "mcounteren"),
                    (0x340, "mscratch"),
                    (0x341, "mepc"),
                    (0x342, "mcause"),
                    (0x343, "mtval"),
                    (0x344, "mip"),
                    (0x301, "misa"),
                    (0xF14, "mhartid"),
                    // Supervisor-level CSRs
                    (0x100, "sstatus"),
                    (0x104, "sie"),
                    (0x105, "stvec"),
                    (0x106, "scounteren"),
                    (0x140, "sscratch"),
                    (0x141, "sepc"),
                    (0x142, "scause"),
                    (0x143, "stval"),
                    (0x144, "sip"),
                    (0x180, "satp"),
                    (0x5C0, "scyclecmp"),
                ],
            },

            Self::Unknown => return Err(TargetError::UnknownArch),
        };
        Ok(mapper)
    }
}

impl RegisterMapper {
    fn reg_name(&self, reg_id: RegisterId) -> Result<&'static str> {
        self.gen_reg_names
            .get(usize::try_from(reg_id).map_err(|_| TargetError::BadGeneralReg(reg_id))?)
            .copied()
            .ok_or(TargetError::BadGeneralReg(reg_id))
    }

    fn csr_name(&self, csr_addr: CsrAddress) -> Result<&'static str> {
        self.csr_map
            .iter()
            .find(|(addr, _)| *addr == csr_addr)
            .map(|(_, name)| *name)
            .ok_or(TargetError::BadCsrReg(csr_addr))
    }

    fn name_to_reg_id(&self, name: &str) -> Result<RegisterId> {
        self.gen_reg_names
            .iter()
            .position(|reg_name| *reg_name == name)
            .map(|idx| idx as RegisterId)
            .ok_or_else(|| TargetError::UnknownRegisterName(name.to_string()))
    }

    fn name_to_csr_addr(&self, name: &str) -> Result<CsrAddress> {
        self.csr_map
            .iter()
            .find(|(_, csr_name)| *csr_name == name)
            .map(|(addr, _)| *addr)
            .ok_or_else(|| TargetError::UnknownCsrName(name.to_string()))
    }
}

impl fmt::Display for CpuArch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Mips => write!(f, "MIPS R4000 32-bit"),
            Self::RiscV32 => write!(f, "RISC-V 32-bit"),
            Self::RiscV64 => write!(f, "RISC-V 64-bit"),
            Self::Unknown => write!(f, "<unknown architecture>"),
        }
    }
}

impl From<MsimError> for TargetError {
    fn from(error: MsimError) -> Self {
        // Default to SessionLost (fatal), only RequestFailed is recoverable
        match error {
            MsimError::RequestFailed => Self::RequestFailed,
            _ => Self::SessionLost,
        }
    }
}
