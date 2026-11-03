use super::{Bits, huffman::Codebook};
use anyhow::{Result, bail};
use std::sync::{Arc, Mutex};
#[cfg(test)]
thread_local! { static GROUP_HITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[derive(Clone, Copy, Default)]
struct AcSkip {
    bits: u8,
    advance: u8,
}
#[derive(Clone, Copy, Default)]
struct AcGroup {
    bits: u8,
    advance: u8,
    end: bool,
}
pub(super) struct AcTable {
    codes: Codebook,
    fast: [AcSkip; 1024],
    groups: Vec<AcGroup>,
}
impl AcTable {
    pub(super) fn shared(counts: &[u8; 16], values: &[u8]) -> Result<Arc<Self>> {
        static TABLES: Mutex<AcTables> = Mutex::new(AcTables { tables: Vec::new() });
        TABLES.lock().unwrap().get(counts, values)
    }
    fn build(counts: &[u8; 16], values: &[u8]) -> Result<Self> {
        let codes = Codebook::new(counts, values)?;
        let mut fast = [AcSkip::default(); 1024];
        for (code, len, symbol) in codes.entries() {
            let magnitude = symbol & 15;
            if magnitude > 10 || (magnitude == 0 && symbol != 0 && symbol != 0xf0) {
                bail!("invalid sequential AC magnitude");
            }
            if len <= 10 {
                let start = usize::from(code) << (10 - len);
                let end = (usize::from(code) + 1) << (10 - len);
                let advance = match symbol {
                    0 => 64,
                    0xf0 => 16,
                    _ => (symbol >> 4) + 1,
                };
                fast[start..end].fill(AcSkip {
                    bits: len + magnitude,
                    advance,
                });
            }
        }
        let mut groups = vec![AcGroup::default(); 65536];
        for (prefix, group) in groups.iter_mut().enumerate() {
            let mut bits = 0u8;
            let mut advance = 0u8;
            while bits < 16 {
                let window = ((prefix as u32) << bits) & 65535;
                let entry = fast[(window >> 6) as usize];
                if entry.bits == 0 || bits + entry.bits > 16 {
                    break;
                }
                bits += entry.bits;
                if entry.advance == 64 {
                    *group = AcGroup {
                        bits,
                        advance,
                        end: true,
                    };
                    break;
                }
                if u16::from(advance) + u16::from(entry.advance) >= 64 {
                    break;
                }
                advance += entry.advance;
                *group = AcGroup {
                    bits,
                    advance,
                    end: false,
                };
            }
        }
        Ok(Self {
            codes,
            fast,
            groups,
        })
    }
    pub(super) fn skip(&self, reader: &mut Bits) -> Result<()> {
        let mut coefficient = 1usize;
        while coefficient < 64 {
            while reader.cnt < 16 {
                if reader.refill().is_none() {
                    break;
                }
            }
            let group = self.groups[(reader.cur >> 48) as usize];
            if group.bits > 0
                && i32::from(group.bits) <= reader.cnt
                && usize::from(group.advance) < 64 - coefficient
            {
                #[cfg(test)]
                GROUP_HITS.set(GROUP_HITS.get() + 1);
                reader.consume(u32::from(group.bits));
                if group.end {
                    return Ok(());
                }
                coefficient += usize::from(group.advance);
                continue;
            }
            while reader.cnt < 10 {
                if reader.refill().is_none() {
                    break;
                }
            }
            let entry = self.fast[(reader.cur >> 54) as usize];
            let advance = if entry.bits != 0 {
                while reader.cnt < i32::from(entry.bits) {
                    reader
                        .refill()
                        .ok_or_else(|| anyhow::anyhow!("truncated AC value"))?;
                }
                reader.consume(u32::from(entry.bits));
                entry.advance
            } else {
                let symbol = self.codes.symbol(reader)?;
                reader
                    .receive(u32::from(symbol & 15))
                    .ok_or_else(|| anyhow::anyhow!("truncated AC value"))?;
                if symbol == 0 {
                    64
                } else if symbol == 0xf0 {
                    16
                } else {
                    (symbol >> 4) + 1
                }
            };
            if advance == 64 {
                return Ok(());
            }
            coefficient += usize::from(advance);
            if coefficient > 64 {
                bail!("AC coefficient exceeds block");
            }
        }
        Ok(())
    }
}

struct AcTables {
    tables: Vec<Arc<AcTable>>,
}
impl AcTables {
    fn get(&mut self, counts: &[u8; 16], values: &[u8]) -> Result<Arc<AcTable>> {
        if let Some(table) = self
            .tables
            .iter()
            .find(|t| t.codes.counts == *counts && t.codes.values == values)
        {
            return Ok(table.clone());
        }
        let table = Arc::new(AcTable::build(counts, values)?);
        // Bound shared expanded tables to approximately four MiB across unrelated image jobs.
        if self.tables.len() < 16 {
            self.tables.push(table.clone());
        }
        Ok(table)
    }
}
#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/media/thumbnails/pjpeg/ac.rs"]
mod tests;
