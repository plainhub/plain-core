use crate::zip::result::ZipResult;
use crate::zip::result::invalid;
use std::io::Write;
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CustomExtraField {
    pub(crate) central_only: bool,
    pub header_id: u16,
    pub data: Box<[u8]>,
}
impl CustomExtraField {
    pub(crate) fn new(central_only: bool, header_id: u16, data: Box<[u8]>) -> Self {
        Self {
            central_only,
            header_id,
            data,
        }
    }

    pub(crate) fn full_size(&self, is_local_header: bool) -> usize {
        if self.central_only && is_local_header {
            return 0;
        }
        let size = self.data.len();
        size_of::<u16>() + size_of::<u16>() + size
    }
    pub(crate) fn write<W: Write>(&self, write: &mut W, is_local_header: bool) -> ZipResult<()> {
        if self.central_only && is_local_header {
            return Ok(());
        }
        write.write_all(&self.header_id.to_le_bytes())?;
        let size = self.data.len() as u16;
        write.write_all(&size.to_le_bytes())?;
        write.write_all(&self.data)?;
        Ok(())
    }
}
