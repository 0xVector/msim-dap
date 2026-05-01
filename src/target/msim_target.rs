use super::{DebugTarget, Register, Result, TargetError};
use crate::dwarf::DwarfIndex;
use crate::msim::{Connection, CpuArch, CsrAddress, MsimError, RegisterId, Request};
use crate::{Address, CpuId, LineNo};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

pub struct MsimTarget<S: Connection> {
    connection: S,
    index: DwarfIndex,
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

impl<S: Connection> MsimTarget<S> {
    pub fn new(session: S, index: DwarfIndex) -> Self {
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

impl<S: Connection> DebugTarget for MsimTarget<S> {
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

    fn stop(&mut self) -> Result<()> {
        match self.connection.send(Request::Stop) {
            // We treat these errors as success
            Ok(_)
            | Err(MsimError::ListenerDied | MsimError::ClosedError | MsimError::IOError(_)) => {
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    fn step_by(&mut self, count: u64) -> Result<()> {
        self.connection.send(Request::Step(count))?.to_result()?;
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
            value: self
                .connection
                .send(Request::ReadPC(cpu))?
                .to_result()?
                .arg0,
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
            self.connection
                .send(Request::WritePC { cpu, value })?
                .to_result()?;
            return Ok(());
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
                    // Disabled
                    // (0xC02, "instret"),
                    // (0x10A, "senvcfg"),
                    // (0x5A8, "scontext"),
                    // (0xF11, "mvendorid"),
                    // (0xF12, "marchid"),
                    // (0xF13, "mimpid"),
                    // (0xF15, "mconfigptr"),
                    // (0x310, "mstatush"),
                    // (0x34A, "mtinst"),
                    // (0x34B, "mtval2"),
                    // (0x30A, "menvcfg"),
                    // (0x31A, "menvcfgh"),
                    // (0x747, "mseccfg"),
                    // (0x757, "mseccfgh"),
                    // (0x3A0, "pmpcfg0"),
                    // (0x3A1, "pmpcfg1"),
                    // (0x3A2, "pmpcfg2"),
                    // (0x3A3, "pmpcfg3"),
                    // (0x3A4, "pmpcfg4"),
                    // (0x3A5, "pmpcfg5"),
                    // (0x3A6, "pmpcfg6"),
                    // (0x3A7, "pmpcfg7"),
                    // (0x3A8, "pmpcfg8"),
                    // (0x3A9, "pmpcfg9"),
                    // (0x3AA, "pmpcfg10"),
                    // (0x3AB, "pmpcfg11"),
                    // (0x3AC, "pmpcfg12"),
                    // (0x3AD, "pmpcfg13"),
                    // (0x3AE, "pmpcfg14"),
                    // (0x3AF, "pmpcfg15"),
                    // (0x3B0, "pmpaddr0"),
                    // (0x3B1, "pmpaddr1"),
                    // (0x3B2, "pmpaddr2"),
                    // (0x3B3, "pmpaddr3"),
                    // (0x3B4, "pmpaddr4"),
                    // (0x3B5, "pmpaddr5"),
                    // (0x3B6, "pmpaddr6"),
                    // (0x3B7, "pmpaddr7"),
                    // (0x3B8, "pmpaddr8"),
                    // (0x3B9, "pmpaddr9"),
                    // (0x3BA, "pmpaddr10"),
                    // (0x3BB, "pmpaddr11"),
                    // (0x3BC, "pmpaddr12"),
                    // (0x3BD, "pmpaddr13"),
                    // (0x3BE, "pmpaddr14"),
                    // (0x3BF, "pmpaddr15"),
                    // (0x3C0, "pmpaddr16"),
                    // (0x3C1, "pmpaddr17"),
                    // (0x3C2, "pmpaddr18"),
                    // (0x3C3, "pmpaddr19"),
                    // (0x3C4, "pmpaddr20"),
                    // (0x3C5, "pmpaddr21"),
                    // (0x3C6, "pmpaddr22"),
                    // (0x3C7, "pmpaddr23"),
                    // (0x3C8, "pmpaddr24"),
                    // (0x3C9, "pmpaddr25"),
                    // (0x3CA, "pmpaddr26"),
                    // (0x3CB, "pmpaddr27"),
                    // (0x3CC, "pmpaddr28"),
                    // (0x3CD, "pmpaddr29"),
                    // (0x3CE, "pmpaddr30"),
                    // (0x3CF, "pmpaddr31"),
                    // (0x3D0, "pmpaddr32"),
                    // (0x3D1, "pmpaddr33"),
                    // (0x3D2, "pmpaddr34"),
                    // (0x3D3, "pmpaddr35"),
                    // (0x3D4, "pmpaddr36"),
                    // (0x3D5, "pmpaddr37"),
                    // (0x3D6, "pmpaddr38"),
                    // (0x3D7, "pmpaddr39"),
                    // (0x3D8, "pmpaddr40"),
                    // (0x3D9, "pmpaddr41"),
                    // (0x3DA, "pmpaddr42"),
                    // (0x3DB, "pmpaddr43"),
                    // (0x3DC, "pmpaddr44"),
                    // (0x3DD, "pmpaddr45"),
                    // (0x3DE, "pmpaddr46"),
                    // (0x3DF, "pmpaddr47"),
                    // (0x3E0, "pmpaddr48"),
                    // (0x3E1, "pmpaddr49"),
                    // (0x3E2, "pmpaddr50"),
                    // (0x3E3, "pmpaddr51"),
                    // (0x3E4, "pmpaddr52"),
                    // (0x3E5, "pmpaddr53"),
                    // (0x3E6, "pmpaddr54"),
                    // (0x3E7, "pmpaddr55"),
                    // (0x3E8, "pmpaddr56"),
                    // (0x3E9, "pmpaddr57"),
                    // (0x3EA, "pmpaddr58"),
                    // (0x3EB, "pmpaddr59"),
                    // (0x3EC, "pmpaddr60"),
                    // (0x3ED, "pmpaddr61"),
                    // (0x3EE, "pmpaddr62"),
                    // (0x3EF, "pmpaddr63"),
                    // (0xB00, "mcycle"),
                    // (0xB02, "minstret"),
                    // (0xB03, "mhpmcounter3"),
                    // (0xB04, "mhpmcounter4"),
                    // (0xB05, "mhpmcounter5"),
                    // (0xB06, "mhpmcounter6"),
                    // (0xB07, "mhpmcounter7"),
                    // (0xB08, "mhpmcounter8"),
                    // (0xB09, "mhpmcounter9"),
                    // (0xB0A, "mhpmcounter10"),
                    // (0xB0B, "mhpmcounter11"),
                    // (0xB0C, "mhpmcounter12"),
                    // (0xB0D, "mhpmcounter13"),
                    // (0xB0E, "mhpmcounter14"),
                    // (0xB0F, "mhpmcounter15"),
                    // (0xB10, "mhpmcounter16"),
                    // (0xB11, "mhpmcounter17"),
                    // (0xB12, "mhpmcounter18"),
                    // (0xB13, "mhpmcounter19"),
                    // (0xB14, "mhpmcounter20"),
                    // (0xB15, "mhpmcounter21"),
                    // (0xB16, "mhpmcounter22"),
                    // (0xB17, "mhpmcounter23"),
                    // (0xB18, "mhpmcounter24"),
                    // (0xB19, "mhpmcounter25"),
                    // (0xB1A, "mhpmcounter26"),
                    // (0xB1B, "mhpmcounter27"),
                    // (0xB1C, "mhpmcounter28"),
                    // (0xB1D, "mhpmcounter29"),
                    // (0xB1E, "mhpmcounter30"),
                    // (0xB1F, "mhpmcounter31"),
                    // (0xB80, "mcycleh"),
                    // (0xB82, "minstreth"),
                    // (0xB83, "mhpmcounter3h"),
                    // (0xB84, "mhpmcounter4h"),
                    // (0xB85, "mhpmcounter5h"),
                    // (0xB86, "mhpmcounter6h"),
                    // (0xB87, "mhpmcounter7h"),
                    // (0xB88, "mhpmcounter8h"),
                    // (0xB89, "mhpmcounter9h"),
                    // (0xB8A, "mhpmcounter10h"),
                    // (0xB8B, "mhpmcounter11h"),
                    // (0xB8C, "mhpmcounter12h"),
                    // (0xB8D, "mhpmcounter13h"),
                    // (0xB8E, "mhpmcounter14h"),
                    // (0xB8F, "mhpmcounter15h"),
                    // (0xB90, "mhpmcounter16h"),
                    // (0xB91, "mhpmcounter17h"),
                    // (0xB92, "mhpmcounter18h"),
                    // (0xB93, "mhpmcounter19h"),
                    // (0xB94, "mhpmcounter20h"),
                    // (0xB95, "mhpmcounter21h"),
                    // (0xB96, "mhpmcounter22h"),
                    // (0xB97, "mhpmcounter23h"),
                    // (0xB98, "mhpmcounter24h"),
                    // (0xB99, "mhpmcounter25h"),
                    // (0xB9A, "mhpmcounter26h"),
                    // (0xB9B, "mhpmcounter27h"),
                    // (0xB9C, "mhpmcounter28h"),
                    // (0xB9D, "mhpmcounter29h"),
                    // (0xB9E, "mhpmcounter30h"),
                    // (0xB9F, "mhpmcounter31h"),
                    // (0x320, "mcountinhibit"),
                    // (0x323, "mhpmevent3"),
                    // (0x324, "mhpmevent4"),
                    // (0x325, "mhpmevent5"),
                    // (0x326, "mhpmevent6"),
                    // (0x327, "mhpmevent7"),
                    // (0x328, "mhpmevent8"),
                    // (0x329, "mhpmevent9"),
                    // (0x32A, "mhpmevent10"),
                    // (0x32B, "mhpmevent11"),
                    // (0x32C, "mhpmevent12"),
                    // (0x32D, "mhpmevent13"),
                    // (0x32E, "mhpmevent14"),
                    // (0x32F, "mhpmevent15"),
                    // (0x330, "mhpmevent16"),
                    // (0x331, "mhpmevent17"),
                    // (0x332, "mhpmevent18"),
                    // (0x333, "mhpmevent19"),
                    // (0x334, "mhpmevent20"),
                    // (0x335, "mhpmevent21"),
                    // (0x336, "mhpmevent22"),
                    // (0x337, "mhpmevent23"),
                    // (0x338, "mhpmevent24"),
                    // (0x339, "mhpmevent25"),
                    // (0x33A, "mhpmevent26"),
                    // (0x33B, "mhpmevent27"),
                    // (0x33C, "mhpmevent28"),
                    // (0x33D, "mhpmevent29"),
                    // (0x33E, "mhpmevent30"),
                    // (0x33F, "mhpmevent31"),
                    // (0x7A0, "tselect"),
                    // (0x7A1, "tdata1"),
                    // (0x7A2, "tdata2"),
                    // (0x7A3, "tdata3"),
                    // (0x7A8, "mcontext"),
                    // (0x7B0, "dcsr"),
                    // (0x7B1, "dpc"),
                    // (0x7B2, "dscratch0"),
                    // (0x7B3, "dscratch1"),
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
