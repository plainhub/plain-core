use core::fmt;

macro_rules! tags {
    {
        // Permit arbitrary meta items, which include documentation.
        $( #[$enum_attr:meta] )*
        $vis:vis enum $name:ident($ty:tt) $(unknown($(#[$unknown_meta:meta])* $unknown_doc:ident))* {
            // Each of the `Name = Val,` permitting documentation.
            $($(#[$ident_attr:meta])* $tag:ident = $val:expr,)*
        }
    } => {
        $( #[$enum_attr] )*
        #[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
        #[non_exhaustive]
        #[repr($ty)]
        pub enum $name {
            $($(#[$ident_attr])* $tag = $val,)*
            $(
                $(#[$unknown_meta])*
                Unknown($ty),
            )*
        }

        impl $name {
            #[inline(always)]
            const fn __from_inner_type(n: $ty) -> Result<Self, $ty> {
                match n {
                    $( $val => Ok($name::$tag), )*
                    n => Err(n),
                }
            }

            #[inline(always)]
            const fn __to_inner_type(&self) -> $ty {
                match *self {
                    $( $name::$tag => $val, )*
                    $( $name::Unknown($unknown_doc) => { $unknown_doc }, )*
                }
            }
        }

        tags!($name, $ty, $($unknown_doc)*);
    };
    // For u16 tags, provide direct inherent primitive conversion methods.
    ($name:tt, u16, $($unknown_doc:ident)*) => {
        impl $name {
            #[inline(always)]
            pub const fn from_u16(val: u16) -> Option<Self> {
                match Self::__from_inner_type(val) {
                    Ok(v) => Some(v),
                    Err(_) => None,
                }
            }

            $(
            #[inline(always)]
            pub const fn from_u16_exhaustive($unknown_doc: u16) -> Self {
                match Self::__from_inner_type($unknown_doc) {
                    Ok(v) => v,
                    Err(_) => $name::Unknown($unknown_doc),
                }
            }
            )*

            #[inline(always)]
            pub const fn to_u16(&self) -> u16 {
                Self::__to_inner_type(self)
            }
        }
    };
    // For other tag types, do nothing for now. With concat_idents one could
    // provide inherent conversion methods for all types.
    ($name:tt, $ty:tt, $($unknown_doc:literal)*) => {};
}

// Note: These tags appear in the order they are mentioned in the TIFF reference
tags! {
pub enum Tag(u16) unknown(
    unknown
) {
    // Baseline tags:
    Artist = 315,
    // grayscale images PhotometricInterpretation 1 or 3
    BitsPerSample = 258,
    CellLength = 265, // TODO add support
    CellWidth = 264, // TODO add support
    // palette-color images (PhotometricInterpretation 3)
    ColorMap = 320, // TODO add support
    Compression = 259, // TODO add support for 2 and 32773
    DateTime = 306,
    ExtraSamples = 338, // TODO add support
    FillOrder = 266, // TODO add support
    FreeByteCounts = 289, // TODO add support
    FreeOffsets = 288, // TODO add support
    GrayResponseCurve = 291, // TODO add support
    GrayResponseUnit = 290, // TODO add support
    HostComputer = 316,
    ImageDescription = 270,
    ImageLength = 257,
    ImageWidth = 256,
    Make = 271,
    MaxSampleValue = 281, // TODO add support
    MinSampleValue = 280, // TODO add support
    Model = 272,
    NewSubfileType = 254, // TODO add support
    Orientation = 274, // TODO add support
    PhotometricInterpretation = 262,
    PlanarConfiguration = 284,
    ResolutionUnit = 296, // TODO add support
    RowsPerStrip = 278,
    SamplesPerPixel = 277,
    Software = 305,
    StripByteCounts = 279,
    StripOffsets = 273,
    SubfileType = 255, // TODO add support
    Threshholding = 263, // TODO add support
    XResolution = 282,
    YResolution = 283,
    // Advanced tags
    Predictor = 317,
    TileWidth = 322,
    TileLength = 323,
    TileOffsets = 324,
    TileByteCounts = 325,
    SubIfd = 330,
    // Data Sample Format
    SampleFormat = 339,
    SMinSampleValue = 340, // TODO add support
    SMaxSampleValue = 341, // TODO add support
    // JPEG
    JPEGTables = 347,
    // Subsampling
    #[doc(alias = "YCbCrSubsampling")]
    ChromaSubsampling = 530, // TODO add support
    #[doc(alias = "YCbCrPositioning")]
    ChromaPositioning = 531, // TODO add support
    // GeoTIFF
    ModelPixelScaleTag = 33550, // (SoftDesk)
    ModelTransformationTag = 34264, // (JPL Carto Group)
    ModelTiepointTag = 33922, // (Intergraph)
    // <https://web.archive.org/web/20131111073619/http://www.exif.org/Exif2-1.PDF>
    // *Do note its typo in the Decimal id*
    Copyright = 33_432,
    // <https://web.archive.org/web/20131111073619/http://www.exif.org/Exif2-1.PDF>
    ExifDirectory = 0x8769,
    // <https://web.archive.org/web/20131111073619/http://www.exif.org/Exif2-1.PDF>
    GpsDirectory = 0x8825,
    // <https://www.color.org/technotes/ICC-Technote-ProfileEmbedding.pdf>
    IccProfile = 34675,
    GeoKeyDirectoryTag = 34735, // (SPOT)
    GeoDoubleParamsTag = 34736, // (SPOT)
    GeoAsciiParamsTag = 34737, // (SPOT)
    ExifVersion = 0x9000,
    GdalNodata = 42113, // Contains areas with missing data
}
}
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
// We could be using `NonZeroU64` here but I find that this complicates semantics. This type
// represents the integer value stored as a value in a tag. The semantics of treating `0` as an end
// marker are imposed by the IFD. (It's unclear if Pointer tags such as Exif would allow `0` but in
// practice it just returns garbage and the validity does not matter greatly to us).
pub struct IfdPointer(pub u64);

impl fmt::LowerHex for IfdPointer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl core::fmt::UpperHex for IfdPointer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}

tags! {
pub enum Type(u16) {
    BYTE = 1,
    ASCII = 2,
    SHORT = 3,
    LONG = 4,
    RATIONAL = 5,
    SBYTE = 6,
    UNDEFINED = 7,
    SSHORT = 8,
    SLONG = 9,
    SRATIONAL = 10,
    FLOAT = 11,
    DOUBLE = 12,
    IFD = 13,
    LONG8 = 16,
    SLONG8 = 17,
    IFD8 = 18,
}
}

impl Type {
    pub(crate) fn byte_len(&self) -> u8 {
        match *self {
            Type::BYTE | Type::SBYTE | Type::ASCII | Type::UNDEFINED => 1,
            Type::SHORT | Type::SSHORT => 2,
            Type::LONG | Type::SLONG | Type::FLOAT | Type::IFD => 4,
            Type::LONG8
            | Type::SLONG8
            | Type::DOUBLE
            | Type::RATIONAL
            | Type::SRATIONAL
            | Type::IFD8 => 8,
        }
    }

    pub(crate) fn value_bytes(
        &self,
        count: u64,
    ) -> Result<u64, crate::image_codecs::tiff::error::TiffError> {
        let tag_size = u64::from(self.byte_len());

        match count.checked_mul(tag_size) {
            Some(n) => Ok(n),
            None => Err(crate::image_codecs::tiff::error::TiffError::LimitsExceeded),
        }
    }

    pub(crate) fn endian_bytes(self) -> EndianBytes {
        match self {
            Type::BYTE | Type::SBYTE | Type::ASCII | Type::UNDEFINED => EndianBytes::One,
            Type::SHORT | Type::SSHORT => EndianBytes::Two,
            Type::LONG
            | Type::SLONG
            | Type::FLOAT
            | Type::IFD
            | Type::RATIONAL
            | Type::SRATIONAL => EndianBytes::Four,
            Type::LONG8 | Type::SLONG8 | Type::DOUBLE | Type::IFD8 => EndianBytes::Eight,
        }
    }
}

tags! {
pub enum CompressionMethod(u16) unknown(
    unknown
) {
    None = 1,
    Huffman = 2,
    Fax3 = 3,
    Fax4 = 4,
    LZW = 5,
    JPEG = 6,
    // "Extended JPEG" or "new JPEG" style
    ModernJPEG = 7,
    Deflate = 8,
    OldDeflate = 0x80B2,
    PackBits = 0x8005,

    // Self-assigned by libtiff
    ZSTD = 0xC350,

    // Self-assigned by libtiff
    WebP = 0xC351,
}
}

tags! {
pub enum PhotometricInterpretation(u16) {
    WhiteIsZero = 0,
    BlackIsZero = 1,
    RGB = 2,
    RGBPalette = 3,
    TransparencyMask = 4,
    CMYK = 5,
    YCbCr = 6,
    CIELab = 8,
    IccLab = 9,
    ItuLab = 10,
}
}

tags! {
pub enum PlanarConfiguration(u16) {
    Chunky = 1,
    Planar = 2,
}
}

tags! {
pub enum Predictor(u16) {
    None = 1,
    Horizontal = 2,
    FloatingPoint = 3,
}
}

tags! {
pub enum ResolutionUnit(u16) {
    None = 1,
    Inch = 2,
    Centimeter = 3,
}
}

tags! {
pub enum SampleFormat(u16) unknown(
    unknown
) {
    Uint = 1,
    Int = 2,
    IEEEFP = 3,
    Void = 4,
}
}

tags! {
pub enum ExtraSamples(u16) {
    Unspecified = 0,
    AssociatedAlpha = 1,
    UnassociatedAlpha = 2,
}
}
pub struct ValueBuffer {
    bytes: Vec<u8>,
    ty: Type,
    count: u64,
    byte_order: ByteOrder,
}

impl ValueBuffer {
    pub fn empty(ty: Type) -> Self {
        ValueBuffer {
            bytes: vec![],
            ty,
            count: 0,
            byte_order: ByteOrder::native(),
        }
    }

    pub fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    pub fn data_type(&self) -> Type {
        self.ty
    }
    pub fn count(&self) -> u64 {
        debug_assert!({
            self.ty
                .value_bytes(self.count)
                .is_ok_and(|n| n <= self.bytes.len() as u64)
        });

        self.count
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.assumed_len_from_count()]
    }
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        let len = self.assumed_len_from_count();
        &mut self.bytes[..len]
    }
    pub fn set_byte_order(&mut self, byte_order: ByteOrder) {
        let len = self.assumed_len_from_count();

        self.byte_order
            .convert(self.ty, &mut self.bytes[..len], byte_order);

        self.byte_order = byte_order;
    }
    pub(crate) fn prepare_length(&mut self, to_len: usize) {
        if to_len > self.bytes.len() {
            self.bytes.resize(to_len, 0);
        }

        if self.bytes.len() < to_len / 2 {
            self.bytes.truncate(to_len);
            self.bytes.shrink_to_fit();
        }
    }
    pub(crate) fn assume_type(&mut self, ty: Type, count: u64, bo: ByteOrder) {
        debug_assert!({
            ty.value_bytes(count)
                .is_ok_and(|n| n <= self.bytes.len() as u64)
        });

        self.byte_order = bo;
        self.ty = ty;
        self.count = count;
    }

    pub(crate) fn raw_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    fn assumed_len_from_count(&self) -> usize {
        usize::from(self.ty.byte_len()) * self.count as usize
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteOrder {
    LittleEndian,
    BigEndian,
}

impl ByteOrder {
    pub const fn native() -> Self {
        match () {
            #[cfg(target_endian = "little")]
            () => ByteOrder::LittleEndian,
            #[cfg(target_endian = "big")]
            () => ByteOrder::BigEndian,
            #[cfg(not(any(target_endian = "big", target_endian = "little")))]
            () => compile_error!("Unsupported target"),
        }
    }
    pub fn convert(self, ty: Type, buffer: &mut [u8], to: ByteOrder) {
        self.convert_endian_bytes(ty.endian_bytes(), buffer, to)
    }

    pub(crate) fn convert_endian_bytes(self, cls: EndianBytes, buffer: &mut [u8], to: ByteOrder) {
        if self == to {
            return;
        }

        // FIXME: at MSRV 1.89 or higher use `slice::as_chunks_mut`.
        match cls {
            EndianBytes::One => {
                // No change needed
            }
            EndianBytes::Two => {
                for chunk in buffer.chunks_exact_mut(2) {
                    let chunk: &mut [u8; 2] = chunk.try_into().unwrap();
                    *chunk = u16::from_be_bytes(*chunk).to_le_bytes();
                }
            }
            EndianBytes::Four => {
                for chunk in buffer.chunks_exact_mut(4) {
                    let chunk: &mut [u8; 4] = chunk.try_into().unwrap();
                    *chunk = u32::from_be_bytes(*chunk).to_le_bytes();
                }
            }
            EndianBytes::Eight => {
                for chunk in buffer.chunks_exact_mut(8) {
                    let chunk: &mut [u8; 8] = chunk.try_into().unwrap();
                    *chunk = u64::from_be_bytes(*chunk).to_le_bytes();
                }
            }
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum EndianBytes {
    One,
    Two,
    Four,
    Eight,
}
