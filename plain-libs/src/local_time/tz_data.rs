//! Rust parser of ZoneInfoDb(`tzdata`) on Android and OpenHarmony
//!
//! Ported from: https://android.googlesource.com/platform/prebuilts/fullsdk/sources/+/refs/heads/androidx-appcompat-release/android-34/com/android/i18n/timezone/ZoneInfoDb.java
use std::{
    ffi::CStr,
    fmt::Debug,
    fs::File,
    io::{Error, ErrorKind, Read, Result, Seek, SeekFrom},
};

/// Get timezone data from the `tzdata` file of HarmonyOS NEXT.
#[cfg(target_env = "ohos")]
pub(crate) fn for_zone(tz_string: &str) -> Result<Option<Vec<u8>>> {
    let mut file = File::open("/system/etc/zoneinfo/tzdata")?;
    find_tz_data::<OHOS_ENTRY_LEN>(&mut file, tz_string.as_bytes())
}

/// Get timezone data from the `tzdata` file of Android.
#[cfg(target_os = "android")]
pub(crate) fn for_zone(tz_string: &str) -> Result<Option<Vec<u8>>> {
    let mut file = open_android_tz_data_file()?;
    find_tz_data::<ANDROID_ENTRY_LEN>(&mut file, tz_string.as_bytes())
}

/// Open the `tzdata` file of Android from the environment variables.
#[cfg(target_os = "android")]
fn open_android_tz_data_file() -> Result<File> {
    for (env_var, path) in [
        // Default root path: /apex/com.android.tzdata
        ("ANDROID_TZDATA_ROOT", "/etc/tz"),
        // Default root path: /data
        ("ANDROID_DATA", "/misc/zoneinfo/current"),
        // Default root path: /system
        ("ANDROID_ROOT", "/usr/share/zoneinfo"),
    ] {
        if let Ok(env_value) = std::env::var(env_var) {
            if let Ok(file) = File::open(format!("{}{}/tzdata", env_value, path)) {
                return Ok(file);
            }
        }
    }
    Err(Error::from(ErrorKind::NotFound))
}

/// Get timezone data from the `tzdata` file reader
#[cfg(any(test, target_env = "ohos", target_os = "android"))]
fn find_tz_data<const ENTRY_LEN: usize>(
    mut reader: impl Read + Seek,
    tz_name: &[u8],
) -> Result<Option<Vec<u8>>> {
    let header = TzDataHeader::new(&mut reader)?;
    let length = reader.seek(SeekFrom::End(0))?;
    let index_length = header
        .data_offset
        .checked_sub(header.index_offset)
        .ok_or_else(invalid_offsets)? as usize;
    if header.index_offset < 24
        || header.data_offset > header.zonetab_offset
        || u64::from(header.zonetab_offset) > length
        || !index_length.is_multiple_of(ENTRY_LEN)
    {
        return Err(invalid_offsets());
    }
    reader.seek(SeekFrom::Start(u64::from(header.index_offset)))?;
    let index = TzDataIndexes::new::<ENTRY_LEN>(&mut reader, index_length)?;
    Ok(if let Some(entry) = index.find_timezone(tz_name) {
        Some(index.find_tzdata(reader, &header, entry)?)
    } else {
        None
    })
}

fn invalid_offsets() -> Error {
    Error::new(ErrorKind::InvalidData, "invalid tzdata offsets")
}

/// Header of the `tzdata` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TzDataHeader {
    version: [u8; 5],
    index_offset: u32,
    data_offset: u32,
    zonetab_offset: u32,
}

