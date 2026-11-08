use std::io::{self, Read};

use crate::compound_file::ReadLeNumber;
use crate::compound_file::internal::{Version, consts};

//===========================================================================//

#[derive(Clone, PartialEq, Eq)]
pub struct Header {
    pub version: Version,
    pub num_fat_sectors: u32,
    pub first_dir_sector: u32,
    pub first_minifat_sector: u32,
    pub first_difat_sector: u32,
    pub initial_difat_entries: [u32; consts::NUM_DIFAT_ENTRIES_IN_HEADER],
}

impl Header {
    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Header> {
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;
        if magic != consts::MAGIC_NUMBER {
            invalid_data!("Invalid CFB file (wrong magic number): {:x?}", magic);
        }
        reader.read_exact(&mut [0u8; 16])?; // reserved field

        // Read the version number, but don't try to interpret it until after
        // we've checked the byte order mark.
        let _minor_version = reader.read_le_u16()?;
        let version_number = reader.read_le_u16()?;

        let byte_order_mark = reader.read_le_u16()?;
        if byte_order_mark != consts::BYTE_ORDER_MARK {
            invalid_data!(
                "Invalid CFB byte order mark (expected 0x{:04X}, found \
                 0x{:04X})",
                consts::BYTE_ORDER_MARK,
                byte_order_mark
            );
        }

        let version = match Version::from_number(version_number) {
            Some(version) => version,
            None => {
                invalid_data!("CFB version {} is not supported", version_number);
            }
        };

        let sector_shift = reader.read_le_u16()?;
        if sector_shift != version.sector_shift() {
            invalid_data!(
                "Incorrect sector shift for CFB version {} (expected {}, \
                 found {})",
                version.number(),
                version.sector_shift(),
                sector_shift
            );
        }

        let mini_sector_shift = reader.read_le_u16()?;
        if mini_sector_shift != consts::MINI_SECTOR_SHIFT {
            invalid_data!(
                "Incorrect mini sector shift (expected {}, found {})",
                consts::MINI_SECTOR_SHIFT,
                mini_sector_shift
            );
        }

        // TODO: require reserved field to be all zeros under strict validation
        reader.read_exact(&mut [0u8; 6])?; // reserved field

        reader.read_le_u32()?;
        let num_fat_sectors = reader.read_le_u32()?;
        let first_dir_sector = reader.read_le_u32()?;
        let _transaction_signature = reader.read_le_u32()?;

        let mini_stream_cutoff = reader.read_le_u32()?;
        if mini_stream_cutoff != consts::MINI_STREAM_CUTOFF {
            invalid_data!(
                "Incorrect mini stream cutoff (expected {}, found {})",
                consts::MINI_STREAM_CUTOFF,
                mini_stream_cutoff
            );
        }

        let first_minifat_sector = reader.read_le_u32()?;
        reader.read_le_u32()?;
        let mut first_difat_sector = reader.read_le_u32()?;
        reader.read_le_u32()?;

        // Some CFB implementations use FREE_SECTOR to indicate END_OF_CHAIN.
        if first_difat_sector == consts::FREE_SECTOR {
            first_difat_sector = consts::END_OF_CHAIN;
        }

        let mut initial_difat_entries = [consts::FREE_SECTOR; consts::NUM_DIFAT_ENTRIES_IN_HEADER];
        for entry in initial_difat_entries.iter_mut() {
            let next = reader.read_le_u32()?;
            if next == consts::FREE_SECTOR {
                break;
            } else if next > consts::MAX_REGULAR_SECTOR {
                invalid_data!(
                    "Initial DIFAT array refers to invalid sector index \
                     0x{:08X}",
                    next
                );
            }
            *entry = next;
        }

        Ok(Header {
            version,
            num_fat_sectors,
            first_dir_sector,
            first_minifat_sector,
            first_difat_sector,
            initial_difat_entries,
        })
    }
}

//===========================================================================//
