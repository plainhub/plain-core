#![allow(unused_imports)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::identity_op)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::doc_lazy_continuation)]
#![allow(rustdoc::broken_intra_doc_links)]
#![allow(rustdoc::bare_urls)]
#![allow(rustdoc::invalid_html_tags)]
#[link(name = "CoreVideo", kind = "framework")]
extern "C" {}
#[cfg(feature = "CVBase")]
#[path = "CVBase.rs"]
mod __CVBase;
#[cfg(feature = "CVBuffer")]
#[path = "CVBuffer.rs"]
mod __CVBuffer;
#[cfg(any())]
#[path = "CVDisplayLink.rs"]
mod __CVDisplayLink;
#[cfg(any())]
#[path = "CVHostTime.rs"]
mod __CVHostTime;
#[cfg(feature = "CVImageBuffer")]
#[path = "CVImageBuffer.rs"]
mod __CVImageBuffer;
#[cfg(any())]
#[path = "CVMetalBuffer.rs"]
mod __CVMetalBuffer;
#[cfg(any())]
#[path = "CVMetalBufferCache.rs"]
mod __CVMetalBufferCache;
#[cfg(any())]
#[path = "CVMetalTexture.rs"]
mod __CVMetalTexture;
#[cfg(any())]
#[path = "CVMetalTextureCache.rs"]
mod __CVMetalTextureCache;
#[cfg(any())]
#[path = "CVOpenGLBuffer.rs"]
mod __CVOpenGLBuffer;
#[cfg(any())]
#[path = "CVOpenGLBufferPool.rs"]
mod __CVOpenGLBufferPool;
#[cfg(any())]
#[path = "CVOpenGLTexture.rs"]
mod __CVOpenGLTexture;
#[cfg(any())]
#[path = "CVOpenGLTextureCache.rs"]
mod __CVOpenGLTextureCache;
#[cfg(feature = "CVPixelBuffer")]
#[path = "CVPixelBuffer.rs"]
mod __CVPixelBuffer;
#[cfg(any())]
#[path = "CVPixelBufferIOSurface.rs"]
mod __CVPixelBufferIOSurface;
#[cfg(feature = "CVPixelBufferPool")]
#[path = "CVPixelBufferPool.rs"]
mod __CVPixelBufferPool;
#[cfg(any())]
#[path = "CVPixelFormatDescription.rs"]
mod __CVPixelFormatDescription;
#[cfg(any())]
#[path = "CVReturn.rs"]
mod __CVReturn;
#[cfg(feature = "CVBase")]
pub use self::__CVBase::CVOptionFlags;
#[cfg(feature = "CVBase")]
pub use self::__CVBase::CVSMPTETime;
#[cfg(feature = "CVBase")]
pub use self::__CVBase::CVSMPTETimeFlags;
#[cfg(feature = "CVBase")]
pub use self::__CVBase::CVSMPTETimeType;
#[cfg(feature = "CVBase")]
pub use self::__CVBase::CVTime;
#[cfg(feature = "CVBase")]
pub use self::__CVBase::CVTimeFlags;
#[cfg(feature = "CVBase")]
pub use self::__CVBase::CVTimeStamp;
#[cfg(feature = "CVBase")]
pub use self::__CVBase::CVTimeStampFlags;
#[cfg(feature = "CVBuffer")]
pub use self::__CVBuffer::CVAttachmentMode;
#[cfg(feature = "CVBuffer")]
pub use self::__CVBuffer::CVBuffer;
#[cfg(any())]
pub use self::__CVDisplayLink::CVDisplayLink;
#[cfg(all(any(), any()))]
pub use self::__CVDisplayLink::CVDisplayLinkCreateWithActiveCGDisplays;
#[cfg(all(any(), any(), any()))]
pub use self::__CVDisplayLink::CVDisplayLinkCreateWithCGDisplay;
#[cfg(all(any(), any(), any()))]
pub use self::__CVDisplayLink::CVDisplayLinkCreateWithCGDisplays;
#[cfg(all(any(), any(), any()))]
pub use self::__CVDisplayLink::CVDisplayLinkCreateWithOpenGLDisplayMask;
#[cfg(any())]
pub use self::__CVDisplayLink::CVDisplayLinkGetActualOutputVideoRefreshPeriod;
#[cfg(all(any(), any()))]
pub use self::__CVDisplayLink::CVDisplayLinkGetCurrentCGDisplay;
#[cfg(all(feature = "CVBase", any(), any()))]
pub use self::__CVDisplayLink::CVDisplayLinkGetCurrentTime;
#[cfg(all(feature = "CVBase", any()))]
pub use self::__CVDisplayLink::CVDisplayLinkGetNominalOutputVideoRefreshPeriod;
#[cfg(all(feature = "CVBase", any()))]
pub use self::__CVDisplayLink::CVDisplayLinkGetOutputVideoLatency;
#[cfg(any())]
pub use self::__CVDisplayLink::CVDisplayLinkIsRunning;
#[cfg(all(feature = "CVBase", any(), any()))]
pub use self::__CVDisplayLink::CVDisplayLinkOutputCallback;
#[cfg(all(feature = "CVBase", any(), any(), any()))]
pub use self::__CVDisplayLink::CVDisplayLinkOutputHandler;
#[cfg(all(any(), any(), any()))]
pub use self::__CVDisplayLink::CVDisplayLinkSetCurrentCGDisplay;
#[cfg(all(any(), any(), any()))]
#[cfg(target_os = "macos")]
pub use self::__CVDisplayLink::CVDisplayLinkSetCurrentCGDisplayFromOpenGLContext;
#[cfg(all(feature = "CVBase", any(), any()))]
pub use self::__CVDisplayLink::CVDisplayLinkSetOutputCallback;
#[cfg(all(feature = "CVBase", any(), any(), any()))]
pub use self::__CVDisplayLink::CVDisplayLinkSetOutputHandler;
#[cfg(all(any(), any()))]
pub use self::__CVDisplayLink::CVDisplayLinkStart;
#[cfg(all(any(), any()))]
pub use self::__CVDisplayLink::CVDisplayLinkStop;
#[cfg(all(feature = "CVBase", any(), any()))]
pub use self::__CVDisplayLink::CVDisplayLinkTranslateTime;
#[cfg(any())]
pub use self::__CVHostTime::CVGetCurrentHostTime;
#[cfg(any())]
pub use self::__CVHostTime::CVGetHostClockFrequency;
#[cfg(any())]
pub use self::__CVHostTime::CVGetHostClockMinimumTimeDelta;
#[cfg(feature = "CVImageBuffer")]
pub use self::__CVImageBuffer::CVColorPrimariesGetIntegerCodePointForString;
#[cfg(feature = "CVImageBuffer")]
pub use self::__CVImageBuffer::CVColorPrimariesGetStringForIntegerCodePoint;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer"))]
pub use self::__CVImageBuffer::CVImageBuffer;
#[cfg(all(feature = "CVImageBuffer", any()))]
pub use self::__CVImageBuffer::CVImageBufferCreateColorSpaceFromAttachments;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer"))]
pub use self::__CVImageBuffer::CVImageBufferGetCleanRect;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer", any()))]
pub use self::__CVImageBuffer::CVImageBufferGetColorSpace;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer"))]
pub use self::__CVImageBuffer::CVImageBufferGetDisplaySize;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer"))]
pub use self::__CVImageBuffer::CVImageBufferGetEncodedSize;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer"))]
pub use self::__CVImageBuffer::CVImageBufferIsFlipped;
#[cfg(feature = "CVImageBuffer")]
pub use self::__CVImageBuffer::CVTransferFunctionGetIntegerCodePointForString;
#[cfg(feature = "CVImageBuffer")]
pub use self::__CVImageBuffer::CVTransferFunctionGetStringForIntegerCodePoint;
#[cfg(feature = "CVImageBuffer")]
pub use self::__CVImageBuffer::CVYCbCrMatrixGetIntegerCodePointForString;
#[cfg(feature = "CVImageBuffer")]
pub use self::__CVImageBuffer::CVYCbCrMatrixGetStringForIntegerCodePoint;
#[cfg(all(feature = "CVBuffer", any()))]
pub use self::__CVMetalBuffer::CVMetalBuffer;
#[cfg(all(feature = "CVBuffer", any(), feature = "objc2", any()))]
#[cfg(not(target_os = "watchos"))]
pub use self::__CVMetalBuffer::CVMetalBufferGetBuffer;
#[cfg(any())]
pub use self::__CVMetalBuffer::CVMetalBufferGetTypeID;
#[cfg(any())]
pub use self::__CVMetalBufferCache::kCVMetalBufferCacheMaximumBufferAgeKey;
#[cfg(any())]
pub use self::__CVMetalBufferCache::CVMetalBufferCache;
#[cfg(all(any(), any(), feature = "objc2", any()))]
#[cfg(not(target_os = "watchos"))]
pub use self::__CVMetalBufferCache::CVMetalBufferCacheCreate;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer", any(), any(), any()))]
pub use self::__CVMetalBufferCache::CVMetalBufferCacheCreateBufferFromImage;
#[cfg(all(feature = "CVBase", any()))]
pub use self::__CVMetalBufferCache::CVMetalBufferCacheFlush;
#[cfg(any())]
pub use self::__CVMetalTexture::kCVMetalTextureStorageMode;
#[cfg(any())]
pub use self::__CVMetalTexture::kCVMetalTextureUsage;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer", any()))]
pub use self::__CVMetalTexture::CVMetalTexture;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    any(),
    feature = "objc2",
    any()
))]
#[cfg(not(target_os = "watchos"))]
pub use self::__CVMetalTexture::CVMetalTextureGetTexture;
#[cfg(any())]
pub use self::__CVMetalTexture::CVMetalTextureGetTypeID;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer", any()))]
pub use self::__CVMetalTexture::CVMetalTextureIsFlipped;
#[cfg(any())]
pub use self::__CVMetalTextureCache::kCVMetalTextureCacheMaximumTextureAgeKey;
#[cfg(any())]
pub use self::__CVMetalTextureCache::CVMetalTextureCache;
#[cfg(all(any(), any(), feature = "objc2", any()))]
#[cfg(not(target_os = "watchos"))]
pub use self::__CVMetalTextureCache::CVMetalTextureCacheCreate;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    any(),
    any(),
    any(),
    any()
))]
#[cfg(not(target_os = "watchos"))]
pub use self::__CVMetalTextureCache::CVMetalTextureCacheCreateTextureFromImage;
#[cfg(all(feature = "CVBase", any()))]
pub use self::__CVMetalTextureCache::CVMetalTextureCacheFlush;
#[cfg(any())]
pub use self::__CVOpenGLBuffer::kCVOpenGLBufferHeight;
#[cfg(any())]
pub use self::__CVOpenGLBuffer::kCVOpenGLBufferInternalFormat;
#[cfg(any())]
pub use self::__CVOpenGLBuffer::kCVOpenGLBufferMaximumMipmapLevel;
#[cfg(any())]
pub use self::__CVOpenGLBuffer::kCVOpenGLBufferTarget;
#[cfg(any())]
pub use self::__CVOpenGLBuffer::kCVOpenGLBufferWidth;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer", any()))]
pub use self::__CVOpenGLBuffer::CVOpenGLBuffer;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer", any(), any(), any()))]
#[cfg(target_os = "macos")]
pub use self::__CVOpenGLBuffer::CVOpenGLBufferAttach;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer", any(), any()))]
pub use self::__CVOpenGLBuffer::CVOpenGLBufferCreate;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer", any()))]
pub use self::__CVOpenGLBuffer::CVOpenGLBufferGetAttributes;
#[cfg(any())]
pub use self::__CVOpenGLBuffer::CVOpenGLBufferGetTypeID;
#[cfg(any())]
pub use self::__CVOpenGLBufferPool::kCVOpenGLBufferPoolMaximumBufferAgeKey;
#[cfg(any())]
pub use self::__CVOpenGLBufferPool::kCVOpenGLBufferPoolMinimumBufferCountKey;
#[cfg(any())]
pub use self::__CVOpenGLBufferPool::CVOpenGLBufferPool;
#[cfg(all(any(), any()))]
pub use self::__CVOpenGLBufferPool::CVOpenGLBufferPoolCreate;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer", any(), any(), any()))]
pub use self::__CVOpenGLBufferPool::CVOpenGLBufferPoolCreateOpenGLBuffer;
#[cfg(any())]
pub use self::__CVOpenGLBufferPool::CVOpenGLBufferPoolGetAttributes;
#[cfg(any())]
pub use self::__CVOpenGLBufferPool::CVOpenGLBufferPoolGetOpenGLBufferAttributes;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer", any()))]
pub use self::__CVOpenGLTexture::CVOpenGLTexture;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer", any(), any()))]
#[cfg(target_os = "macos")]
pub use self::__CVOpenGLTexture::CVOpenGLTextureGetName;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer", any(), any()))]
#[cfg(target_os = "macos")]
pub use self::__CVOpenGLTexture::CVOpenGLTextureGetTarget;
#[cfg(any())]
pub use self::__CVOpenGLTexture::CVOpenGLTextureGetTypeID;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer", any()))]
pub use self::__CVOpenGLTexture::CVOpenGLTextureIsFlipped;
#[cfg(any())]
pub use self::__CVOpenGLTextureCache::kCVOpenGLTextureCacheChromaSamplingModeAutomatic;
#[cfg(any())]
pub use self::__CVOpenGLTextureCache::kCVOpenGLTextureCacheChromaSamplingModeBestPerformance;
#[cfg(any())]
pub use self::__CVOpenGLTextureCache::kCVOpenGLTextureCacheChromaSamplingModeHighestQuality;
#[cfg(any())]
pub use self::__CVOpenGLTextureCache::kCVOpenGLTextureCacheChromaSamplingModeKey;
#[cfg(any())]
pub use self::__CVOpenGLTextureCache::CVOpenGLTextureCache;
#[cfg(all(any(), any(), any()))]
#[cfg(target_os = "macos")]
pub use self::__CVOpenGLTextureCache::CVOpenGLTextureCacheCreate;
#[cfg(all(feature = "CVBuffer", feature = "CVImageBuffer", any(), any(), any()))]
pub use self::__CVOpenGLTextureCache::CVOpenGLTextureCacheCreateTextureFromImage;
#[cfg(all(feature = "CVBase", any()))]
pub use self::__CVOpenGLTextureCache::CVOpenGLTextureCacheFlush;
#[cfg(feature = "CVPixelBuffer")]
pub use self::__CVPixelBuffer::kCVPixelBufferPixelFormatTypeKey;
#[cfg(feature = "CVPixelBuffer")]
pub use self::__CVPixelBuffer::kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBuffer;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferCopyCreationAttributes;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer",
    any()
))]
pub use self::__CVPixelBuffer::CVPixelBufferCreate;
#[cfg(all(feature = "CVPixelBuffer", any()))]
pub use self::__CVPixelBuffer::CVPixelBufferCreateResolvedAttributesDictionary;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer",
    any()
))]
pub use self::__CVPixelBuffer::CVPixelBufferCreateWithBytes;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer",
    any()
))]
pub use self::__CVPixelBuffer::CVPixelBufferCreateWithPlanarBytes;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer",
    any()
))]
pub use self::__CVPixelBuffer::CVPixelBufferFillExtendedPixels;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferGetBaseAddress;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferGetBaseAddressOfPlane;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferGetBytesPerRow;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferGetBytesPerRowOfPlane;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferGetDataSize;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferGetExtendedPixels;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferGetHeight;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferGetHeightOfPlane;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferGetPixelFormatType;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferGetPlaneCount;
#[cfg(feature = "CVPixelBuffer")]
pub use self::__CVPixelBuffer::CVPixelBufferGetTypeID;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferGetWidth;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferGetWidthOfPlane;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferIsCompatibleWithAttributes;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer"
))]
pub use self::__CVPixelBuffer::CVPixelBufferIsPlanar;
#[cfg(all(
    feature = "CVBase",
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer",
    any()
))]
pub use self::__CVPixelBuffer::CVPixelBufferLockBaseAddress;
#[cfg(all(feature = "CVBase", feature = "CVPixelBuffer"))]
pub use self::__CVPixelBuffer::CVPixelBufferLockFlags;
#[cfg(feature = "CVPixelBuffer")]
pub use self::__CVPixelBuffer::CVPixelBufferReleaseBytesCallback;
#[cfg(feature = "CVPixelBuffer")]
pub use self::__CVPixelBuffer::CVPixelBufferReleasePlanarBytesCallback;
#[cfg(all(
    feature = "CVBase",
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer",
    any()
))]
pub use self::__CVPixelBuffer::CVPixelBufferUnlockBaseAddress;
#[cfg(feature = "CVPixelBuffer")]
pub use self::__CVPixelBuffer::CVPlanarComponentInfo;
#[cfg(feature = "CVPixelBuffer")]
pub use self::__CVPixelBuffer::CVPlanarPixelBufferInfo;
#[cfg(feature = "CVPixelBuffer")]
pub use self::__CVPixelBuffer::CVPlanarPixelBufferInfo_YCbCrBiPlanar;
#[cfg(feature = "CVPixelBuffer")]
pub use self::__CVPixelBuffer::CVPlanarPixelBufferInfo_YCbCrPlanar;
#[cfg(any())]
pub use self::__CVPixelBufferIOSurface::kCVPixelBufferIOSurfaceCoreAnimationCompatibilityKey;
#[cfg(any())]
pub use self::__CVPixelBufferIOSurface::kCVPixelBufferIOSurfaceOpenGLESFBOCompatibilityKey;
#[cfg(any())]
pub use self::__CVPixelBufferIOSurface::kCVPixelBufferIOSurfaceOpenGLESTextureCompatibilityKey;
#[cfg(any())]
pub use self::__CVPixelBufferIOSurface::kCVPixelBufferIOSurfaceOpenGLFBOCompatibilityKey;
#[cfg(any())]
pub use self::__CVPixelBufferIOSurface::kCVPixelBufferIOSurfaceOpenGLTextureCompatibilityKey;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer",
    any(),
    any(),
    any()
))]
#[cfg(not(target_os = "watchos"))]
pub use self::__CVPixelBufferIOSurface::CVPixelBufferCreateWithIOSurface;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer",
    any(),
    any()
))]
#[cfg(not(target_os = "watchos"))]
pub use self::__CVPixelBufferIOSurface::CVPixelBufferGetIOSurface;
#[cfg(feature = "CVPixelBufferPool")]
pub use self::__CVPixelBufferPool::CVPixelBufferPool;
#[cfg(all(feature = "CVBase", feature = "CVPixelBufferPool"))]
pub use self::__CVPixelBufferPool::CVPixelBufferPoolFlushFlags;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatBitsPerBlock;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatBitsPerComponent;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatBlackBlock;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatBlockHeight;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatBlockHorizontalAlignment;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatBlockVerticalAlignment;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatBlockWidth;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatCGBitmapContextCompatibility;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatCGBitmapInfo;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatCGImageCompatibility;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatCodecType;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatComponentRange;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatComponentRange_FullRange;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatComponentRange_VideoRange;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatComponentRange_WideRange;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatConstant;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatContainsAlpha;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatContainsGrayscale;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatContainsRGB;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatContainsSenselArray;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatContainsYCbCr;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatFillExtendedPixelsCallback;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatFourCC;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatHorizontalSubsampling;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatName;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatOpenGLCompatibility;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatOpenGLESCompatibility;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatOpenGLFormat;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatOpenGLInternalFormat;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatOpenGLType;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatPlanes;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatQDCompatibility;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::kCVPixelFormatVerticalSubsampling;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer",
    any()
))]
pub use self::__CVPixelFormatDescription::CVFillExtendedPixelsCallBack;
#[cfg(all(
    feature = "CVBuffer",
    feature = "CVImageBuffer",
    feature = "CVPixelBuffer",
    any()
))]
pub use self::__CVPixelFormatDescription::CVFillExtendedPixelsCallBackData;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::CVIsCompressedPixelFormatAvailable;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::CVPixelFormatDescriptionArrayCreateWithAllPixelFormatTypes;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::CVPixelFormatDescriptionCreateWithPixelFormatType;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::CVPixelFormatDescriptionRegisterDescriptionWithPixelFormatType;
#[cfg(any())]
pub use self::__CVPixelFormatDescription::CVPixelFormatTypeCopyFourCharCodeString;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnAllocationFailed;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnDisplayLinkAlreadyRunning;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnDisplayLinkCallbacksNotSet;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnDisplayLinkNotRunning;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnError;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnFirst;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnInvalidArgument;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnInvalidDisplay;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnInvalidPixelBufferAttributes;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnInvalidPixelFormat;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnInvalidPoolAttributes;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnInvalidSize;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnLast;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnPixelBufferNotMetalCompatible;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnPixelBufferNotOpenGLCompatible;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnPoolAllocationFailed;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnRetry;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnSuccess;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnUnsupported;
#[cfg(any())]
pub use self::__CVReturn::kCVReturnWouldExceedAllocationThreshold;
#[cfg(any())]
pub use self::__CVReturn::CVReturn;