impl TzDataHeader {
    /// Parse the header of the `tzdata` file.
    fn new(mut data: impl Read) -> Result<Self> {
        let version = {
            let mut magic = [0; TZDATA_VERSION_LEN];
            data.read_exact(&mut magic)?;
            if !magic.starts_with(b"tzdata") || magic[TZDATA_VERSION_LEN - 1] != 0 {
                return Err(Error::new(ErrorKind::Other, "invalid tzdata header magic"));
            }
            let mut version = [0; 5];
            version.copy_from_slice(&magic[6..11]);
            version
        };

        let mut offset = [0; 4];
        data.read_exact(&mut offset)?;
        let index_offset = u32::from_be_bytes(offset);
        data.read_exact(&mut offset)?;
        let data_offset = u32::from_be_bytes(offset);
        data.read_exact(&mut offset)?;
        let zonetab_offset = u32::from_be_bytes(offset);

        Ok(Self {
            version,
            index_offset,
            data_offset,
            zonetab_offset,
        })
    }
}

/// Indexes of the `tzdata` file.
struct TzDataIndexes {
    indexes: Vec<TzDataIndex>,
}

impl TzDataIndexes {
    /// Create a new `TzDataIndexes` from the `tzdata` file reader.
    fn new<const ENTRY_LEN: usize>(mut reader: impl Read, index_length: usize) -> Result<Self> {
        let mut buf = vec![0; index_length];
        reader.read_exact(&mut buf)?;
        Ok(TzDataIndexes {
            indexes: buf
                .chunks_exact(ENTRY_LEN)
                .filter_map(|chunk| {
                    CStr::from_bytes_until_nul(&chunk[..TZ_NAME_LEN])
                        .ok()
                        .map(|name| {
                            let name = name.to_bytes().to_vec().into_boxed_slice();
                            let offset = u32::from_be_bytes(
                                chunk[TZ_NAME_LEN..TZ_NAME_LEN + 4].try_into().unwrap(),
                            );
                            let length = u32::from_be_bytes(
                                chunk[TZ_NAME_LEN + 4..TZ_NAME_LEN + 8].try_into().unwrap(),
                            );
                            TzDataIndex {
                                name,
                                offset,
                                length,
                            }
                        })
                })
                .collect(),
        })
    }

    /// Find a timezone by name.
    fn find_timezone(&self, timezone: &[u8]) -> Option<&TzDataIndex> {
        // timezones in tzdata are sorted by name.
        self.indexes
            .binary_search_by_key(&timezone, |x| &x.name)
            .map(|x| &self.indexes[x])
            .ok()
    }

    /// Retrieve a chunk of timezone data by the index.
    fn find_tzdata(
        &self,
        mut reader: impl Read + Seek,
        header: &TzDataHeader,
        index: &TzDataIndex,
    ) -> Result<Vec<u8>> {
        let start = u64::from(header.data_offset) + u64::from(index.offset);
        if start + u64::from(index.length) > u64::from(header.zonetab_offset) {
            return Err(invalid_offsets());
        }
        reader.seek(SeekFrom::Start(start))?;
        let mut buffer = vec![0; index.length as usize];
        reader.read_exact(&mut buffer)?;
        Ok(buffer)
    }
}

/// Index entry of the `tzdata` file.
struct TzDataIndex {
    name: Box<[u8]>,
    offset: u32,
    length: u32,
}

/// Ohos tzdata index entry size: `name + offset + length`
#[cfg(any(test, target_env = "ohos"))]
const OHOS_ENTRY_LEN: usize = TZ_NAME_LEN + 2 * size_of::<u32>();
/// Android tzdata index entry size: `name + offset + length + raw_utc_offset(legacy)`:
/// [reference](https://android.googlesource.com/platform/prebuilts/fullsdk/sources/+/refs/heads/androidx-appcompat-release/android-34/com/android/i18n/timezone/ZoneInfoDb.java#271)
#[cfg(any(test, target_os = "android"))]
const ANDROID_ENTRY_LEN: usize = TZ_NAME_LEN + 3 * size_of::<u32>();
/// The database reserves 40 bytes for each id.
const TZ_NAME_LEN: usize = 40;
/// Size of the version string in the header of `tzdata` file.
/// e.g. `tzdata2024b\0`
const TZDATA_VERSION_LEN: usize = 12;

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/local_time/tz_data.rs"]
mod tests;
