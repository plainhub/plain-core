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
#[link(name = "CoreMedia", kind = "framework")]
extern "C" {}
#[cfg(any())]
#[path = "CMAttachment.rs"]
mod __CMAttachment;
#[cfg(any())]
#[path = "CMAudioClock.rs"]
mod __CMAudioClock;
#[cfg(any())]
#[path = "CMAudioDeviceClock.rs"]
mod __CMAudioDeviceClock;
#[cfg(feature = "CMBase")]
#[path = "CMBase.rs"]
mod __CMBase;
#[cfg(any())]
#[path = "CMBlockBuffer.rs"]
mod __CMBlockBuffer;
#[cfg(any())]
#[path = "CMBufferQueue.rs"]
mod __CMBufferQueue;
#[cfg(feature = "CMFormatDescription")]
#[path = "CMFormatDescription.rs"]
mod __CMFormatDescription;
#[cfg(any())]
#[path = "CMFormatDescriptionBridge.rs"]
mod __CMFormatDescriptionBridge;
#[cfg(any())]
#[path = "CMMemoryPool.rs"]
mod __CMMemoryPool;
#[cfg(any())]
#[path = "CMMetadata.rs"]
mod __CMMetadata;
#[cfg(feature = "CMSampleBuffer")]
#[path = "CMSampleBuffer.rs"]
mod __CMSampleBuffer;
#[cfg(any())]
#[path = "CMSimpleQueue.rs"]
mod __CMSimpleQueue;
#[cfg(feature = "CMSync")]
#[path = "CMSync.rs"]
mod __CMSync;
#[cfg(feature = "CMTag")]
#[path = "CMTag.rs"]
mod __CMTag;
#[cfg(feature = "CMTagCollection")]
#[path = "CMTagCollection.rs"]
mod __CMTagCollection;
#[cfg(feature = "CMTaggedBufferGroup")]
#[path = "CMTaggedBufferGroup.rs"]
mod __CMTaggedBufferGroup;
#[cfg(any())]
#[path = "CMTextMarkup.rs"]
mod __CMTextMarkup;
#[cfg(feature = "CMTime")]
#[path = "CMTime.rs"]
mod __CMTime;
#[cfg(feature = "CMTimeRange")]
#[path = "CMTimeRange.rs"]
mod __CMTimeRange;
#[cfg(any())]
pub use self::__CMAttachment::kCMAttachmentMode_ShouldNotPropagate;
#[cfg(any())]
pub use self::__CMAttachment::kCMAttachmentMode_ShouldPropagate;
#[cfg(any())]
pub use self::__CMAttachment::CMAttachmentBearer;
#[cfg(any())]
pub use self::__CMAttachment::CMAttachmentMode;
#[cfg(any())]
pub use self::__CMAttachment::CMCopyDictionaryOfAttachments;
#[cfg(any())]
pub use self::__CMAttachment::CMGetAttachment;
#[cfg(any())]
pub use self::__CMAttachment::CMPropagateAttachments;
#[cfg(any())]
pub use self::__CMAttachment::CMRemoveAllAttachments;
#[cfg(any())]
pub use self::__CMAttachment::CMRemoveAttachment;
#[cfg(any())]
pub use self::__CMAttachment::CMSetAttachment;
#[cfg(any())]
pub use self::__CMAttachment::CMSetAttachments;
#[cfg(all(any(), feature = "CMSync"))]
pub use self::__CMAudioClock::CMAudioClockCreate;
#[cfg(all(any(), feature = "CMSync"))]
pub use self::__CMAudioDeviceClock::CMAudioDeviceClockCreate;
#[cfg(all(any(), feature = "CMSync", any()))]
pub use self::__CMAudioDeviceClock::CMAudioDeviceClockCreateFromAudioDeviceID;
#[cfg(all(any(), feature = "CMSync", any()))]
pub use self::__CMAudioDeviceClock::CMAudioDeviceClockGetAudioDevice;
#[cfg(all(any(), feature = "CMSync", any()))]
pub use self::__CMAudioDeviceClock::CMAudioDeviceClockSetAudioDeviceID;
#[cfg(all(any(), feature = "CMSync"))]
pub use self::__CMAudioDeviceClock::CMAudioDeviceClockSetAudioDeviceUID;
#[cfg(feature = "CMBase")]
pub use self::__CMBase::CMItemCount;
#[cfg(feature = "CMBase")]
pub use self::__CMBase::CMItemIndex;
#[cfg(feature = "CMBase")]
pub use self::__CMBase::CMPersistentTrackID;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferAlwaysCopyDataFlag;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferAssureMemoryNowFlag;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferBadCustomBlockSourceErr;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferBadLengthParameterErr;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferBadOffsetParameterErr;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferBadPointerParameterErr;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferBlockAllocationFailedErr;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferCustomBlockSourceVersion;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferDontOptimizeDepthFlag;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferEmptyBBufErr;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferInsufficientSpaceErr;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferNoErr;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferPermitEmptyReferenceFlag;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferStructureAllocationFailedErr;
#[cfg(any())]
pub use self::__CMBlockBuffer::kCMBlockBufferUnallocatedBlockErr;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBuffer;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferAccessDataBytes;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferAppendBufferReference;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferAppendMemoryBlock;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferAssureBlockMemory;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferCopyDataBytes;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferCreateContiguous;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferCreateEmpty;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferCreateWithBufferReference;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferCreateWithMemoryBlock;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferCustomBlockSource;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferFillDataBytes;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferFlags;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferGetDataLength;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferGetDataPointer;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferIsEmpty;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferIsRangeContiguous;
#[cfg(any())]
pub use self::__CMBlockBuffer::CMBlockBufferReplaceDataBytes;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueError_AllocationFailed;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueError_BadTriggerDuration;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueError_CannotModifyQueueFromTriggerCallback;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueError_EnqueueAfterEndOfData;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueError_InvalidBuffer;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueError_InvalidCMBufferCallbacksStruct;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueError_InvalidTriggerCondition;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueError_InvalidTriggerToken;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueError_QueueIsFull;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueError_RequiredParameterMissing;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueTrigger_WhenBufferCountBecomesGreaterThan;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueTrigger_WhenBufferCountBecomesLessThan;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueTrigger_WhenDataBecomesReady;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueTrigger_WhenDurationBecomesGreaterThan;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueTrigger_WhenDurationBecomesGreaterThanOrEqualTo;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueTrigger_WhenDurationBecomesGreaterThanOrEqualToAndBufferCountBecomesGreaterThan;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueTrigger_WhenDurationBecomesLessThan;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueTrigger_WhenDurationBecomesLessThanOrEqualTo;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueTrigger_WhenEndOfDataReached;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueTrigger_WhenMaxPresentationTimeStampChanges;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueTrigger_WhenMinPresentationTimeStampChanges;
#[cfg(any())]
pub use self::__CMBufferQueue::kCMBufferQueueTrigger_WhenReset;
#[cfg(any())]
pub use self::__CMBufferQueue::opaqueCMBufferQueueTriggerToken;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBuffer;
#[cfg(all(any(), feature = "CMTime"))]
pub use self::__CMBufferQueue::CMBufferCallbacks;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferCompareCallback;
#[cfg(all(any(), any()))]
pub use self::__CMBufferQueue::CMBufferCompareHandler;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferGetBooleanCallback;
#[cfg(all(any(), any()))]
pub use self::__CMBufferQueue::CMBufferGetBooleanHandler;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferGetSizeCallback;
#[cfg(all(any(), any()))]
pub use self::__CMBufferQueue::CMBufferGetSizeHandler;
#[cfg(all(any(), feature = "CMTime"))]
pub use self::__CMBufferQueue::CMBufferGetTimeCallback;
#[cfg(all(any(), feature = "CMTime", any()))]
pub use self::__CMBufferQueue::CMBufferGetTimeHandler;
#[cfg(all(any(), feature = "CMTime", any()))]
pub use self::__CMBufferQueue::CMBufferHandlers;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueue;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueCallForEachBuffer;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueContainsEndOfData;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueCopyHead;
#[cfg(all(feature = "CMBase", any(), feature = "CMTime"))]
pub use self::__CMBufferQueue::CMBufferQueueCreate;
#[cfg(all(feature = "CMBase", any(), feature = "CMTime", any()))]
pub use self::__CMBufferQueue::CMBufferQueueCreateWithHandlers;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueDequeueAndRetain;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueDequeueIfDataReadyAndRetain;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueEnqueue;
#[cfg(all(feature = "CMBase", any()))]
pub use self::__CMBufferQueue::CMBufferQueueGetBufferCount;
#[cfg(all(any(), feature = "CMTime"))]
pub use self::__CMBufferQueue::CMBufferQueueGetCallbacksForSampleBuffersSortedByOutputPTS;
#[cfg(all(any(), feature = "CMTime"))]
pub use self::__CMBufferQueue::CMBufferQueueGetCallbacksForUnsortedSampleBuffers;
#[cfg(all(any(), feature = "CMTime"))]
pub use self::__CMBufferQueue::CMBufferQueueGetDuration;
#[cfg(all(any(), feature = "CMTime"))]
pub use self::__CMBufferQueue::CMBufferQueueGetEndPresentationTimeStamp;
#[cfg(all(any(), feature = "CMTime"))]
pub use self::__CMBufferQueue::CMBufferQueueGetFirstDecodeTimeStamp;
#[cfg(all(any(), feature = "CMTime"))]
pub use self::__CMBufferQueue::CMBufferQueueGetFirstPresentationTimeStamp;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueGetHead;
#[cfg(all(any(), feature = "CMTime"))]
pub use self::__CMBufferQueue::CMBufferQueueGetMaxPresentationTimeStamp;
#[cfg(all(any(), feature = "CMTime"))]
pub use self::__CMBufferQueue::CMBufferQueueGetMinDecodeTimeStamp;
#[cfg(all(any(), feature = "CMTime"))]
pub use self::__CMBufferQueue::CMBufferQueueGetMinPresentationTimeStamp;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueGetTotalSize;
#[cfg(all(any(), feature = "CMTime"))]
pub use self::__CMBufferQueue::CMBufferQueueInstallTrigger;
#[cfg(all(any(), feature = "CMTime", any()))]
pub use self::__CMBufferQueue::CMBufferQueueInstallTriggerHandler;
#[cfg(all(feature = "CMBase", any(), any()))]
pub use self::__CMBufferQueue::CMBufferQueueInstallTriggerHandlerWithIntegerThreshold;
#[cfg(all(feature = "CMBase", any()))]
pub use self::__CMBufferQueue::CMBufferQueueInstallTriggerWithIntegerThreshold;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueIsAtEndOfData;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueIsEmpty;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueMarkEndOfData;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueRemoveTrigger;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueReset;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueResetWithCallback;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueSetValidationCallback;
#[cfg(all(any(), any()))]
pub use self::__CMBufferQueue::CMBufferQueueSetValidationHandler;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueTestTrigger;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueTriggerCallback;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueTriggerCondition;
#[cfg(all(any(), any()))]
pub use self::__CMBufferQueue::CMBufferQueueTriggerHandler;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferQueueTriggerToken;
#[cfg(any())]
pub use self::__CMBufferQueue::CMBufferValidationCallback;
#[cfg(all(any(), any()))]
pub use self::__CMBufferQueue::CMBufferValidationHandler;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMAudioCodecType;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMAudioFormatDescription;
#[cfg(all(feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescription::CMAudioFormatDescriptionCreate;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMAudioFormatDescriptionCreateSummary;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMAudioFormatDescriptionEqual;
#[cfg(all(feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescription::CMAudioFormatDescriptionGetChannelLayout;
#[cfg(all(feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescription::CMAudioFormatDescriptionGetFormatList;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMAudioFormatDescriptionGetMagicCookie;
#[cfg(all(feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescription::CMAudioFormatDescriptionGetMostCompatibleFormat;
#[cfg(all(feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescription::CMAudioFormatDescriptionGetRichestDecodableFormat;
#[cfg(all(feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescription::CMAudioFormatDescriptionGetStreamBasicDescription;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMAudioFormatDescriptionMask;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMClosedCaptionFormatDescription;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMClosedCaptionFormatType;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMFormatDescription;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMMediaType;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMMetadataFormatDescription;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMMetadataFormatDescriptionCreateByMergingMetadataFormatDescriptions;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMMetadataFormatDescriptionCreateWithKeys;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMMetadataFormatDescriptionCreateWithMetadataFormatDescriptionAndMetadataSpecifications;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMMetadataFormatDescriptionCreateWithMetadataSpecifications;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMMetadataFormatDescriptionGetIdentifiers;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMMetadataFormatDescriptionGetKeyWithLocalID;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMMetadataFormatType;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMMuxedFormatDescription;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMMuxedFormatDescriptionCreate;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMMuxedStreamType;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMPixelFormatType;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMSubtitleFormatType;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMTaggedBufferGroupFormatDescription;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMTaggedBufferGroupFormatType;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMTextDisplayFlags;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMTextFormatDescription;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMTextFormatDescriptionGetDefaultTextBox;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMTextFormatDescriptionGetDisplayFlags;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMTextFormatDescriptionGetFontName;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMTextFormatDescriptionGetJustification;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMTextFormatType;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMTextJustificationValue;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMTimeCodeFormatDescription;
#[cfg(all(feature = "CMFormatDescription", feature = "CMTime"))]
pub use self::__CMFormatDescription::CMTimeCodeFormatDescriptionCreate;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMTimeCodeFormatDescriptionGetFrameQuanta;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMTimeCodeFormatDescriptionGetTimeCodeFlags;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMTimeCodeFormatType;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMVideoCodecType;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMVideoDimensions;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMVideoFormatDescription;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMVideoFormatDescriptionCopyTagCollectionArray;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMVideoFormatDescriptionCreate;
#[cfg(all(feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescription::CMVideoFormatDescriptionCreateForImageBuffer;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMVideoFormatDescriptionCreateFromH264ParameterSets;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMVideoFormatDescriptionCreateFromHEVCParameterSets;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMVideoFormatDescriptionGetCleanAperture;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMVideoFormatDescriptionGetDimensions;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMVideoFormatDescriptionGetExtensionKeysCommonWithImageBuffers;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMVideoFormatDescriptionGetH264ParameterSetAtIndex;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMVideoFormatDescriptionGetHEVCParameterSetAtIndex;
#[cfg(feature = "CMFormatDescription")]
pub use self::__CMFormatDescription::CMVideoFormatDescriptionGetPresentationDimensions;
#[cfg(all(feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescription::CMVideoFormatDescriptionMatchesImageBuffer;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMFormatDescriptionBridgeError_AllocationFailed;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMFormatDescriptionBridgeError_IncompatibleFormatDescription;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMFormatDescriptionBridgeError_InvalidFormatDescription;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMFormatDescriptionBridgeError_InvalidParameter;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMFormatDescriptionBridgeError_InvalidSerializedSampleDescription;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMFormatDescriptionBridgeError_InvalidSlice;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMFormatDescriptionBridgeError_UnsupportedSampleDescriptionFlavor;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMImageDescriptionFlavor_3GPFamily;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMImageDescriptionFlavor_ISOFamily;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMImageDescriptionFlavor_ISOFamilyWithAppleExtensions;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMImageDescriptionFlavor_QuickTimeMovie;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMSoundDescriptionFlavor_3GPFamily;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMSoundDescriptionFlavor_ISOFamily;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMSoundDescriptionFlavor_QuickTimeMovie;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::kCMSoundDescriptionFlavor_QuickTimeMovieV2;
#[cfg(all(any(), feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMAudioFormatDescriptionCopyAsBigEndianSoundDescriptionBlockBuffer;
#[cfg(all(any(), feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMAudioFormatDescriptionCreateFromBigEndianSoundDescriptionBlockBuffer;
#[cfg(all(feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMAudioFormatDescriptionCreateFromBigEndianSoundDescriptionData;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMClosedCaptionDescriptionFlavor;
#[cfg(all(any(), feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMClosedCaptionFormatDescriptionCopyAsBigEndianClosedCaptionDescriptionBlockBuffer;
#[cfg(all(any(), feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMClosedCaptionFormatDescriptionCreateFromBigEndianClosedCaptionDescriptionBlockBuffer;
#[cfg(all(feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMClosedCaptionFormatDescriptionCreateFromBigEndianClosedCaptionDescriptionData;
#[cfg(all(any(), any()))]
pub use self::__CMFormatDescriptionBridge::CMDoesBigEndianSoundDescriptionRequireLegacyCBRSampleTableLayout;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMImageDescriptionFlavor;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMMetadataDescriptionFlavor;
#[cfg(all(any(), feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMMetadataFormatDescriptionCopyAsBigEndianMetadataDescriptionBlockBuffer;
#[cfg(all(any(), feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMMetadataFormatDescriptionCreateFromBigEndianMetadataDescriptionBlockBuffer;
#[cfg(all(feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMMetadataFormatDescriptionCreateFromBigEndianMetadataDescriptionData;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMSoundDescriptionFlavor;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMSwapBigEndianClosedCaptionDescriptionToHost;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMSwapBigEndianImageDescriptionToHost;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMSwapBigEndianMetadataDescriptionToHost;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMSwapBigEndianSoundDescriptionToHost;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMSwapBigEndianTextDescriptionToHost;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMSwapBigEndianTimeCodeDescriptionToHost;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMSwapHostEndianClosedCaptionDescriptionToBig;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMSwapHostEndianImageDescriptionToBig;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMSwapHostEndianMetadataDescriptionToBig;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMSwapHostEndianSoundDescriptionToBig;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMSwapHostEndianTextDescriptionToBig;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMSwapHostEndianTimeCodeDescriptionToBig;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMTextDescriptionFlavor;
#[cfg(all(any(), feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMTextFormatDescriptionCopyAsBigEndianTextDescriptionBlockBuffer;
#[cfg(all(any(), feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMTextFormatDescriptionCreateFromBigEndianTextDescriptionBlockBuffer;
#[cfg(all(feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMTextFormatDescriptionCreateFromBigEndianTextDescriptionData;
#[cfg(any())]
pub use self::__CMFormatDescriptionBridge::CMTimeCodeDescriptionFlavor;
#[cfg(all(any(), feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMTimeCodeFormatDescriptionCopyAsBigEndianTimeCodeDescriptionBlockBuffer;
#[cfg(all(any(), feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMTimeCodeFormatDescriptionCreateFromBigEndianTimeCodeDescriptionBlockBuffer;
#[cfg(all(feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMTimeCodeFormatDescriptionCreateFromBigEndianTimeCodeDescriptionData;
#[cfg(all(any(), feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMVideoFormatDescriptionCopyAsBigEndianImageDescriptionBlockBuffer;
#[cfg(all(any(), feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMVideoFormatDescriptionCreateFromBigEndianImageDescriptionBlockBuffer;
#[cfg(all(feature = "CMFormatDescription", any()))]
pub use self::__CMFormatDescriptionBridge::CMVideoFormatDescriptionCreateFromBigEndianImageDescriptionData;
#[cfg(any())]
pub use self::__CMMemoryPool::kCMMemoryPoolError_AllocationFailed;
#[cfg(any())]
pub use self::__CMMemoryPool::kCMMemoryPoolError_InvalidParameter;
#[cfg(any())]
pub use self::__CMMemoryPool::kCMMemoryPoolOption_AgeOutPeriod;
#[cfg(any())]
pub use self::__CMMemoryPool::CMMemoryPool;
#[cfg(any())]
pub use self::__CMMemoryPool::CMMemoryPoolCreate;
#[cfg(any())]
pub use self::__CMMemoryPool::CMMemoryPoolFlush;
#[cfg(any())]
pub use self::__CMMemoryPool::CMMemoryPoolGetAllocator;
#[cfg(any())]
pub use self::__CMMemoryPool::CMMemoryPoolInvalidate;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_AffineTransformF64;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_BMP;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_DimensionsF32;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_ExtendedRasterRectangleValue;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_Float32;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_Float64;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_GIF;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_JPEG;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_JSON;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_PNG;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_PerspectiveTransformF64;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_PointF32;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_PolygonF32;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_PolylineF32;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_RasterRectangleValue;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_RawData;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_RectF32;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_SInt16;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_SInt32;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_SInt64;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_SInt8;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_UInt16;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_UInt32;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_UInt64;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_UInt8;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_UTF16;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataBaseDataType_UTF8;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataDataTypeRegistryError_AllocationFailed;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataDataTypeRegistryError_BadDataTypeIdentifier;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataDataTypeRegistryError_DataTypeAlreadyRegistered;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataDataTypeRegistryError_MultipleConformingBaseTypes;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataDataTypeRegistryError_RequiredParameterMissing;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataDataTypeRegistryError_RequiresConformingBaseType;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataDataType_QuickTimeMetadataDirection;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataDataType_QuickTimeMetadataLocation_ISO6709;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataDataType_QuickTimeMetadataMilliLux;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataDataType_QuickTimeMetadataUUID;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifierError_AllocationFailed;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifierError_BadIdentifier;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifierError_BadKey;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifierError_BadKeyLength;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifierError_BadKeySpace;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifierError_BadKeyType;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifierError_BadNumberKey;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifierError_NoKeyValueAvailable;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifierError_RequiredParameterMissing;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifier_QuickTimeMetadataDirection_Facing;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifier_QuickTimeMetadataDisplayMaskRectangleMono;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifier_QuickTimeMetadataDisplayMaskRectangleStereoLeft;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifier_QuickTimeMetadataDisplayMaskRectangleStereoRight;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifier_QuickTimeMetadataLivePhotoStillImageTransform;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifier_QuickTimeMetadataLivePhotoStillImageTransformReferenceDimensions;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifier_QuickTimeMetadataLocation_ISO6709;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifier_QuickTimeMetadataPreferredAffineTransform;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifier_QuickTimeMetadataPresentationImmersiveMedia;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifier_QuickTimeMetadataSceneIlluminance;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifier_QuickTimeMetadataSegmentIdentifier;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifier_QuickTimeMetadataSpatialAudioMix;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataIdentifier_QuickTimeMetadataVideoOrientation;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataKeySpace_HLSDateRange;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataKeySpace_ID3;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataKeySpace_ISOUserData;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataKeySpace_Icy;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataKeySpace_QuickTimeMetadata;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataKeySpace_QuickTimeUserData;
#[cfg(any())]
pub use self::__CMMetadata::kCMMetadataKeySpace_iTunes;
#[cfg(any())]
pub use self::__CMMetadata::CMMetadataCreateIdentifierForKeyAndKeySpace;
#[cfg(any())]
pub use self::__CMMetadata::CMMetadataCreateKeyFromIdentifier;
#[cfg(any())]
pub use self::__CMMetadata::CMMetadataCreateKeyFromIdentifierAsCFData;
#[cfg(any())]
pub use self::__CMMetadata::CMMetadataCreateKeySpaceFromIdentifier;
#[cfg(any())]
pub use self::__CMMetadata::CMMetadataDataTypeRegistryDataTypeConformsToDataType;
#[cfg(any())]
pub use self::__CMMetadata::CMMetadataDataTypeRegistryDataTypeIsBaseDataType;
#[cfg(any())]
pub use self::__CMMetadata::CMMetadataDataTypeRegistryDataTypeIsRegistered;
#[cfg(any())]
pub use self::__CMMetadata::CMMetadataDataTypeRegistryGetBaseDataTypeForConformingDataType;
#[cfg(any())]
pub use self::__CMMetadata::CMMetadataDataTypeRegistryGetBaseDataTypes;
#[cfg(any())]
pub use self::__CMMetadata::CMMetadataDataTypeRegistryGetConformingDataTypes;
#[cfg(any())]
pub use self::__CMMetadata::CMMetadataDataTypeRegistryGetDataTypeDescription;
#[cfg(any())]
pub use self::__CMMetadata::CMMetadataDataTypeRegistryRegisterDataType;
#[cfg(all(
    feature = "CMBase",
    any(),
    feature = "CMFormatDescription",
    feature = "CMSampleBuffer",
    feature = "CMTime",
    any()
))]
pub use self::__CMSampleBuffer::CMAudioSampleBufferCreateReadyWithPacketDescriptions;
#[cfg(all(
    feature = "CMBase",
    any(),
    feature = "CMFormatDescription",
    feature = "CMSampleBuffer",
    feature = "CMTime",
    any()
))]
pub use self::__CMSampleBuffer::CMAudioSampleBufferCreateWithPacketDescriptions;
#[cfg(all(
    feature = "CMBase",
    any(),
    feature = "CMFormatDescription",
    feature = "CMSampleBuffer",
    feature = "CMTime",
    any(),
    any()
))]
pub use self::__CMSampleBuffer::CMAudioSampleBufferCreateWithPacketDescriptionsAndMakeDataReadyHandler;
#[cfg(feature = "CMSampleBuffer")]
pub use self::__CMSampleBuffer::CMSampleBuffer;
#[cfg(feature = "CMSampleBuffer")]
pub use self::__CMSampleBuffer::CMSampleBufferInvalidateCallback;
#[cfg(all(feature = "CMSampleBuffer", any()))]
pub use self::__CMSampleBuffer::CMSampleBufferInvalidateHandler;
#[cfg(feature = "CMSampleBuffer")]
pub use self::__CMSampleBuffer::CMSampleBufferMakeDataReadyCallback;
#[cfg(all(feature = "CMSampleBuffer", any()))]
pub use self::__CMSampleBuffer::CMSampleBufferMakeDataReadyHandler;
#[cfg(all(feature = "CMSampleBuffer", feature = "CMTime"))]
pub use self::__CMSampleBuffer::CMSampleTimingInfo;
#[cfg(any())]
pub use self::__CMSimpleQueue::kCMSimpleQueueError_AllocationFailed;
#[cfg(any())]
pub use self::__CMSimpleQueue::kCMSimpleQueueError_ParameterOutOfRange;
#[cfg(any())]
pub use self::__CMSimpleQueue::kCMSimpleQueueError_QueueIsFull;
#[cfg(any())]
pub use self::__CMSimpleQueue::kCMSimpleQueueError_RequiredParameterMissing;
#[cfg(any())]
pub use self::__CMSimpleQueue::CMSimpleQueue;
#[cfg(any())]
pub use self::__CMSimpleQueue::CMSimpleQueueCreate;
#[cfg(any())]
pub use self::__CMSimpleQueue::CMSimpleQueueDequeue;
#[cfg(any())]
pub use self::__CMSimpleQueue::CMSimpleQueueEnqueue;
#[cfg(any())]
pub use self::__CMSimpleQueue::CMSimpleQueueGetCapacity;
#[cfg(any())]
pub use self::__CMSimpleQueue::CMSimpleQueueGetCount;
#[cfg(any())]
pub use self::__CMSimpleQueue::CMSimpleQueueGetHead;
#[cfg(any())]
pub use self::__CMSimpleQueue::CMSimpleQueueReset;
#[cfg(feature = "CMSync")]
pub use self::__CMSync::CMClock;
#[cfg(feature = "CMSync")]
pub use self::__CMSync::CMClockOrTimebase;
#[cfg(all(feature = "CMSync", feature = "CMTime"))]
pub use self::__CMSync::CMSyncConvertTime;
#[cfg(feature = "CMSync")]
pub use self::__CMSync::CMSyncGetRelativeRate;
#[cfg(all(feature = "CMSync", feature = "CMTime"))]
pub use self::__CMSync::CMSyncGetRelativeRateAndAnchorTime;
#[cfg(all(feature = "CMSync", feature = "CMTime"))]
pub use self::__CMSync::CMSyncGetTime;
#[cfg(feature = "CMSync")]
pub use self::__CMSync::CMSyncMightDrift;
#[cfg(feature = "CMSync")]
pub use self::__CMSync::CMTimebase;
#[cfg(feature = "CMTag")]
pub use self::__CMTag::CMPackingType;
#[cfg(feature = "CMTag")]
pub use self::__CMTag::CMProjectionType;
#[cfg(feature = "CMTag")]
pub use self::__CMTag::CMStereoViewComponents;
#[cfg(feature = "CMTag")]
pub use self::__CMTag::CMStereoViewInterpretationOptions;
#[cfg(feature = "CMTag")]
pub use self::__CMTag::CMTag;
#[cfg(feature = "CMTag")]
pub use self::__CMTag::CMTagCategory;
#[cfg(feature = "CMTag")]
pub use self::__CMTag::CMTagDataType;
#[cfg(feature = "CMTag")]
pub use self::__CMTag::CMTagError;
#[cfg(feature = "CMTag")]
pub use self::__CMTag::CMTagValue;
#[cfg(feature = "CMTagCollection")]
pub use self::__CMTagCollection::CMMutableTagCollection;
#[cfg(feature = "CMTagCollection")]
pub use self::__CMTagCollection::CMTagCollection;
#[cfg(all(feature = "CMTag", feature = "CMTagCollection"))]
pub use self::__CMTagCollection::CMTagCollectionApplierFunction;
#[cfg(feature = "CMTagCollection")]
pub use self::__CMTagCollection::CMTagCollectionError;
#[cfg(all(feature = "CMTag", feature = "CMTagCollection"))]
pub use self::__CMTagCollection::CMTagCollectionTagFilterFunction;
#[cfg(all(
    feature = "CMFormatDescription",
    feature = "CMSampleBuffer",
    feature = "CMTaggedBufferGroup",
    feature = "CMTime"
))]
pub use self::__CMTaggedBufferGroup::CMSampleBufferCreateForTaggedBufferGroup;
#[cfg(feature = "CMTaggedBufferGroup")]
pub use self::__CMTaggedBufferGroup::CMTaggedBufferGroup;
#[cfg(feature = "CMTaggedBufferGroup")]
pub use self::__CMTaggedBufferGroup::CMTaggedBufferGroupError;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAlignmentType_End;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAlignmentType_Left;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAlignmentType_Middle;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAlignmentType_Right;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAlignmentType_Start;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_Alignment;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_BackgroundColorARGB;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_BaseFontSizePercentageRelativeToVideoHeight;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_BoldStyle;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_CharacterBackgroundColorARGB;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_CharacterEdgeStyle;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_FontFamilyName;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_FontFamilyNameList;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_ForegroundColorARGB;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_GenericFontFamilyName;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_ItalicStyle;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_OrthogonalLinePositionPercentageRelativeToWritingDirection;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_RelativeFontSize;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_TextPositionPercentageRelativeToWritingDirection;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_UnderlineStyle;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_VerticalLayout;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupAttribute_WritingDirectionSizePercentage;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupCharacterEdgeStyle_Depressed;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupCharacterEdgeStyle_DropShadow;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupCharacterEdgeStyle_None;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupCharacterEdgeStyle_Raised;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupCharacterEdgeStyle_Uniform;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupGenericFontName_Casual;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupGenericFontName_Cursive;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupGenericFontName_Default;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupGenericFontName_Fantasy;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupGenericFontName_Monospace;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupGenericFontName_MonospaceSansSerif;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupGenericFontName_MonospaceSerif;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupGenericFontName_ProportionalSansSerif;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupGenericFontName_ProportionalSerif;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupGenericFontName_SansSerif;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupGenericFontName_Serif;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextMarkupGenericFontName_SmallCapital;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextVerticalLayout_LeftToRight;
#[cfg(any())]
pub use self::__CMTextMarkup::kCMTextVerticalLayout_RightToLeft;
#[cfg(feature = "CMTime")]
pub use self::__CMTime::kCMTimeZero;
#[cfg(feature = "CMTime")]
pub use self::__CMTime::CMTime;
#[cfg(feature = "CMTime")]
pub use self::__CMTime::CMTimeEpoch;
#[cfg(feature = "CMTime")]
pub use self::__CMTime::CMTimeFlags;
#[cfg(feature = "CMTime")]
pub use self::__CMTime::CMTimeRoundingMethod;
#[cfg(feature = "CMTime")]
pub use self::__CMTime::CMTimeScale;
#[cfg(feature = "CMTime")]
pub use self::__CMTime::CMTimeValue;
#[cfg(all(feature = "CMTime", feature = "CMTimeRange"))]
pub use self::__CMTimeRange::CMTimeMapping;
#[cfg(all(feature = "CMTime", feature = "CMTimeRange"))]
pub use self::__CMTimeRange::CMTimeRange;
