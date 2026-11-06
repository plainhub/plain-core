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
#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}
#[cfg(any())]
#[path = "AVAnimation.rs"]
mod __AVAnimation;
#[cfg(feature = "AVAsset")]
#[path = "AVAsset.rs"]
mod __AVAsset;
#[cfg(any())]
#[path = "AVAssetCache.rs"]
mod __AVAssetCache;
#[cfg(any())]
#[path = "AVAssetDownloadStorageManager.rs"]
mod __AVAssetDownloadStorageManager;
#[cfg(any())]
#[path = "AVAssetDownloadTask.rs"]
mod __AVAssetDownloadTask;
#[cfg(any())]
#[path = "AVAssetExportSession.rs"]
mod __AVAssetExportSession;
#[cfg(any())]
#[path = "AVAssetImageGenerator.rs"]
mod __AVAssetImageGenerator;
#[cfg(any())]
#[path = "AVAssetPlaybackAssistant.rs"]
mod __AVAssetPlaybackAssistant;
#[cfg(feature = "AVAssetReader")]
#[path = "AVAssetReader.rs"]
mod __AVAssetReader;
#[cfg(feature = "AVAssetReaderOutput")]
#[path = "AVAssetReaderOutput.rs"]
mod __AVAssetReaderOutput;
#[cfg(any())]
#[path = "AVAssetResourceLoader.rs"]
mod __AVAssetResourceLoader;
#[cfg(any())]
#[path = "AVAssetSegmentReport.rs"]
mod __AVAssetSegmentReport;
#[cfg(feature = "AVAssetTrack")]
#[path = "AVAssetTrack.rs"]
mod __AVAssetTrack;
#[cfg(any())]
#[path = "AVAssetTrackGroup.rs"]
mod __AVAssetTrackGroup;
#[cfg(any())]
#[path = "AVAssetTrackSegment.rs"]
mod __AVAssetTrackSegment;
#[cfg(any())]
#[path = "AVAssetVariant.rs"]
mod __AVAssetVariant;
#[cfg(feature = "AVAssetWriter")]
#[path = "AVAssetWriter.rs"]
mod __AVAssetWriter;
#[cfg(feature = "AVAssetWriterInput")]
#[path = "AVAssetWriterInput.rs"]
mod __AVAssetWriterInput;
#[cfg(any())]
#[path = "AVAsynchronousKeyValueLoading.rs"]
mod __AVAsynchronousKeyValueLoading;
#[cfg(any())]
#[path = "AVAudioBuffer.rs"]
mod __AVAudioBuffer;
#[cfg(any())]
#[path = "AVAudioChannelLayout.rs"]
mod __AVAudioChannelLayout;
#[cfg(any())]
#[path = "AVAudioConnectionPoint.rs"]
mod __AVAudioConnectionPoint;
#[cfg(any())]
#[path = "AVAudioConverter.rs"]
mod __AVAudioConverter;
#[cfg(any())]
#[path = "AVAudioEngine.rs"]
mod __AVAudioEngine;
#[cfg(any())]
#[path = "AVAudioEnvironmentNode.rs"]
mod __AVAudioEnvironmentNode;
#[cfg(any())]
#[path = "AVAudioFile.rs"]
mod __AVAudioFile;
#[cfg(any())]
#[path = "AVAudioFormat.rs"]
mod __AVAudioFormat;
#[cfg(any())]
#[path = "AVAudioIONode.rs"]
mod __AVAudioIONode;
#[cfg(any())]
#[path = "AVAudioMix.rs"]
mod __AVAudioMix;
#[cfg(any())]
#[path = "AVAudioMixerNode.rs"]
mod __AVAudioMixerNode;
#[cfg(any())]
#[path = "AVAudioMixing.rs"]
mod __AVAudioMixing;
#[cfg(any())]
#[path = "AVAudioNode.rs"]
mod __AVAudioNode;
#[cfg(any())]
#[path = "AVAudioPlayer.rs"]
mod __AVAudioPlayer;
#[cfg(any())]
#[path = "AVAudioPlayerNode.rs"]
mod __AVAudioPlayerNode;
#[cfg(any())]
#[path = "AVAudioProcessingSettings.rs"]
mod __AVAudioProcessingSettings;
#[cfg(any())]
#[path = "AVAudioRecorder.rs"]
mod __AVAudioRecorder;
#[cfg(any())]
#[path = "AVAudioRoutingArbiter.rs"]
mod __AVAudioRoutingArbiter;
#[cfg(any())]
#[path = "AVAudioSequencer.rs"]
mod __AVAudioSequencer;
#[cfg(any())]
#[path = "AVAudioSession.rs"]
mod __AVAudioSession;
#[cfg(any())]
#[path = "AVAudioSessionDeprecated.rs"]
mod __AVAudioSessionDeprecated;
#[cfg(any())]
#[path = "AVAudioSessionRoute.rs"]
mod __AVAudioSessionRoute;
#[cfg(any())]
#[path = "AVAudioSessionTypes.rs"]
mod __AVAudioSessionTypes;
#[cfg(any())]
#[path = "AVAudioSettings.rs"]
mod __AVAudioSettings;
#[cfg(any())]
#[path = "AVAudioTime.rs"]
mod __AVAudioTime;
#[cfg(any())]
#[path = "AVAudioTypes.rs"]
mod __AVAudioTypes;
#[cfg(any())]
#[path = "AVAudioUnit.rs"]
mod __AVAudioUnit;
#[cfg(any())]
#[path = "AVAudioUnitComponent.rs"]
mod __AVAudioUnitComponent;
#[cfg(any())]
#[path = "AVAudioUnitDelay.rs"]
mod __AVAudioUnitDelay;
#[cfg(any())]
#[path = "AVAudioUnitDistortion.rs"]
mod __AVAudioUnitDistortion;
#[cfg(any())]
#[path = "AVAudioUnitEQ.rs"]
mod __AVAudioUnitEQ;
#[cfg(any())]
#[path = "AVAudioUnitEffect.rs"]
mod __AVAudioUnitEffect;
#[cfg(any())]
#[path = "AVAudioUnitGenerator.rs"]
mod __AVAudioUnitGenerator;
#[cfg(any())]
#[path = "AVAudioUnitMIDIInstrument.rs"]
mod __AVAudioUnitMIDIInstrument;
#[cfg(any())]
#[path = "AVAudioUnitReverb.rs"]
mod __AVAudioUnitReverb;
#[cfg(any())]
#[path = "AVAudioUnitSampler.rs"]
mod __AVAudioUnitSampler;
#[cfg(any())]
#[path = "AVAudioUnitTimeEffect.rs"]
mod __AVAudioUnitTimeEffect;
#[cfg(any())]
#[path = "AVAudioUnitTimePitch.rs"]
mod __AVAudioUnitTimePitch;
#[cfg(any())]
#[path = "AVAudioUnitVarispeed.rs"]
mod __AVAudioUnitVarispeed;
#[cfg(any())]
#[path = "AVBase.rs"]
mod __AVBase;
#[cfg(any())]
#[path = "AVCameraCalibrationData.rs"]
mod __AVCameraCalibrationData;
#[cfg(any())]
#[path = "AVCaption.rs"]
mod __AVCaption;
#[cfg(any())]
#[path = "AVCaptionConversionValidator.rs"]
mod __AVCaptionConversionValidator;
#[cfg(any())]
#[path = "AVCaptionFormatConformer.rs"]
mod __AVCaptionFormatConformer;
#[cfg(any())]
#[path = "AVCaptionGroup.rs"]
mod __AVCaptionGroup;
#[cfg(any())]
#[path = "AVCaptionGrouper.rs"]
mod __AVCaptionGrouper;
#[cfg(any())]
#[path = "AVCaptionRenderer.rs"]
mod __AVCaptionRenderer;
#[cfg(any())]
#[path = "AVCaptionSettings.rs"]
mod __AVCaptionSettings;
#[cfg(any())]
#[path = "AVCaptureAudioDataOutput.rs"]
mod __AVCaptureAudioDataOutput;
#[cfg(any())]
#[path = "AVCaptureAudioPreviewOutput.rs"]
mod __AVCaptureAudioPreviewOutput;
#[cfg(any())]
#[path = "AVCaptureControl.rs"]
mod __AVCaptureControl;
#[cfg(any())]
#[path = "AVCaptureDataOutputSynchronizer.rs"]
mod __AVCaptureDataOutputSynchronizer;
#[cfg(any())]
#[path = "AVCaptureDepthDataOutput.rs"]
mod __AVCaptureDepthDataOutput;
#[cfg(any())]
#[path = "AVCaptureDeskViewApplication.rs"]
mod __AVCaptureDeskViewApplication;
#[cfg(any())]
#[path = "AVCaptureDevice.rs"]
mod __AVCaptureDevice;
#[cfg(any())]
#[path = "AVCaptureExternalDisplayConfigurator.rs"]
mod __AVCaptureExternalDisplayConfigurator;
#[cfg(any())]
#[path = "AVCaptureFileOutput.rs"]
mod __AVCaptureFileOutput;
#[cfg(any())]
#[path = "AVCaptureIndexPicker.rs"]
mod __AVCaptureIndexPicker;
#[cfg(any())]
#[path = "AVCaptureInput.rs"]
mod __AVCaptureInput;
#[cfg(any())]
#[path = "AVCaptureMetadataOutput.rs"]
mod __AVCaptureMetadataOutput;
#[cfg(any())]
#[path = "AVCaptureOutput.rs"]
mod __AVCaptureOutput;
#[cfg(any())]
#[path = "AVCaptureOutputBase.rs"]
mod __AVCaptureOutputBase;
#[cfg(any())]
#[path = "AVCapturePhotoOutput.rs"]
mod __AVCapturePhotoOutput;
#[cfg(any())]
#[path = "AVCaptureReactions.rs"]
mod __AVCaptureReactions;
#[cfg(any())]
#[path = "AVCaptureSession.rs"]
mod __AVCaptureSession;
#[cfg(any())]
#[path = "AVCaptureSessionPreset.rs"]
mod __AVCaptureSessionPreset;
#[cfg(any())]
#[path = "AVCaptureSlider.rs"]
mod __AVCaptureSlider;
#[cfg(any())]
#[path = "AVCaptureSpatialAudioMetadataSampleGenerator.rs"]
mod __AVCaptureSpatialAudioMetadataSampleGenerator;
#[cfg(any())]
#[path = "AVCaptureStillImageOutput.rs"]
mod __AVCaptureStillImageOutput;
#[cfg(any())]
#[path = "AVCaptureSystemExposureBiasSlider.rs"]
mod __AVCaptureSystemExposureBiasSlider;
#[cfg(any())]
#[path = "AVCaptureSystemPressure.rs"]
mod __AVCaptureSystemPressure;
#[cfg(any())]
#[path = "AVCaptureSystemZoomSlider.rs"]
mod __AVCaptureSystemZoomSlider;
#[cfg(any())]
#[path = "AVCaptureTimecodeGenerator.rs"]
mod __AVCaptureTimecodeGenerator;
#[cfg(any())]
#[path = "AVCaptureVideoDataOutput.rs"]
mod __AVCaptureVideoDataOutput;
#[cfg(any())]
#[path = "AVCaptureVideoPreviewLayer.rs"]
mod __AVCaptureVideoPreviewLayer;
#[cfg(any())]
#[path = "AVComposition.rs"]
mod __AVComposition;
#[cfg(any())]
#[path = "AVCompositionTrack.rs"]
mod __AVCompositionTrack;
#[cfg(any())]
#[path = "AVCompositionTrackSegment.rs"]
mod __AVCompositionTrackSegment;
#[cfg(any())]
#[path = "AVContentKeySession.rs"]
mod __AVContentKeySession;
#[cfg(any())]
#[path = "AVContinuityDevice.rs"]
mod __AVContinuityDevice;
#[cfg(any())]
#[path = "AVDepthData.rs"]
mod __AVDepthData;
#[cfg(any())]
#[path = "AVError.rs"]
mod __AVError;
#[cfg(any())]
#[path = "AVExternalStorageDevice.rs"]
mod __AVExternalStorageDevice;
#[cfg(any())]
#[path = "AVExternalSyncDevice.rs"]
mod __AVExternalSyncDevice;
#[cfg(any())]
#[path = "AVFAudio.rs"]
mod __AVFAudio;
#[cfg(any())]
#[path = "AVFCapture.rs"]
mod __AVFCapture;
#[cfg(any())]
#[path = "AVFCore.rs"]
mod __AVFCore;
#[cfg(any())]
#[path = "AVGeometry.rs"]
mod __AVGeometry;
#[cfg(any())]
#[path = "AVMIDIPlayer.rs"]
mod __AVMIDIPlayer;
#[cfg(feature = "AVMediaFormat")]
#[path = "AVMediaFormat.rs"]
mod __AVMediaFormat;
#[cfg(any())]
#[path = "AVMediaSelection.rs"]
mod __AVMediaSelection;
#[cfg(any())]
#[path = "AVMediaSelectionGroup.rs"]
mod __AVMediaSelectionGroup;
#[cfg(any())]
#[path = "AVMetadataFormat.rs"]
mod __AVMetadataFormat;
#[cfg(any())]
#[path = "AVMetadataIdentifiers.rs"]
mod __AVMetadataIdentifiers;
#[cfg(any())]
#[path = "AVMetadataItem.rs"]
mod __AVMetadataItem;
#[cfg(any())]
#[path = "AVMetadataObject.rs"]
mod __AVMetadataObject;
#[cfg(any())]
#[path = "AVMetrics.rs"]
mod __AVMetrics;
#[cfg(any())]
#[path = "AVMovie.rs"]
mod __AVMovie;
#[cfg(any())]
#[path = "AVMovieTrack.rs"]
mod __AVMovieTrack;
#[cfg(any())]
#[path = "AVOutputSettingsAssistant.rs"]
mod __AVOutputSettingsAssistant;
#[cfg(any())]
#[path = "AVPlaybackCoordinationMedium.rs"]
mod __AVPlaybackCoordinationMedium;
#[cfg(any())]
#[path = "AVPlaybackCoordinator.rs"]
mod __AVPlaybackCoordinator;
#[cfg(any())]
#[path = "AVPlayer.rs"]
mod __AVPlayer;
#[cfg(any())]
#[path = "AVPlayerInterstitialEventController.rs"]
mod __AVPlayerInterstitialEventController;
#[cfg(any())]
#[path = "AVPlayerItem.rs"]
mod __AVPlayerItem;
#[cfg(any())]
#[path = "AVPlayerItemIntegratedTimeline.rs"]
mod __AVPlayerItemIntegratedTimeline;
#[cfg(any())]
#[path = "AVPlayerItemMediaDataCollector.rs"]
mod __AVPlayerItemMediaDataCollector;
#[cfg(any())]
#[path = "AVPlayerItemOutput.rs"]
mod __AVPlayerItemOutput;
#[cfg(any())]
#[path = "AVPlayerItemProtectedContentAdditions.rs"]
mod __AVPlayerItemProtectedContentAdditions;
#[cfg(any())]
#[path = "AVPlayerItemTrack.rs"]
mod __AVPlayerItemTrack;
#[cfg(any())]
#[path = "AVPlayerLayer.rs"]
mod __AVPlayerLayer;
#[cfg(any())]
#[path = "AVPlayerLooper.rs"]
mod __AVPlayerLooper;
#[cfg(any())]
#[path = "AVPlayerMediaSelectionCriteria.rs"]
mod __AVPlayerMediaSelectionCriteria;
#[cfg(any())]
#[path = "AVPlayerOutput.rs"]
mod __AVPlayerOutput;
#[cfg(any())]
#[path = "AVPortraitEffectsMatte.rs"]
mod __AVPortraitEffectsMatte;
#[cfg(any())]
#[path = "AVQueuedSampleBufferRendering.rs"]
mod __AVQueuedSampleBufferRendering;
#[cfg(any())]
#[path = "AVRenderedCaptionImage.rs"]
mod __AVRenderedCaptionImage;
#[cfg(any())]
#[path = "AVRouteDetector.rs"]
mod __AVRouteDetector;
#[cfg(any())]
#[path = "AVSampleBufferAudioRenderer.rs"]
mod __AVSampleBufferAudioRenderer;
#[cfg(any())]
#[path = "AVSampleBufferDisplayLayer.rs"]
mod __AVSampleBufferDisplayLayer;
#[cfg(any())]
#[path = "AVSampleBufferGenerator.rs"]
mod __AVSampleBufferGenerator;
#[cfg(any())]
#[path = "AVSampleBufferRenderSynchronizer.rs"]
mod __AVSampleBufferRenderSynchronizer;
#[cfg(any())]
#[path = "AVSampleBufferVideoRenderer.rs"]
mod __AVSampleBufferVideoRenderer;
#[cfg(any())]
#[path = "AVSampleCursor.rs"]
mod __AVSampleCursor;
#[cfg(any())]
#[path = "AVSemanticSegmentationMatte.rs"]
mod __AVSemanticSegmentationMatte;
#[cfg(any())]
#[path = "AVSpatialVideoConfiguration.rs"]
mod __AVSpatialVideoConfiguration;
#[cfg(any())]
#[path = "AVSynchronizedLayer.rs"]
mod __AVSynchronizedLayer;
#[cfg(any())]
#[path = "AVTextStyleRule.rs"]
mod __AVTextStyleRule;
#[cfg(any())]
#[path = "AVTime.rs"]
mod __AVTime;
#[cfg(any())]
#[path = "AVTimedMetadataGroup.rs"]
mod __AVTimedMetadataGroup;
#[cfg(any())]
#[path = "AVUtilities.rs"]
mod __AVUtilities;
#[cfg(any())]
#[path = "AVVideoCompositing.rs"]
mod __AVVideoCompositing;
#[cfg(any())]
#[path = "AVVideoComposition.rs"]
mod __AVVideoComposition;
#[cfg(any())]
#[path = "AVVideoPerformanceMetrics.rs"]
mod __AVVideoPerformanceMetrics;
#[cfg(feature = "AVVideoSettings")]
#[path = "AVVideoSettings.rs"]
mod __AVVideoSettings;
#[cfg(all(any(), feature = "objc2-core-foundation"))]
pub use self::__AVAnimation::AVCoreAnimationBeginTimeAtZero;
#[cfg(any())]
pub use self::__AVAnimation::AVLayerVideoGravity;
#[cfg(any())]
pub use self::__AVAnimation::AVLayerVideoGravityResize;
#[cfg(any())]
pub use self::__AVAnimation::AVLayerVideoGravityResizeAspect;
#[cfg(any())]
pub use self::__AVAnimation::AVLayerVideoGravityResizeAspectFill;
#[cfg(feature = "AVAsset")]
pub use self::__AVAsset::AVAsset;
#[cfg(feature = "AVAsset")]
pub use self::__AVAsset::AVAssetReferenceRestrictions;
#[cfg(feature = "AVAsset")]
pub use self::__AVAsset::AVFragmentMinding;
#[cfg(feature = "AVAsset")]
pub use self::__AVAsset::AVFragmentedAsset;
#[cfg(feature = "AVAsset")]
pub use self::__AVAsset::AVFragmentedAssetMinder;
#[cfg(feature = "AVAsset")]
pub use self::__AVAsset::AVMediaExtensionProperties;
#[cfg(feature = "AVAsset")]
pub use self::__AVAsset::AVURLAsset;
#[cfg(any())]
pub use self::__AVAssetCache::AVAssetCache;
#[cfg(any())]
pub use self::__AVAssetDownloadStorageManager::AVAssetDownloadStorageManagementPolicy;
#[cfg(any())]
pub use self::__AVAssetDownloadStorageManager::AVAssetDownloadStorageManager;
#[cfg(any())]
pub use self::__AVAssetDownloadStorageManager::AVAssetDownloadedAssetEvictionPriority;
#[cfg(any())]
pub use self::__AVAssetDownloadStorageManager::AVAssetDownloadedAssetEvictionPriorityDefault;
#[cfg(any())]
pub use self::__AVAssetDownloadStorageManager::AVAssetDownloadedAssetEvictionPriorityImportant;
#[cfg(any())]
pub use self::__AVAssetDownloadStorageManager::AVMutableAssetDownloadStorageManagementPolicy;
#[cfg(any())]
pub use self::__AVAssetDownloadTask::AVAggregateAssetDownloadTask;
#[cfg(any())]
pub use self::__AVAssetDownloadTask::AVAssetDownloadConfiguration;
#[cfg(any())]
pub use self::__AVAssetDownloadTask::AVAssetDownloadContentConfiguration;
#[cfg(any())]
pub use self::__AVAssetDownloadTask::AVAssetDownloadDelegate;
#[cfg(any())]
pub use self::__AVAssetDownloadTask::AVAssetDownloadTask;
#[cfg(any())]
pub use self::__AVAssetDownloadTask::AVAssetDownloadTaskMediaSelectionKey;
#[cfg(any())]
pub use self::__AVAssetDownloadTask::AVAssetDownloadTaskMediaSelectionPrefersMultichannelKey;
#[cfg(any())]
pub use self::__AVAssetDownloadTask::AVAssetDownloadTaskMinimumRequiredMediaBitrateKey;
#[cfg(any())]
pub use self::__AVAssetDownloadTask::AVAssetDownloadTaskMinimumRequiredPresentationSizeKey;
#[cfg(any())]
pub use self::__AVAssetDownloadTask::AVAssetDownloadTaskPrefersHDRKey;
#[cfg(any())]
pub use self::__AVAssetDownloadTask::AVAssetDownloadTaskPrefersLosslessAudioKey;
#[cfg(any())]
pub use self::__AVAssetDownloadTask::AVAssetDownloadURLSession;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPreset1280x720;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPreset1920x1080;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPreset3840x2160;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPreset640x480;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPreset960x540;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetAppleM4A;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetAppleM4V1080pHD;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetAppleM4V480pSD;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetAppleM4V720pHD;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetAppleM4VAppleTV;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetAppleM4VCellular;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetAppleM4VWiFi;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetAppleM4ViPod;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetAppleProRes422LPCM;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetAppleProRes4444LPCM;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetHEVC1920x1080;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetHEVC1920x1080WithAlpha;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetHEVC3840x2160;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetHEVC3840x2160WithAlpha;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetHEVC4320x2160;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetHEVC7680x4320;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetHEVCHighestQuality;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetHEVCHighestQualityWithAlpha;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetHighestQuality;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetLowQuality;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetMVHEVC1440x1440;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetMVHEVC4320x4320;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetMVHEVC7680x7680;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetMVHEVC960x960;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetMediumQuality;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportPresetPassthrough;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportSession;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetExportSessionStatus;
#[cfg(any())]
pub use self::__AVAssetExportSession::AVAssetTrackGroupOutputHandling;
#[cfg(any())]
pub use self::__AVAssetImageGenerator::AVAssetImageGenerator;
#[cfg(any())]
pub use self::__AVAssetImageGenerator::AVAssetImageGeneratorApertureMode;
#[cfg(any())]
pub use self::__AVAssetImageGenerator::AVAssetImageGeneratorApertureModeCleanAperture;
#[cfg(any())]
pub use self::__AVAssetImageGenerator::AVAssetImageGeneratorApertureModeEncodedPixels;
#[cfg(any())]
pub use self::__AVAssetImageGenerator::AVAssetImageGeneratorApertureModeProductionAperture;
#[cfg(all(any(), feature = "block2", any(), feature = "objc2-core-media"))]
pub use self::__AVAssetImageGenerator::AVAssetImageGeneratorCompletionHandler;
#[cfg(any())]
pub use self::__AVAssetImageGenerator::AVAssetImageGeneratorDynamicRangePolicy;
#[cfg(any())]
pub use self::__AVAssetImageGenerator::AVAssetImageGeneratorDynamicRangePolicyForceSDR;
#[cfg(any())]
pub use self::__AVAssetImageGenerator::AVAssetImageGeneratorDynamicRangePolicyMatchSource;
#[cfg(any())]
pub use self::__AVAssetImageGenerator::AVAssetImageGeneratorResult;
#[cfg(any())]
pub use self::__AVAssetPlaybackAssistant::AVAssetPlaybackAssistant;
#[cfg(any())]
pub use self::__AVAssetPlaybackAssistant::AVAssetPlaybackConfigurationOption;
#[cfg(any())]
pub use self::__AVAssetPlaybackAssistant::AVAssetPlaybackConfigurationOptionAppleImmersiveVideo;
#[cfg(any())]
pub use self::__AVAssetPlaybackAssistant::AVAssetPlaybackConfigurationOptionNonRectilinearProjection;
#[cfg(any())]
pub use self::__AVAssetPlaybackAssistant::AVAssetPlaybackConfigurationOptionSpatialVideo;
#[cfg(any())]
pub use self::__AVAssetPlaybackAssistant::AVAssetPlaybackConfigurationOptionStereoMultiviewVideo;
#[cfg(any())]
pub use self::__AVAssetPlaybackAssistant::AVAssetPlaybackConfigurationOptionStereoVideo;
#[cfg(feature = "AVAssetReader")]
pub use self::__AVAssetReader::AVAssetReader;
#[cfg(feature = "AVAssetReader")]
pub use self::__AVAssetReader::AVAssetReaderStatus;
#[cfg(feature = "AVAssetReaderOutput")]
pub use self::__AVAssetReaderOutput::AVAssetReaderAudioMixOutput;
#[cfg(feature = "AVAssetReaderOutput")]
pub use self::__AVAssetReaderOutput::AVAssetReaderCaptionValidationHandling;
#[cfg(feature = "AVAssetReaderOutput")]
pub use self::__AVAssetReaderOutput::AVAssetReaderOutput;
#[cfg(feature = "AVAssetReaderOutput")]
pub use self::__AVAssetReaderOutput::AVAssetReaderOutputCaptionAdaptor;
#[cfg(feature = "AVAssetReaderOutput")]
pub use self::__AVAssetReaderOutput::AVAssetReaderOutputMetadataAdaptor;
#[cfg(feature = "AVAssetReaderOutput")]
pub use self::__AVAssetReaderOutput::AVAssetReaderSampleReferenceOutput;
#[cfg(feature = "AVAssetReaderOutput")]
pub use self::__AVAssetReaderOutput::AVAssetReaderTrackOutput;
#[cfg(feature = "AVAssetReaderOutput")]
pub use self::__AVAssetReaderOutput::AVAssetReaderVideoCompositionOutput;
#[cfg(any())]
pub use self::__AVAssetResourceLoader::AVAssetResourceLoader;
#[cfg(any())]
pub use self::__AVAssetResourceLoader::AVAssetResourceLoaderDelegate;
#[cfg(any())]
pub use self::__AVAssetResourceLoader::AVAssetResourceLoadingContentInformationRequest;
#[cfg(any())]
pub use self::__AVAssetResourceLoader::AVAssetResourceLoadingDataRequest;
#[cfg(any())]
pub use self::__AVAssetResourceLoader::AVAssetResourceLoadingRequest;
#[cfg(any())]
pub use self::__AVAssetResourceLoader::AVAssetResourceLoadingRequestStreamingContentKeyRequestRequiresPersistentKey;
#[cfg(any())]
pub use self::__AVAssetResourceLoader::AVAssetResourceLoadingRequestor;
#[cfg(any())]
pub use self::__AVAssetResourceLoader::AVAssetResourceRenewalRequest;
#[cfg(any())]
pub use self::__AVAssetSegmentReport::AVAssetSegmentReport;
#[cfg(any())]
pub use self::__AVAssetSegmentReport::AVAssetSegmentReportSampleInformation;
#[cfg(any())]
pub use self::__AVAssetSegmentReport::AVAssetSegmentTrackReport;
#[cfg(any())]
pub use self::__AVAssetSegmentReport::AVAssetSegmentType;
#[cfg(feature = "AVAssetTrack")]
pub use self::__AVAssetTrack::AVAssetTrack;
#[cfg(feature = "AVAssetTrack")]
pub use self::__AVAssetTrack::AVFragmentedAssetTrack;
#[cfg(feature = "AVAssetTrack")]
pub use self::__AVAssetTrack::AVTrackAssociationType;
#[cfg(any())]
pub use self::__AVAssetTrackGroup::AVAssetTrackGroup;
#[cfg(any())]
pub use self::__AVAssetTrackSegment::AVAssetTrackSegment;
#[cfg(any())]
pub use self::__AVAssetVariant::AVAssetVariant;
#[cfg(any())]
pub use self::__AVAssetVariant::AVAssetVariantAudioAttributes;
#[cfg(any())]
pub use self::__AVAssetVariant::AVAssetVariantAudioRenditionSpecificAttributes;
#[cfg(any())]
pub use self::__AVAssetVariant::AVAssetVariantQualifier;
#[cfg(any())]
pub use self::__AVAssetVariant::AVAssetVariantVideoAttributes;
#[cfg(any())]
pub use self::__AVAssetVariant::AVAssetVariantVideoLayoutAttributes;
#[cfg(feature = "AVAssetWriter")]
pub use self::__AVAssetWriter::AVAssetWriter;
#[cfg(feature = "AVAssetWriter")]
pub use self::__AVAssetWriter::AVAssetWriterDelegate;
#[cfg(all(feature = "AVAssetWriter", any()))]
pub use self::__AVAssetWriter::AVAssetWriterInputGroup;
#[cfg(feature = "AVAssetWriter")]
pub use self::__AVAssetWriter::AVAssetWriterStatus;
#[cfg(feature = "AVAssetWriterInput")]
pub use self::__AVAssetWriterInput::AVAssetWriterInput;
#[cfg(feature = "AVAssetWriterInput")]
pub use self::__AVAssetWriterInput::AVAssetWriterInputCaptionAdaptor;
#[cfg(feature = "AVAssetWriterInput")]
pub use self::__AVAssetWriterInput::AVAssetWriterInputMediaDataLocation;
#[cfg(feature = "AVAssetWriterInput")]
pub use self::__AVAssetWriterInput::AVAssetWriterInputMetadataAdaptor;
#[cfg(feature = "AVAssetWriterInput")]
pub use self::__AVAssetWriterInput::AVAssetWriterInputPassDescription;
#[cfg(feature = "AVAssetWriterInput")]
pub use self::__AVAssetWriterInput::AVAssetWriterInputPixelBufferAdaptor;
#[cfg(feature = "AVAssetWriterInput")]
pub use self::__AVAssetWriterInput::AVAssetWriterInputTaggedPixelBufferGroupAdaptor;
#[cfg(any())]
pub use self::__AVAsynchronousKeyValueLoading::AVAsynchronousKeyValueLoading;
#[cfg(any())]
pub use self::__AVAsynchronousKeyValueLoading::AVKeyValueStatus;
#[cfg(any())]
pub use self::__AVAudioMix::AVAudioMix;
#[cfg(any())]
pub use self::__AVAudioMix::AVAudioMixInputParameters;
#[cfg(any())]
pub use self::__AVAudioMix::AVMutableAudioMix;
#[cfg(any())]
pub use self::__AVAudioMix::AVMutableAudioMixInputParameters;
#[cfg(any())]
pub use self::__AVAudioProcessingSettings::AVAudioSpatializationFormats;
#[cfg(any())]
pub use self::__AVAudioProcessingSettings::AVAudioTimePitchAlgorithm;
#[cfg(any())]
pub use self::__AVAudioProcessingSettings::AVAudioTimePitchAlgorithmLowQualityZeroLatency;
#[cfg(any())]
pub use self::__AVAudioProcessingSettings::AVAudioTimePitchAlgorithmSpectral;
#[cfg(any())]
pub use self::__AVAudioProcessingSettings::AVAudioTimePitchAlgorithmTimeDomain;
#[cfg(any())]
pub use self::__AVAudioProcessingSettings::AVAudioTimePitchAlgorithmVarispeed;
#[cfg(any())]
pub use self::__AVCameraCalibrationData::AVCameraCalibrationData;
#[cfg(any())]
pub use self::__AVCaption::AVCaption;
#[cfg(any())]
pub use self::__AVCaption::AVCaptionAnimation;
#[cfg(any())]
pub use self::__AVCaption::AVCaptionDecoration;
#[cfg(all(any(), feature = "objc2-core-foundation"))]
pub use self::__AVCaption::AVCaptionDimension;
#[cfg(all(any(), feature = "objc2-core-foundation"))]
pub use self::__AVCaption::AVCaptionDimensionMake;
#[cfg(any())]
pub use self::__AVCaption::AVCaptionFontStyle;
#[cfg(any())]
pub use self::__AVCaption::AVCaptionFontWeight;
#[cfg(all(any(), feature = "objc2-core-foundation"))]
pub use self::__AVCaption::AVCaptionPoint;
#[cfg(all(any(), feature = "objc2-core-foundation"))]
pub use self::__AVCaption::AVCaptionPointMake;
#[cfg(any())]
pub use self::__AVCaption::AVCaptionRegion;
#[cfg(any())]
pub use self::__AVCaption::AVCaptionRegionDisplayAlignment;
#[cfg(any())]
pub use self::__AVCaption::AVCaptionRegionScroll;
#[cfg(any())]
pub use self::__AVCaption::AVCaptionRegionWritingMode;
#[cfg(any())]
pub use self::__AVCaption::AVCaptionRuby;
#[cfg(any())]
pub use self::__AVCaption::AVCaptionRubyAlignment;
#[cfg(any())]
pub use self::__AVCaption::AVCaptionRubyPosition;
#[cfg(all(any(), feature = "objc2-core-foundation"))]
pub use self::__AVCaption::AVCaptionSize;
#[cfg(all(any(), feature = "objc2-core-foundation"))]
pub use self::__AVCaption::AVCaptionSizeMake;
#[cfg(any())]
pub use self::__AVCaption::AVCaptionTextAlignment;
#[cfg(any())]
pub use self::__AVCaption::AVCaptionTextCombine;
#[cfg(any())]
pub use self::__AVCaption::AVCaptionUnitsType;
#[cfg(any())]
pub use self::__AVCaption::AVMutableCaption;
#[cfg(any())]
pub use self::__AVCaption::AVMutableCaptionRegion;
#[cfg(any())]
pub use self::__AVCaptionConversionValidator::AVCaptionConversionAdjustment;
#[cfg(any())]
pub use self::__AVCaptionConversionValidator::AVCaptionConversionAdjustmentType;
#[cfg(any())]
pub use self::__AVCaptionConversionValidator::AVCaptionConversionAdjustmentTypeTimeRange;
#[cfg(any())]
pub use self::__AVCaptionConversionValidator::AVCaptionConversionTimeRangeAdjustment;
#[cfg(any())]
pub use self::__AVCaptionConversionValidator::AVCaptionConversionValidator;
#[cfg(any())]
pub use self::__AVCaptionConversionValidator::AVCaptionConversionValidatorStatus;
#[cfg(any())]
pub use self::__AVCaptionConversionValidator::AVCaptionConversionWarning;
#[cfg(any())]
pub use self::__AVCaptionConversionValidator::AVCaptionConversionWarningType;
#[cfg(any())]
pub use self::__AVCaptionConversionValidator::AVCaptionConversionWarningTypeExcessMediaData;
#[cfg(any())]
pub use self::__AVCaptionFormatConformer::AVCaptionFormatConformer;
#[cfg(any())]
pub use self::__AVCaptionGroup::AVCaptionGroup;
#[cfg(any())]
pub use self::__AVCaptionGrouper::AVCaptionGrouper;
#[cfg(any())]
pub use self::__AVCaptionRenderer::AVCaptionRenderer;
#[cfg(any())]
pub use self::__AVCaptionRenderer::AVCaptionRendererScene;
#[cfg(any())]
pub use self::__AVCaptionSettings::AVCaptionMediaSubTypeKey;
#[cfg(any())]
pub use self::__AVCaptionSettings::AVCaptionMediaTypeKey;
#[cfg(any())]
pub use self::__AVCaptionSettings::AVCaptionSettingsKey;
#[cfg(any())]
pub use self::__AVCaptionSettings::AVCaptionTimeCodeFrameDurationKey;
#[cfg(any())]
pub use self::__AVCaptionSettings::AVCaptionUseDropFrameTimeCodeKey;
#[cfg(all(any(), any()))]
pub use self::__AVCaptureAudioDataOutput::AVCaptureAudioDataOutput;
#[cfg(any())]
pub use self::__AVCaptureAudioDataOutput::AVCaptureAudioDataOutputSampleBufferDelegate;
#[cfg(all(any(), any()))]
pub use self::__AVCaptureAudioPreviewOutput::AVCaptureAudioPreviewOutput;
#[cfg(any())]
pub use self::__AVCaptureControl::AVCaptureControl;
#[cfg(any())]
pub use self::__AVCaptureDataOutputSynchronizer::AVCaptureDataOutputSynchronizer;
#[cfg(any())]
pub use self::__AVCaptureDataOutputSynchronizer::AVCaptureDataOutputSynchronizerDelegate;
#[cfg(any())]
pub use self::__AVCaptureDataOutputSynchronizer::AVCaptureSynchronizedData;
#[cfg(any())]
pub use self::__AVCaptureDataOutputSynchronizer::AVCaptureSynchronizedDataCollection;
#[cfg(any())]
pub use self::__AVCaptureDataOutputSynchronizer::AVCaptureSynchronizedDepthData;
#[cfg(any())]
pub use self::__AVCaptureDataOutputSynchronizer::AVCaptureSynchronizedMetadataObjectData;
#[cfg(any())]
pub use self::__AVCaptureDataOutputSynchronizer::AVCaptureSynchronizedSampleBufferData;
#[cfg(all(any(), any()))]
pub use self::__AVCaptureDepthDataOutput::AVCaptureDepthDataOutput;
#[cfg(any())]
pub use self::__AVCaptureDepthDataOutput::AVCaptureDepthDataOutputDelegate;
#[cfg(any())]
pub use self::__AVCaptureDeskViewApplication::AVCaptureDeskViewApplication;
#[cfg(any())]
pub use self::__AVCaptureDeskViewApplication::AVCaptureDeskViewApplicationLaunchConfiguration;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVAuthorizationStatus;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureAspectRatio;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureAspectRatio16x9;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureAspectRatio1x1;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureAspectRatio3x4;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureAspectRatio4x3;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureAspectRatio9x16;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureAutoFocusRangeRestriction;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureAutoFocusSystem;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureCameraLensSmudgeDetectionStatus;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureCenterStageControlMode;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureCinematicVideoFocusMode;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureColorSpace;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDevice;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceDiscoverySession;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceFormat;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceInputSource;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDevicePosition;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceRotationCoordinator;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceSubjectAreaDidChangeNotification;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTransportControlsPlaybackMode;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTransportControlsSpeed;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceType;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeBuiltInDualCamera;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeBuiltInDualWideCamera;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeBuiltInDuoCamera;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeBuiltInLiDARDepthCamera;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeBuiltInMicrophone;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeBuiltInTelephotoCamera;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeBuiltInTripleCamera;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeBuiltInTrueDepthCamera;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeBuiltInUltraWideCamera;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeBuiltInWideAngleCamera;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeContinuityCamera;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeDeskViewCamera;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeExternal;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeExternalUnknown;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceTypeMicrophone;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceWasConnectedNotification;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureDeviceWasDisconnectedNotification;
#[cfg(all(any(), feature = "objc2-core-media"))]
pub use self::__AVCaptureDevice::AVCaptureExposureDurationCurrent;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureExposureMode;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureExposureTargetBiasCurrent;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureFlashMode;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureFocusMode;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureFraming;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureISOCurrent;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureLensPositionCurrent;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureMaxAvailableTorchLevel;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureMicrophoneMode;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCapturePrimaryConstituentDeviceRestrictedSwitchingBehaviorConditions;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCapturePrimaryConstituentDeviceSwitchingBehavior;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureSceneMonitoringStatus;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureSceneMonitoringStatusNotEnoughLight;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureSmartFramingMonitor;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureSystemUserInterface;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureTorchMode;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureVideoStabilizationMode;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureWhiteBalanceChromaticityValues;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureWhiteBalanceGains;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureWhiteBalanceGainsCurrent;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureWhiteBalanceMode;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureWhiteBalanceTemperatureAndTintValues;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureWhiteBalanceTemperatureAndTintValuesCloudy;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureWhiteBalanceTemperatureAndTintValuesDaylight;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureWhiteBalanceTemperatureAndTintValuesFluorescent;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureWhiteBalanceTemperatureAndTintValuesShadow;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVCaptureWhiteBalanceTemperatureAndTintValuesTungsten;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVExposureBiasRange;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVFrameRateRange;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVSpatialCaptureDiscomfortReason;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVSpatialCaptureDiscomfortReasonNotEnoughLight;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVSpatialCaptureDiscomfortReasonSubjectTooClose;
#[cfg(any())]
pub use self::__AVCaptureDevice::AVZoomRange;
#[cfg(any())]
pub use self::__AVCaptureExternalDisplayConfigurator::AVCaptureExternalDisplayConfiguration;
#[cfg(any())]
pub use self::__AVCaptureExternalDisplayConfigurator::AVCaptureExternalDisplayConfigurator;
#[cfg(all(any(), any()))]
pub use self::__AVCaptureFileOutput::AVCaptureAudioFileOutput;
#[cfg(all(any(), any()))]
pub use self::__AVCaptureFileOutput::AVCaptureFileOutput;
#[cfg(any())]
pub use self::__AVCaptureFileOutput::AVCaptureFileOutputDelegate;
#[cfg(any())]
pub use self::__AVCaptureFileOutput::AVCaptureFileOutputRecordingDelegate;
#[cfg(all(any(), any()))]
pub use self::__AVCaptureFileOutput::AVCaptureMovieFileOutput;
#[cfg(all(any(), any()))]
pub use self::__AVCaptureIndexPicker::AVCaptureIndexPicker;
#[cfg(any())]
pub use self::__AVCaptureInput::AVCaptureDeviceInput;
#[cfg(any())]
pub use self::__AVCaptureInput::AVCaptureInput;
#[cfg(any())]
pub use self::__AVCaptureInput::AVCaptureInputPort;
#[cfg(any())]
pub use self::__AVCaptureInput::AVCaptureInputPortFormatDescriptionDidChangeNotification;
#[cfg(any())]
pub use self::__AVCaptureInput::AVCaptureMetadataInput;
#[cfg(any())]
pub use self::__AVCaptureInput::AVCaptureMultichannelAudioMode;
#[cfg(any())]
pub use self::__AVCaptureInput::AVCaptureScreenInput;
#[cfg(all(any(), any()))]
pub use self::__AVCaptureMetadataOutput::AVCaptureMetadataOutput;
#[cfg(any())]
pub use self::__AVCaptureMetadataOutput::AVCaptureMetadataOutputObjectsDelegate;
#[cfg(any())]
pub use self::__AVCaptureOutputBase::AVCaptureOutput;
#[cfg(any())]
pub use self::__AVCaptureOutputBase::AVCaptureOutputDataDroppedReason;
#[cfg(any())]
pub use self::__AVCapturePhotoOutput::AVCaptureDeferredPhotoProxy;
#[cfg(any())]
pub use self::__AVCapturePhotoOutput::AVCaptureLensStabilizationStatus;
#[cfg(any())]
pub use self::__AVCapturePhotoOutput::AVCapturePhoto;
#[cfg(any())]
pub use self::__AVCapturePhotoOutput::AVCapturePhotoBracketSettings;
#[cfg(any())]
pub use self::__AVCapturePhotoOutput::AVCapturePhotoCaptureDelegate;
#[cfg(any())]
pub use self::__AVCapturePhotoOutput::AVCapturePhotoFileDataRepresentationCustomizer;
#[cfg(all(any(), any()))]
pub use self::__AVCapturePhotoOutput::AVCapturePhotoOutput;
#[cfg(any())]
pub use self::__AVCapturePhotoOutput::AVCapturePhotoOutputCaptureReadiness;
#[cfg(any())]
pub use self::__AVCapturePhotoOutput::AVCapturePhotoOutputReadinessCoordinator;
#[cfg(any())]
pub use self::__AVCapturePhotoOutput::AVCapturePhotoOutputReadinessCoordinatorDelegate;
#[cfg(any())]
pub use self::__AVCapturePhotoOutput::AVCapturePhotoQualityPrioritization;
#[cfg(any())]
pub use self::__AVCapturePhotoOutput::AVCapturePhotoSettings;
#[cfg(any())]
pub use self::__AVCapturePhotoOutput::AVCaptureResolvedPhotoSettings;
#[cfg(any())]
pub use self::__AVCaptureReactions::AVCaptureReactionEffectState;
#[cfg(any())]
pub use self::__AVCaptureReactions::AVCaptureReactionSystemImageNameForType;
#[cfg(any())]
pub use self::__AVCaptureReactions::AVCaptureReactionType;
#[cfg(any())]
pub use self::__AVCaptureReactions::AVCaptureReactionTypeBalloons;
#[cfg(any())]
pub use self::__AVCaptureReactions::AVCaptureReactionTypeConfetti;
#[cfg(any())]
pub use self::__AVCaptureReactions::AVCaptureReactionTypeFireworks;
#[cfg(any())]
pub use self::__AVCaptureReactions::AVCaptureReactionTypeHeart;
#[cfg(any())]
pub use self::__AVCaptureReactions::AVCaptureReactionTypeLasers;
#[cfg(any())]
pub use self::__AVCaptureReactions::AVCaptureReactionTypeRain;
#[cfg(any())]
pub use self::__AVCaptureReactions::AVCaptureReactionTypeThumbsDown;
#[cfg(any())]
pub use self::__AVCaptureReactions::AVCaptureReactionTypeThumbsUp;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureAudioChannel;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureConnection;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureMultiCamSession;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureSession;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureSessionControlsDelegate;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureSessionDeferredStartDelegate;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureSessionDidStartRunningNotification;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureSessionDidStopRunningNotification;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureSessionErrorKey;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureSessionInterruptionEndedNotification;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureSessionInterruptionReason;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureSessionInterruptionReasonKey;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureSessionInterruptionSystemPressureStateKey;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureSessionRuntimeErrorNotification;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureSessionWasInterruptedNotification;
#[cfg(any())]
pub use self::__AVCaptureSession::AVCaptureVideoOrientation;
#[cfg(any())]
pub use self::__AVCaptureSession::AVVideoFieldMode;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPreset;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPreset1280x720;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPreset1920x1080;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPreset320x240;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPreset352x288;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPreset3840x2160;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPreset640x480;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPreset960x540;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPresetHigh;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPresetInputPriority;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPresetLow;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPresetMedium;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPresetPhoto;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPresetiFrame1280x720;
#[cfg(any())]
pub use self::__AVCaptureSessionPreset::AVCaptureSessionPresetiFrame960x540;
#[cfg(all(any(), any()))]
pub use self::__AVCaptureSlider::AVCaptureSlider;
#[cfg(any())]
pub use self::__AVCaptureSpatialAudioMetadataSampleGenerator::AVCaptureSpatialAudioMetadataSampleGenerator;
#[cfg(any())]
pub use self::__AVCaptureStillImageOutput::AVCaptureAutoExposureBracketedStillImageSettings;
#[cfg(any())]
pub use self::__AVCaptureStillImageOutput::AVCaptureBracketedStillImageSettings;
#[cfg(any())]
pub use self::__AVCaptureStillImageOutput::AVCaptureManualExposureBracketedStillImageSettings;
#[cfg(all(any(), any()))]
pub use self::__AVCaptureStillImageOutput::AVCaptureStillImageOutput;
#[cfg(all(any(), any()))]
pub use self::__AVCaptureSystemExposureBiasSlider::AVCaptureSystemExposureBiasSlider;
#[cfg(any())]
pub use self::__AVCaptureSystemPressure::AVCaptureSystemPressureFactors;
#[cfg(any())]
pub use self::__AVCaptureSystemPressure::AVCaptureSystemPressureLevel;
#[cfg(any())]
pub use self::__AVCaptureSystemPressure::AVCaptureSystemPressureLevelCritical;
#[cfg(any())]
pub use self::__AVCaptureSystemPressure::AVCaptureSystemPressureLevelFair;
#[cfg(any())]
pub use self::__AVCaptureSystemPressure::AVCaptureSystemPressureLevelNominal;
#[cfg(any())]
pub use self::__AVCaptureSystemPressure::AVCaptureSystemPressureLevelSerious;
#[cfg(any())]
pub use self::__AVCaptureSystemPressure::AVCaptureSystemPressureLevelShutdown;
#[cfg(any())]
pub use self::__AVCaptureSystemPressure::AVCaptureSystemPressureState;
#[cfg(all(any(), any()))]
pub use self::__AVCaptureSystemZoomSlider::AVCaptureSystemZoomSlider;
#[cfg(all(any(), feature = "objc2-core-media"))]
pub use self::__AVCaptureTimecodeGenerator::AVCaptureTimecode;
#[cfg(all(any(), feature = "objc2-core-media"))]
pub use self::__AVCaptureTimecodeGenerator::AVCaptureTimecodeAdvancedByFrames;
#[cfg(all(any(), feature = "objc2-core-foundation", feature = "objc2-core-media"))]
pub use self::__AVCaptureTimecodeGenerator::AVCaptureTimecodeCreateMetadataSampleBufferAssociatedWithPresentationTimeStamp;
#[cfg(all(any(), feature = "objc2-core-foundation", feature = "objc2-core-media"))]
pub use self::__AVCaptureTimecodeGenerator::AVCaptureTimecodeCreateMetadataSampleBufferForDuration;
#[cfg(any())]
pub use self::__AVCaptureTimecodeGenerator::AVCaptureTimecodeGenerator;
#[cfg(any())]
pub use self::__AVCaptureTimecodeGenerator::AVCaptureTimecodeGeneratorDelegate;
#[cfg(any())]
pub use self::__AVCaptureTimecodeGenerator::AVCaptureTimecodeGeneratorSynchronizationStatus;
#[cfg(any())]
pub use self::__AVCaptureTimecodeGenerator::AVCaptureTimecodeSource;
#[cfg(any())]
pub use self::__AVCaptureTimecodeGenerator::AVCaptureTimecodeSourceType;
#[cfg(all(any(), any()))]
pub use self::__AVCaptureVideoDataOutput::AVCaptureVideoDataOutput;
#[cfg(any())]
pub use self::__AVCaptureVideoDataOutput::AVCaptureVideoDataOutputSampleBufferDelegate;
#[cfg(all(any(), any()))]
#[cfg(not(target_os = "watchos"))]
pub use self::__AVCaptureVideoPreviewLayer::AVCaptureVideoPreviewLayer;
#[cfg(all(feature = "AVAsset", any()))]
pub use self::__AVComposition::AVComposition;
#[cfg(all(feature = "AVAsset", any()))]
pub use self::__AVComposition::AVMutableComposition;
#[cfg(all(feature = "AVAssetTrack", any()))]
pub use self::__AVCompositionTrack::AVCompositionTrack;
#[cfg(any())]
pub use self::__AVCompositionTrack::AVCompositionTrackFormatDescriptionReplacement;
#[cfg(all(feature = "AVAssetTrack", any()))]
pub use self::__AVCompositionTrack::AVMutableCompositionTrack;
#[cfg(all(any(), any()))]
pub use self::__AVCompositionTrackSegment::AVCompositionTrackSegment;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKey;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeyRecipient;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeyRequest;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeyRequestProtocolVersionsKey;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeyRequestRandomDeviceIdentifierSeedKey;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeyRequestRequiresValidationDataInSecureTokenKey;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeyRequestRetryReason;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeyRequestRetryReasonReceivedObsoleteContentKey;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeyRequestRetryReasonReceivedResponseWithExpiredLease;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeyRequestRetryReasonTimedOut;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeyRequestShouldRandomizeDeviceIdentifierKey;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeyRequestStatus;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeyResponse;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeySession;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeySessionDelegate;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeySessionServerPlaybackContextOption;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeySessionServerPlaybackContextOptionProtocolVersions;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeySessionServerPlaybackContextOptionServerChallenge;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeySpecifier;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeySystem;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeySystemAuthorizationToken;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeySystemClearKey;
#[cfg(any())]
pub use self::__AVContentKeySession::AVContentKeySystemFairPlayStreaming;
#[cfg(any())]
pub use self::__AVContentKeySession::AVExternalContentProtectionStatus;
#[cfg(any())]
pub use self::__AVContentKeySession::AVPersistableContentKeyRequest;
#[cfg(all(any(), feature = "objc2-core-media"))]
pub use self::__AVContentKeySession::AVSampleBufferAttachContentKey;
#[cfg(any())]
pub use self::__AVContinuityDevice::AVContinuityDevice;
#[cfg(any())]
pub use self::__AVDepthData::AVDepthData;
#[cfg(any())]
pub use self::__AVDepthData::AVDepthDataAccuracy;
#[cfg(any())]
pub use self::__AVDepthData::AVDepthDataQuality;
#[cfg(any())]
pub use self::__AVError::AVError;
#[cfg(any())]
pub use self::__AVError::AVErrorDeviceKey;
#[cfg(any())]
pub use self::__AVError::AVErrorDiscontinuityFlagsKey;
#[cfg(any())]
pub use self::__AVError::AVErrorFileSizeKey;
#[cfg(any())]
pub use self::__AVError::AVErrorFileTypeKey;
#[cfg(any())]
pub use self::__AVError::AVErrorMediaSubTypeKey;
#[cfg(any())]
pub use self::__AVError::AVErrorMediaTypeKey;
#[cfg(any())]
pub use self::__AVError::AVErrorPIDKey;
#[cfg(any())]
pub use self::__AVError::AVErrorPersistentTrackIDKey;
#[cfg(any())]
pub use self::__AVError::AVErrorPresentationTimeStampKey;
#[cfg(any())]
pub use self::__AVError::AVErrorRecordingSuccessfullyFinishedKey;
#[cfg(any())]
pub use self::__AVError::AVErrorTimeKey;
#[cfg(any())]
pub use self::__AVError::AVFoundationErrorDomain;
#[cfg(any())]
pub use self::__AVExternalStorageDevice::AVExternalStorageDevice;
#[cfg(any())]
pub use self::__AVExternalStorageDevice::AVExternalStorageDeviceDiscoverySession;
#[cfg(any())]
pub use self::__AVExternalSyncDevice::AVExternalSyncDevice;
#[cfg(any())]
pub use self::__AVExternalSyncDevice::AVExternalSyncDeviceDelegate;
#[cfg(any())]
pub use self::__AVExternalSyncDevice::AVExternalSyncDeviceDiscoverySession;
#[cfg(any())]
pub use self::__AVExternalSyncDevice::AVExternalSyncDeviceStatus;
#[cfg(all(any(), feature = "objc2-core-foundation"))]
pub use self::__AVGeometry::AVMakeRectWithAspectRatioInsideRect;
#[cfg(any())]
pub use self::__AVGeometry::NSValueCMVideoDimensionsExtensions;
#[cfg(feature = "AVMediaFormat")]
pub use self::__AVMediaFormat::AVFileType;
#[cfg(feature = "AVMediaFormat")]
pub use self::__AVMediaFormat::AVFileTypeMPEG4;
#[cfg(feature = "AVMediaFormat")]
pub use self::__AVMediaFormat::AVFileTypeProfile;
#[cfg(feature = "AVMediaFormat")]
pub use self::__AVMediaFormat::AVMediaCharacteristic;
#[cfg(feature = "AVMediaFormat")]
pub use self::__AVMediaFormat::AVMediaType;
#[cfg(feature = "AVMediaFormat")]
pub use self::__AVMediaFormat::AVMediaTypeAudio;
#[cfg(feature = "AVMediaFormat")]
pub use self::__AVMediaFormat::AVMediaTypeVideo;
#[cfg(feature = "AVMediaFormat")]
pub use self::__AVMediaFormat::AVVideoRange;
#[cfg(any())]
pub use self::__AVMediaSelection::AVMediaSelection;
#[cfg(any())]
pub use self::__AVMediaSelection::AVMutableMediaSelection;
#[cfg(any())]
pub use self::__AVMediaSelectionGroup::AVCustomMediaSelectionScheme;
#[cfg(any())]
pub use self::__AVMediaSelectionGroup::AVMediaPresentationSelector;
#[cfg(any())]
pub use self::__AVMediaSelectionGroup::AVMediaPresentationSetting;
#[cfg(any())]
pub use self::__AVMediaSelectionGroup::AVMediaSelectionGroup;
#[cfg(any())]
pub use self::__AVMediaSelectionGroup::AVMediaSelectionOption;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyAlbumAndTrack;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyAuthor;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyCollection;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyCopyright;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyDescription;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyGenre;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyKeywordList;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyLocation;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyMediaClassification;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyMediaRating;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyPerformer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyRecordingYear;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyThumbnail;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyTitle;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadata3GPUserDataKeyUserRating;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyAccessibilityDescription;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyAlbumName;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyArtist;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyArtwork;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyAuthor;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyContributor;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyCopyrights;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyCreationDate;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyCreator;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyDescription;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyFormat;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyIdentifier;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyLanguage;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyLastModifiedDate;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyLocation;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyMake;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyModel;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyPublisher;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyRelation;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeySoftware;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeySource;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeySubject;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyTitle;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataCommonKeyType;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataExtraAttributeBaseURIKey;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataExtraAttributeInfoKey;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataExtraAttributeKey;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataExtraAttributeValueURIKey;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataFormat;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataFormatHLSMetadata;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataFormatID3Metadata;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataFormatISOUserData;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataFormatQuickTimeMetadata;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataFormatQuickTimeUserData;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataFormatUnknown;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataFormatiTunesMetadata;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyAlbumSortOrder;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyAlbumTitle;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyAttachedPicture;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyAudioEncryption;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyAudioSeekPointIndex;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyBand;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyBeatsPerMinute;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyComments;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyCommercial;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyCommercialInformation;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyCommerical;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyComposer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyConductor;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyContentGroupDescription;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyContentType;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyCopyright;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyCopyrightInformation;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyDate;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyEncodedBy;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyEncodedWith;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyEncodingTime;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyEncryption;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyEqualization;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyEqualization2;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyEventTimingCodes;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyFileOwner;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyFileType;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyGeneralEncapsulatedObject;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyGroupIdentifier;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyInitialKey;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyInternationalStandardRecordingCode;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyInternetRadioStationName;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyInternetRadioStationOwner;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyInvolvedPeopleList_v23;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyInvolvedPeopleList_v24;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyLanguage;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyLeadPerformer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyLength;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyLink;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyLyricist;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyMPEGLocationLookupTable;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyMediaType;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyModifiedBy;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyMood;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyMusicCDIdentifier;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyMusicianCreditsList;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyOfficialArtistWebpage;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyOfficialAudioFileWebpage;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyOfficialAudioSourceWebpage;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyOfficialInternetRadioStationHomepage;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyOfficialPublisherWebpage;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyOriginalAlbumTitle;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyOriginalArtist;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyOriginalFilename;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyOriginalLyricist;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyOriginalReleaseTime;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyOriginalReleaseYear;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyOwnership;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyPartOfASet;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyPayment;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyPerformerSortOrder;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyPlayCounter;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyPlaylistDelay;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyPopularimeter;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyPositionSynchronization;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyPrivate;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyProducedNotice;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyPublisher;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyRecommendedBufferSize;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyRecordingDates;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyRecordingTime;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyRelativeVolumeAdjustment;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyRelativeVolumeAdjustment2;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyReleaseTime;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyReverb;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeySeek;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeySetSubtitle;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeySignature;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeySize;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeySubTitle;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeySynchronizedLyric;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeySynchronizedTempoCodes;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyTaggingTime;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyTermsOfUse;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyTime;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyTitleDescription;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyTitleSortOrder;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyTrackNumber;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyUniqueFileIdentifier;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyUnsynchronizedLyric;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyUserText;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyUserURL;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataID3MetadataKeyYear;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataISOUserDataKeyAccessibilityDescription;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataISOUserDataKeyCopyright;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataISOUserDataKeyDate;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataISOUserDataKeyTaggedCharacteristic;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataIcyMetadataKeyStreamTitle;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataIcyMetadataKeyStreamURL;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataKey;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataKeySpace;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataKeySpaceAudioFile;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataKeySpaceCommon;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataKeySpaceHLSDateRange;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataKeySpaceID3;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataKeySpaceISOUserData;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataKeySpaceIcy;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataKeySpaceQuickTimeMetadata;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataKeySpaceQuickTimeUserData;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataKeySpaceiTunes;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyAccessibilityDescription;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyAlbum;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyArranger;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyArtist;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyArtwork;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyAuthor;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyCameraFocalLength35mmEquivalent;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyCameraFrameReadoutTime;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyCameraISOSensitivity;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyCameraIdentifier;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyCameraLensIrisFNumber;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyCameraLensModel;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyCameraShutterSpeedAngle;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyCameraShutterSpeedTime;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyCameraWhiteBalance;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyCinematicVideoIntent;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyCollectionUser;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyComment;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyComposer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyContentIdentifier;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyCopyright;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyCreationDate;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyCredits;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyDescription;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyDirectionFacing;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyDirectionMotion;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyDirector;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyDisplayName;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyEncodedBy;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyFullFrameRatePlaybackIntent;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyGenre;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyInformation;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyIsMontage;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyKeywords;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyLocationBody;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyLocationDate;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyLocationISO6709;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyLocationName;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyLocationNote;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyLocationRole;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyMake;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyModel;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyOriginalArtist;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyPerformer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyPhonogramRights;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyProducer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyPublisher;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyRatingUser;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeySoftware;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyTitle;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyWhiteBalanceByCCTColorMatrices;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyWhiteBalanceByCCTWhiteBalanceFactors;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyYear;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeMetadataKeyiXML;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyAccessibilityDescription;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyAlbum;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyArranger;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyArtist;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyAuthor;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyChapter;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyComment;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyComposer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyCopyright;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyCreationDate;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyCredits;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyDescription;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyDirector;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyDisclaimer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyEncodedBy;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyFullName;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyGenre;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyHostComputer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyInformation;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyKeywords;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyLocationISO6709;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyMake;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyModel;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyOriginalArtist;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyOriginalFormat;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyOriginalSource;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyPerformers;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyPhonogramRights;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyProducer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyProduct;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyPublisher;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeySoftware;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeySpecialPlaybackRequirements;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyTaggedCharacteristic;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyTrack;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyTrackName;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyURLLink;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyWarning;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataQuickTimeUserDataKeyWriter;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyAccountKind;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyAcknowledgement;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyAlbum;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyAlbumArtist;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyAppleID;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyArranger;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyArtDirector;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyArtist;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyArtistID;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyAuthor;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyBeatsPerMin;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyComposer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyConductor;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyContentRating;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyCopyright;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyCoverArt;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyCredits;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyDescription;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyDirector;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyDiscCompilation;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyDiscNumber;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyEQ;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyEncodedBy;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyEncodingTool;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyExecProducer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyGenreID;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyGrouping;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyLinerNotes;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyLyrics;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyOnlineExtras;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyOriginalArtist;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyPerformer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyPhonogramRights;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyPlaylistID;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyPredefinedGenre;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyProducer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyPublisher;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyRecordCompany;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyReleaseDate;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeySoloist;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeySongID;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeySongName;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeySoundEngineer;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyThanks;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyTrackNumber;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyTrackSubTitle;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyUserComment;
#[cfg(any())]
pub use self::__AVMetadataFormat::AVMetadataiTunesMetadataKeyUserGenre;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierAccessibilityDescription;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierAlbumName;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierArtist;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierArtwork;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierAssetIdentifier;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierAuthor;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierContributor;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierCopyrights;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierCreationDate;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierCreator;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierDescription;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierFormat;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierLanguage;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierLastModifiedDate;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierLocation;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierMake;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierModel;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierPublisher;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierRelation;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierSoftware;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierSource;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierSubject;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierTitle;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataCommonIdentifierType;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataAlbumAndTrack;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataAuthor;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataCollection;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataCopyright;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataDescription;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataGenre;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataKeywordList;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataLocation;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataMediaClassification;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataMediaRating;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataPerformer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataRecordingYear;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataThumbnail;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataTitle;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifier3GPUserDataUserRating;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataAlbumSortOrder;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataAlbumTitle;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataAttachedPicture;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataAudioEncryption;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataAudioSeekPointIndex;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataBand;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataBeatsPerMinute;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataComments;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataCommercial;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataCommercialInformation;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataCommerical;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataComposer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataConductor;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataContentGroupDescription;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataContentType;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataCopyright;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataCopyrightInformation;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataDate;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataEncodedBy;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataEncodedWith;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataEncodingTime;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataEncryption;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataEqualization;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataEqualization2;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataEventTimingCodes;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataFileOwner;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataFileType;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataGeneralEncapsulatedObject;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataGroupIdentifier;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataInitialKey;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataInternationalStandardRecordingCode;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataInternetRadioStationName;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataInternetRadioStationOwner;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataInvolvedPeopleList_v23;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataInvolvedPeopleList_v24;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataLanguage;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataLeadPerformer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataLength;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataLink;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataLyricist;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataMPEGLocationLookupTable;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataMediaType;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataModifiedBy;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataMood;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataMusicCDIdentifier;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataMusicianCreditsList;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataOfficialArtistWebpage;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataOfficialAudioFileWebpage;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataOfficialAudioSourceWebpage;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataOfficialInternetRadioStationHomepage;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataOfficialPublisherWebpage;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataOriginalAlbumTitle;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataOriginalArtist;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataOriginalFilename;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataOriginalLyricist;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataOriginalReleaseTime;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataOriginalReleaseYear;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataOwnership;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataPartOfASet;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataPayment;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataPerformerSortOrder;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataPlayCounter;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataPlaylistDelay;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataPopularimeter;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataPositionSynchronization;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataPrivate;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataProducedNotice;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataPublisher;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataRecommendedBufferSize;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataRecordingDates;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataRecordingTime;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataRelativeVolumeAdjustment;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataRelativeVolumeAdjustment2;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataReleaseTime;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataReverb;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataSeek;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataSetSubtitle;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataSignature;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataSize;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataSubTitle;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataSynchronizedLyric;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataSynchronizedTempoCodes;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataTaggingTime;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataTermsOfUse;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataTime;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataTitleDescription;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataTitleSortOrder;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataTrackNumber;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataUniqueFileIdentifier;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataUnsynchronizedLyric;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataUserText;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataUserURL;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierID3MetadataYear;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierISOUserDataAccessibilityDescription;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierISOUserDataCopyright;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierISOUserDataDate;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierISOUserDataTaggedCharacteristic;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierIcyMetadataStreamTitle;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierIcyMetadataStreamURL;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataAIMEData;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataAccessibilityDescription;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataAlbum;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataArranger;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataArtist;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataArtwork;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataAuthor;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataAutoLivePhoto;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataCameraFocalLength35mmEquivalent;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataCameraFrameReadoutTime;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataCameraISOSensitivity;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataCameraIdentifier;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataCameraLensIrisFNumber;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataCameraLensModel;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataCameraShutterSpeedAngle;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataCameraShutterSpeedTime;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataCameraWhiteBalance;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataCinematicVideoIntent;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataCollectionUser;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataComment;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataComposer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataContentIdentifier;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataCopyright;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataCreationDate;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataCredits;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataDescription;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataDetectedCatBody;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataDetectedDogBody;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataDetectedFace;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataDetectedHumanBody;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataDetectedSalientObject;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataDirectionFacing;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataDirectionMotion;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataDirector;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataDisplayName;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataEncodedBy;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataFullFrameRatePlaybackIntent;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataGenre;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataInformation;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataIsMontage;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataKeywords;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataLivePhotoVitalityScore;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataLivePhotoVitalityScoringVersion;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataLocationBody;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataLocationDate;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataLocationHorizontalAccuracyInMeters;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataLocationISO6709;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataLocationName;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataLocationNote;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataLocationRole;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataMake;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataModel;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataOriginalArtist;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataPerformer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataPhonogramRights;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataPreferredAffineTransform;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataPresentationImmersiveMedia;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataProducer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataPublisher;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataRatingUser;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataSoftware;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataSpatialOverCaptureQualityScore;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataSpatialOverCaptureQualityScoringVersion;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataTitle;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataVideoOrientation;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataWhiteBalanceByCCTColorMatrices;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataWhiteBalanceByCCTWhiteBalanceFactors;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataYear;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeMetadataiXML;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataAccessibilityDescription;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataAlbum;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataArranger;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataArtist;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataAuthor;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataChapter;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataComment;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataComposer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataCopyright;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataCreationDate;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataCredits;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataDescription;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataDirector;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataDisclaimer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataEncodedBy;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataFullName;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataGenre;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataHostComputer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataInformation;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataKeywords;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataLocationISO6709;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataMake;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataModel;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataOriginalArtist;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataOriginalFormat;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataOriginalSource;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataPerformers;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataPhonogramRights;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataProducer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataProduct;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataPublisher;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataSoftware;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataSpecialPlaybackRequirements;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataTaggedCharacteristic;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataTrack;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataTrackName;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataURLLink;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataWarning;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifierQuickTimeUserDataWriter;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataAccountKind;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataAcknowledgement;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataAlbum;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataAlbumArtist;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataAppleID;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataArranger;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataArtDirector;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataArtist;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataArtistID;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataAuthor;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataBeatsPerMin;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataComposer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataConductor;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataContentRating;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataCopyright;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataCoverArt;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataCredits;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataDescription;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataDirector;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataDiscCompilation;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataDiscNumber;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataEQ;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataEncodedBy;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataEncodingTool;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataExecProducer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataGenreID;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataGrouping;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataLinerNotes;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataLyrics;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataOnlineExtras;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataOriginalArtist;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataPerformer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataPhonogramRights;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataPlaylistID;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataPredefinedGenre;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataProducer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataPublisher;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataRecordCompany;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataReleaseDate;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataSoloist;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataSongID;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataSongName;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataSoundEngineer;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataThanks;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataTrackNumber;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataTrackSubTitle;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataUserComment;
#[cfg(any())]
pub use self::__AVMetadataIdentifiers::AVMetadataIdentifieriTunesMetadataUserGenre;
#[cfg(any())]
pub use self::__AVMetadataItem::AVMetadataItem;
#[cfg(any())]
pub use self::__AVMetadataItem::AVMetadataItemFilter;
#[cfg(any())]
pub use self::__AVMetadataItem::AVMetadataItemValueRequest;
#[cfg(any())]
pub use self::__AVMetadataItem::AVMutableMetadataItem;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataBodyObject;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataCatBodyObject;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataCatHeadObject;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataDogBodyObject;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataDogHeadObject;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataFaceObject;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataHumanBodyObject;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataHumanFullBodyObject;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataMachineReadableCodeObject;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObject;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectType;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeAztecCode;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeCatBody;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeCatHead;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeCodabarCode;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeCode128Code;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeCode39Code;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeCode39Mod43Code;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeCode93Code;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeDataMatrixCode;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeDogBody;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeDogHead;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeEAN13Code;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeEAN8Code;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeFace;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeGS1DataBarCode;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeGS1DataBarExpandedCode;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeGS1DataBarLimitedCode;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeHumanBody;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeHumanFullBody;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeITF14Code;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeInterleaved2of5Code;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeMicroPDF417Code;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeMicroQRCode;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypePDF417Code;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeQRCode;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeSalientObject;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataObjectTypeUPCECode;
#[cfg(any())]
pub use self::__AVMetadataObject::AVMetadataSalientObject;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricContentKeyRequestEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricDownloadSummaryEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricErrorEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricEventStream;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricEventStreamPublisher;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricEventStreamSubscriber;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricHLSMediaSegmentRequestEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricHLSPlaylistRequestEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricMediaRendition;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricMediaResourceRequestEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricPlayerItemInitialLikelyToKeepUpEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricPlayerItemLikelyToKeepUpEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricPlayerItemPlaybackSummaryEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricPlayerItemRateChangeEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricPlayerItemSeekDidCompleteEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricPlayerItemSeekEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricPlayerItemStallEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricPlayerItemVariantSwitchEvent;
#[cfg(any())]
pub use self::__AVMetrics::AVMetricPlayerItemVariantSwitchStartEvent;
#[cfg(all(feature = "AVAsset", any()))]
pub use self::__AVMovie::AVFragmentedMovie;
#[cfg(any())]
pub use self::__AVMovie::AVFragmentedMovieContainsMovieFragmentsDidChangeNotification;
#[cfg(any())]
pub use self::__AVMovie::AVFragmentedMovieDurationDidChangeNotification;
#[cfg(all(feature = "AVAsset", any()))]
pub use self::__AVMovie::AVFragmentedMovieMinder;
#[cfg(any())]
pub use self::__AVMovie::AVFragmentedMovieWasDefragmentedNotification;
#[cfg(any())]
pub use self::__AVMovie::AVMediaDataStorage;
#[cfg(all(feature = "AVAsset", any()))]
pub use self::__AVMovie::AVMovie;
#[cfg(any())]
pub use self::__AVMovie::AVMovieReferenceRestrictionsKey;
#[cfg(any())]
pub use self::__AVMovie::AVMovieShouldSupportAliasDataReferencesKey;
#[cfg(any())]
pub use self::__AVMovie::AVMovieWritingOptions;
#[cfg(all(feature = "AVAsset", any()))]
pub use self::__AVMovie::AVMutableMovie;
#[cfg(all(feature = "AVAssetTrack", any()))]
pub use self::__AVMovieTrack::AVFragmentedMovieTrack;
#[cfg(any())]
pub use self::__AVMovieTrack::AVFragmentedMovieTrackSegmentsDidChangeNotification;
#[cfg(any())]
pub use self::__AVMovieTrack::AVFragmentedMovieTrackTimeRangeDidChangeNotification;
#[cfg(any())]
pub use self::__AVMovieTrack::AVFragmentedMovieTrackTotalSampleDataLengthDidChangeNotification;
#[cfg(all(feature = "AVAssetTrack", any()))]
pub use self::__AVMovieTrack::AVMovieTrack;
#[cfg(all(feature = "AVAssetTrack", any()))]
pub use self::__AVMovieTrack::AVMutableMovieTrack;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsAssistant;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPreset;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPreset1280x720;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPreset1920x1080;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPreset3840x2160;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPreset640x480;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPreset960x540;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPresetHEVC1920x1080;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPresetHEVC1920x1080WithAlpha;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPresetHEVC3840x2160;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPresetHEVC3840x2160WithAlpha;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPresetHEVC4320x2160;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPresetHEVC7680x4320;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPresetMVHEVC1440x1440;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPresetMVHEVC4320x4320;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPresetMVHEVC7680x7680;
#[cfg(any())]
pub use self::__AVOutputSettingsAssistant::AVOutputSettingsPresetMVHEVC960x960;
#[cfg(any())]
pub use self::__AVPlaybackCoordinationMedium::AVPlaybackCoordinationMedium;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVCoordinatedPlaybackParticipant;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVCoordinatedPlaybackSuspension;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVCoordinatedPlaybackSuspensionReason;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVCoordinatedPlaybackSuspensionReasonAudioSessionInterrupted;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVCoordinatedPlaybackSuspensionReasonCoordinatedPlaybackNotPossible;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVCoordinatedPlaybackSuspensionReasonPlayingInterstitial;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVCoordinatedPlaybackSuspensionReasonStallRecovery;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVCoordinatedPlaybackSuspensionReasonUserActionRequired;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVCoordinatedPlaybackSuspensionReasonUserIsChangingCurrentTime;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVDelegatingPlaybackCoordinator;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVDelegatingPlaybackCoordinatorBufferingCommand;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVDelegatingPlaybackCoordinatorPauseCommand;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVDelegatingPlaybackCoordinatorPlayCommand;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVDelegatingPlaybackCoordinatorPlaybackControlCommand;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVDelegatingPlaybackCoordinatorRateChangeOptions;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVDelegatingPlaybackCoordinatorSeekCommand;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVDelegatingPlaybackCoordinatorSeekOptions;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVPlaybackCoordinator;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVPlaybackCoordinatorOtherParticipantsDidChangeNotification;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVPlaybackCoordinatorPlaybackControlDelegate;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVPlaybackCoordinatorSuspensionReasonsDidChangeNotification;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVPlayerPlaybackCoordinator;
#[cfg(any())]
pub use self::__AVPlaybackCoordinator::AVPlayerPlaybackCoordinatorDelegate;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayer;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerActionAtItemEnd;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerAudiovisualBackgroundPlaybackPolicy;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerAvailableHDRModesDidChangeNotification;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerEligibleForHDRPlaybackDidChangeNotification;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerHDRMode;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerNetworkResourcePriority;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerRateDidChangeNotification;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerRateDidChangeOriginatingParticipantKey;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerRateDidChangeReason;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerRateDidChangeReasonAppBackgrounded;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerRateDidChangeReasonAudioSessionInterrupted;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerRateDidChangeReasonKey;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerRateDidChangeReasonSetRateCalled;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerRateDidChangeReasonSetRateFailed;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerStatus;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerTimeControlStatus;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerWaitingForCoordinatedPlaybackReason;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerWaitingReason;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerWaitingToMinimizeStallsReason;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerWaitingWhileEvaluatingBufferingRateReason;
#[cfg(any())]
pub use self::__AVPlayer::AVPlayerWaitingWithNoItemToPlayReason;
#[cfg(any())]
pub use self::__AVPlayer::AVQueuePlayer;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEvent;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventAssetListResponseStatus;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventController;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventCue;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventJoinCue;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventLeaveCue;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitor;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorAssetListResponseStatusDidChangeErrorKey;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorAssetListResponseStatusDidChangeEventKey;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorAssetListResponseStatusDidChangeNotification;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorAssetListResponseStatusDidChangeStatusKey;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorCurrentEventDidChangeNotification;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorCurrentEventSkippableStateDidChangeEventKey;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorCurrentEventSkippableStateDidChangeNotification;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorCurrentEventSkippableStateDidChangeSkipControlLabelKey;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorCurrentEventSkippableStateDidChangeStateKey;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorCurrentEventSkippedEventKey;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorCurrentEventSkippedNotification;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorEventsDidChangeNotification;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorInterstitialEventDidFinishDidPlayEntireEventKey;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorInterstitialEventDidFinishEventKey;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorInterstitialEventDidFinishNotification;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorInterstitialEventDidFinishPlayoutTimeKey;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorInterstitialEventWasUnscheduledErrorKey;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorInterstitialEventWasUnscheduledEventKey;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventMonitorInterstitialEventWasUnscheduledNotification;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventNoCue;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventRestrictions;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventSkippableEventState;
#[cfg(any())]
pub use self::__AVPlayerInterstitialEventController::AVPlayerInterstitialEventTimelineOccupancy;
#[cfg(all(any(), any()))]
pub use self::__AVPlayerInterstitialEventController::AVPlayerWaitingDuringInterstitialEventReason;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItem;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemAccessLog;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemAccessLogEvent;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemDidPlayToEndTimeNotification;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemErrorLog;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemErrorLogEvent;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemFailedToPlayToEndTimeErrorKey;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemFailedToPlayToEndTimeNotification;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemMediaSelectionDidChangeNotification;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemNewAccessLogEntryNotification;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemNewErrorLogEntryNotification;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemPlaybackStalledNotification;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemRecommendedTimeOffsetFromLiveDidChangeNotification;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemStatus;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemTimeJumpedNotification;
#[cfg(any())]
pub use self::__AVPlayerItem::AVPlayerItemTimeJumpedOriginatingParticipantKey;
#[cfg(any())]
pub use self::__AVPlayerItem::AVVariantPreferences;
#[cfg(any())]
pub use self::__AVPlayerItemIntegratedTimeline::AVPlayerIntegratedTimelineSnapshotsOutOfSyncNotification;
#[cfg(any())]
pub use self::__AVPlayerItemIntegratedTimeline::AVPlayerIntegratedTimelineSnapshotsOutOfSyncReason;
#[cfg(any())]
pub use self::__AVPlayerItemIntegratedTimeline::AVPlayerIntegratedTimelineSnapshotsOutOfSyncReasonCurrentSegmentChanged;
#[cfg(any())]
pub use self::__AVPlayerItemIntegratedTimeline::AVPlayerIntegratedTimelineSnapshotsOutOfSyncReasonKey;
#[cfg(any())]
pub use self::__AVPlayerItemIntegratedTimeline::AVPlayerIntegratedTimelineSnapshotsOutOfSyncReasonLoadedTimeRangesChanged;
#[cfg(any())]
pub use self::__AVPlayerItemIntegratedTimeline::AVPlayerIntegratedTimelineSnapshotsOutOfSyncReasonSegmentsChanged;
#[cfg(any())]
pub use self::__AVPlayerItemIntegratedTimeline::AVPlayerItemIntegratedTimeline;
#[cfg(any())]
pub use self::__AVPlayerItemIntegratedTimeline::AVPlayerItemIntegratedTimelineObserver;
#[cfg(any())]
pub use self::__AVPlayerItemIntegratedTimeline::AVPlayerItemIntegratedTimelineSnapshot;
#[cfg(any())]
pub use self::__AVPlayerItemIntegratedTimeline::AVPlayerItemSegment;
#[cfg(any())]
pub use self::__AVPlayerItemIntegratedTimeline::AVPlayerItemSegmentType;
#[cfg(any())]
pub use self::__AVPlayerItemMediaDataCollector::AVPlayerItemMediaDataCollector;
#[cfg(any())]
pub use self::__AVPlayerItemMediaDataCollector::AVPlayerItemMetadataCollector;
#[cfg(any())]
pub use self::__AVPlayerItemMediaDataCollector::AVPlayerItemMetadataCollectorPushDelegate;
#[cfg(any())]
pub use self::__AVPlayerItemOutput::AVPlayerItemLegibleOutput;
#[cfg(any())]
pub use self::__AVPlayerItemOutput::AVPlayerItemLegibleOutputPushDelegate;
#[cfg(any())]
pub use self::__AVPlayerItemOutput::AVPlayerItemLegibleOutputTextStylingResolution;
#[cfg(any())]
pub use self::__AVPlayerItemOutput::AVPlayerItemLegibleOutputTextStylingResolutionDefault;
#[cfg(any())]
pub use self::__AVPlayerItemOutput::AVPlayerItemLegibleOutputTextStylingResolutionSourceAndRulesOnly;
#[cfg(any())]
pub use self::__AVPlayerItemOutput::AVPlayerItemMetadataOutput;
#[cfg(any())]
pub use self::__AVPlayerItemOutput::AVPlayerItemMetadataOutputPushDelegate;
#[cfg(any())]
pub use self::__AVPlayerItemOutput::AVPlayerItemOutput;
#[cfg(any())]
pub use self::__AVPlayerItemOutput::AVPlayerItemOutputPullDelegate;
#[cfg(any())]
pub use self::__AVPlayerItemOutput::AVPlayerItemOutputPushDelegate;
#[cfg(any())]
pub use self::__AVPlayerItemOutput::AVPlayerItemRenderedLegibleOutput;
#[cfg(any())]
pub use self::__AVPlayerItemOutput::AVPlayerItemRenderedLegibleOutputPushDelegate;
#[cfg(any())]
pub use self::__AVPlayerItemOutput::AVPlayerItemVideoOutput;
#[cfg(any())]
pub use self::__AVPlayerItemProtectedContentAdditions::AVContentAuthorizationStatus;
#[cfg(any())]
pub use self::__AVPlayerItemTrack::AVPlayerItemTrack;
#[cfg(any())]
pub use self::__AVPlayerItemTrack::AVPlayerItemTrackVideoFieldModeDeinterlaceFields;
#[cfg(all(any(), any()))]
#[cfg(not(target_os = "watchos"))]
pub use self::__AVPlayerLayer::AVPlayerLayer;
#[cfg(any())]
pub use self::__AVPlayerLooper::AVPlayerLooper;
#[cfg(any())]
pub use self::__AVPlayerLooper::AVPlayerLooperItemOrdering;
#[cfg(any())]
pub use self::__AVPlayerLooper::AVPlayerLooperStatus;
#[cfg(any())]
pub use self::__AVPlayerMediaSelectionCriteria::AVPlayerMediaSelectionCriteria;
#[cfg(any())]
pub use self::__AVPlayerOutput::AVPlayerVideoOutput;
#[cfg(any())]
pub use self::__AVPlayerOutput::AVPlayerVideoOutputConfiguration;
#[cfg(any())]
pub use self::__AVPlayerOutput::AVVideoOutputSpecification;
#[cfg(all(any(), feature = "objc2-core-foundation", feature = "objc2-core-media"))]
pub use self::__AVPlayerOutput::CMTagCollectionCreateWithVideoOutputPreset;
#[cfg(any())]
pub use self::__AVPlayerOutput::CMTagCollectionVideoOutputPreset;
#[cfg(any())]
pub use self::__AVPortraitEffectsMatte::AVPortraitEffectsMatte;
#[cfg(any())]
pub use self::__AVQueuedSampleBufferRendering::AVQueuedSampleBufferRendering;
#[cfg(any())]
pub use self::__AVQueuedSampleBufferRendering::AVQueuedSampleBufferRenderingStatus;
#[cfg(any())]
pub use self::__AVRenderedCaptionImage::AVRenderedCaptionImage;
#[cfg(any())]
pub use self::__AVRouteDetector::AVRouteDetector;
#[cfg(any())]
pub use self::__AVRouteDetector::AVRouteDetectorMultipleRoutesDetectedDidChangeNotification;
#[cfg(any())]
pub use self::__AVSampleBufferAudioRenderer::AVSampleBufferAudioRenderer;
#[cfg(any())]
pub use self::__AVSampleBufferAudioRenderer::AVSampleBufferAudioRendererFlushTimeKey;
#[cfg(any())]
pub use self::__AVSampleBufferAudioRenderer::AVSampleBufferAudioRendererOutputConfigurationDidChangeNotification;
#[cfg(any())]
pub use self::__AVSampleBufferAudioRenderer::AVSampleBufferAudioRendererWasFlushedAutomaticallyNotification;
#[cfg(all(any(), any()))]
#[cfg(not(target_os = "watchos"))]
pub use self::__AVSampleBufferDisplayLayer::AVSampleBufferDisplayLayer;
#[cfg(any())]
pub use self::__AVSampleBufferDisplayLayer::AVSampleBufferDisplayLayerFailedToDecodeNotification;
#[cfg(any())]
pub use self::__AVSampleBufferDisplayLayer::AVSampleBufferDisplayLayerFailedToDecodeNotificationErrorKey;
#[cfg(any())]
pub use self::__AVSampleBufferDisplayLayer::AVSampleBufferDisplayLayerOutputObscuredDueToInsufficientExternalProtectionDidChangeNotification;
#[cfg(any())]
pub use self::__AVSampleBufferDisplayLayer::AVSampleBufferDisplayLayerReadyForDisplayDidChangeNotification;
#[cfg(any())]
pub use self::__AVSampleBufferDisplayLayer::AVSampleBufferDisplayLayerRequiresFlushToResumeDecodingDidChangeNotification;
#[cfg(any())]
pub use self::__AVSampleBufferGenerator::AVSampleBufferGenerator;
#[cfg(any())]
pub use self::__AVSampleBufferGenerator::AVSampleBufferGeneratorBatch;
#[cfg(any())]
pub use self::__AVSampleBufferGenerator::AVSampleBufferRequest;
#[cfg(any())]
pub use self::__AVSampleBufferGenerator::AVSampleBufferRequestDirection;
#[cfg(any())]
pub use self::__AVSampleBufferGenerator::AVSampleBufferRequestMode;
#[cfg(any())]
pub use self::__AVSampleBufferRenderSynchronizer::AVSampleBufferRenderSynchronizer;
#[cfg(any())]
pub use self::__AVSampleBufferRenderSynchronizer::AVSampleBufferRenderSynchronizerRateDidChangeNotification;
#[cfg(any())]
pub use self::__AVSampleBufferVideoRenderer::AVSampleBufferVideoRenderer;
#[cfg(any())]
pub use self::__AVSampleBufferVideoRenderer::AVSampleBufferVideoRendererDidFailToDecodeNotification;
#[cfg(any())]
pub use self::__AVSampleBufferVideoRenderer::AVSampleBufferVideoRendererDidFailToDecodeNotificationErrorKey;
#[cfg(any())]
pub use self::__AVSampleBufferVideoRenderer::AVSampleBufferVideoRendererRequiresFlushToResumeDecodingDidChangeNotification;
#[cfg(any())]
pub use self::__AVSampleCursor::AVSampleCursor;
#[cfg(any())]
pub use self::__AVSampleCursor::AVSampleCursorAudioDependencyInfo;
#[cfg(any())]
pub use self::__AVSampleCursor::AVSampleCursorChunkInfo;
#[cfg(any())]
pub use self::__AVSampleCursor::AVSampleCursorDependencyInfo;
#[cfg(any())]
pub use self::__AVSampleCursor::AVSampleCursorStorageRange;
#[cfg(any())]
pub use self::__AVSampleCursor::AVSampleCursorSyncInfo;
#[cfg(any())]
pub use self::__AVSemanticSegmentationMatte::AVSemanticSegmentationMatte;
#[cfg(any())]
pub use self::__AVSemanticSegmentationMatte::AVSemanticSegmentationMatteType;
#[cfg(any())]
pub use self::__AVSemanticSegmentationMatte::AVSemanticSegmentationMatteTypeGlasses;
#[cfg(any())]
pub use self::__AVSemanticSegmentationMatte::AVSemanticSegmentationMatteTypeHair;
#[cfg(any())]
pub use self::__AVSemanticSegmentationMatte::AVSemanticSegmentationMatteTypeSkin;
#[cfg(any())]
pub use self::__AVSemanticSegmentationMatte::AVSemanticSegmentationMatteTypeTeeth;
#[cfg(any())]
pub use self::__AVSpatialVideoConfiguration::AVSpatialVideoConfiguration;
#[cfg(all(any(), any()))]
#[cfg(not(target_os = "watchos"))]
pub use self::__AVSynchronizedLayer::AVSynchronizedLayer;
#[cfg(any())]
pub use self::__AVTextStyleRule::AVTextStyleRule;
#[cfg(any())]
pub use self::__AVTime::NSCoderAVTimeCoding;
#[cfg(any())]
pub use self::__AVTime::NSValueAVFoundationExtensions;
#[cfg(any())]
pub use self::__AVTimedMetadataGroup::AVDateRangeMetadataGroup;
#[cfg(any())]
pub use self::__AVTimedMetadataGroup::AVMetadataGroup;
#[cfg(any())]
pub use self::__AVTimedMetadataGroup::AVMutableDateRangeMetadataGroup;
#[cfg(any())]
pub use self::__AVTimedMetadataGroup::AVMutableTimedMetadataGroup;
#[cfg(any())]
pub use self::__AVTimedMetadataGroup::AVTimedMetadataGroup;
#[cfg(any())]
pub use self::__AVVideoCompositing::AVAsynchronousCIImageFilteringRequest;
#[cfg(any())]
pub use self::__AVVideoCompositing::AVAsynchronousVideoCompositionRequest;
#[cfg(all(any(), feature = "objc2-core-foundation"))]
pub use self::__AVVideoCompositing::AVEdgeWidths;
#[cfg(any())]
pub use self::__AVVideoCompositing::AVPixelAspectRatio;
#[cfg(any())]
pub use self::__AVVideoCompositing::AVVideoCompositing;
#[cfg(any())]
pub use self::__AVVideoCompositing::AVVideoCompositionInstructionProtocol;
#[cfg(any())]
pub use self::__AVVideoCompositing::AVVideoCompositionRenderContext;
#[cfg(any())]
pub use self::__AVVideoCompositing::AVVideoCompositionRenderHint;
#[cfg(any())]
pub use self::__AVVideoComposition::AVMutableVideoComposition;
#[cfg(any())]
pub use self::__AVVideoComposition::AVMutableVideoCompositionInstruction;
#[cfg(any())]
pub use self::__AVVideoComposition::AVMutableVideoCompositionLayerInstruction;
#[cfg(any())]
pub use self::__AVVideoComposition::AVVideoComposition;
#[cfg(any())]
pub use self::__AVVideoComposition::AVVideoCompositionCoreAnimationTool;
#[cfg(any())]
pub use self::__AVVideoComposition::AVVideoCompositionInstruction;
#[cfg(any())]
pub use self::__AVVideoComposition::AVVideoCompositionLayerInstruction;
#[cfg(any())]
pub use self::__AVVideoComposition::AVVideoCompositionPerFrameHDRDisplayMetadataPolicy;
#[cfg(any())]
pub use self::__AVVideoComposition::AVVideoCompositionPerFrameHDRDisplayMetadataPolicyGenerate;
#[cfg(any())]
pub use self::__AVVideoComposition::AVVideoCompositionPerFrameHDRDisplayMetadataPolicyPropagate;
#[cfg(any())]
pub use self::__AVVideoComposition::AVVideoCompositionValidationHandling;
#[cfg(any())]
pub use self::__AVVideoPerformanceMetrics::AVVideoPerformanceMetrics;
#[cfg(feature = "AVVideoSettings")]
pub use self::__AVVideoSettings::AVVideoApertureMode;
#[cfg(feature = "AVVideoSettings")]
pub use self::__AVVideoSettings::AVVideoCodecKey;
#[cfg(feature = "AVVideoSettings")]
pub use self::__AVVideoSettings::AVVideoCodecType;
#[cfg(feature = "AVVideoSettings")]
pub use self::__AVVideoSettings::AVVideoCodecTypeH264;
#[cfg(feature = "AVVideoSettings")]
pub use self::__AVVideoSettings::AVVideoHeightKey;
#[cfg(feature = "AVVideoSettings")]
pub use self::__AVVideoSettings::AVVideoWidthKey;
