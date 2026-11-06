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
#[cfg_attr(target_vendor = "apple", link(name = "Foundation", kind = "framework"))]
extern "C" {}
#[cfg(any())]
#[path = "FoundationErrors.rs"]
mod __FoundationErrors;
#[cfg(any())]
#[path = "FoundationLegacySwiftCompatibility.rs"]
mod __FoundationLegacySwiftCompatibility;
#[cfg(any())]
#[path = "NSAffineTransform.rs"]
mod __NSAffineTransform;
#[cfg(any())]
#[path = "NSAppleEventDescriptor.rs"]
mod __NSAppleEventDescriptor;
#[cfg(any())]
#[path = "NSAppleEventManager.rs"]
mod __NSAppleEventManager;
#[cfg(any())]
#[path = "NSAppleScript.rs"]
mod __NSAppleScript;
#[cfg(any())]
#[path = "NSArchiver.rs"]
mod __NSArchiver;
#[cfg(feature = "NSArray")]
#[path = "NSArray.rs"]
mod __NSArray;
#[cfg(any())]
#[path = "NSAttributedString.rs"]
mod __NSAttributedString;
#[cfg(any())]
#[path = "NSAutoreleasePool.rs"]
mod __NSAutoreleasePool;
#[cfg(any())]
#[path = "NSBackgroundActivityScheduler.rs"]
mod __NSBackgroundActivityScheduler;
#[cfg(any())]
#[path = "NSBundle.rs"]
mod __NSBundle;
#[cfg(any())]
#[path = "NSByteCountFormatter.rs"]
mod __NSByteCountFormatter;
#[cfg(any())]
#[path = "NSByteOrder.rs"]
mod __NSByteOrder;
#[cfg(any())]
#[path = "NSCache.rs"]
mod __NSCache;
#[cfg(any())]
#[path = "NSCalendar.rs"]
mod __NSCalendar;
#[cfg(any())]
#[path = "NSCalendarDate.rs"]
mod __NSCalendarDate;
#[cfg(any())]
#[path = "NSCharacterSet.rs"]
mod __NSCharacterSet;
#[cfg(any())]
#[path = "NSClassDescription.rs"]
mod __NSClassDescription;
#[cfg(any())]
#[path = "NSCoder.rs"]
mod __NSCoder;
#[cfg(any())]
#[path = "NSComparisonPredicate.rs"]
mod __NSComparisonPredicate;
#[cfg(any())]
#[path = "NSCompoundPredicate.rs"]
mod __NSCompoundPredicate;
#[cfg(any())]
#[path = "NSConnection.rs"]
mod __NSConnection;
#[cfg(feature = "NSData")]
#[path = "NSData.rs"]
mod __NSData;
#[cfg(feature = "NSDate")]
#[path = "NSDate.rs"]
mod __NSDate;
#[cfg(any())]
#[path = "NSDateComponentsFormatter.rs"]
mod __NSDateComponentsFormatter;
#[cfg(any())]
#[path = "NSDateFormatter.rs"]
mod __NSDateFormatter;
#[cfg(any())]
#[path = "NSDateInterval.rs"]
mod __NSDateInterval;
#[cfg(any())]
#[path = "NSDateIntervalFormatter.rs"]
mod __NSDateIntervalFormatter;
#[cfg(any())]
#[path = "NSDebug.rs"]
mod __NSDebug;
#[cfg(any())]
#[path = "NSDecimal.rs"]
mod __NSDecimal;
#[cfg(any())]
#[path = "NSDecimalNumber.rs"]
mod __NSDecimalNumber;
#[cfg(feature = "NSDictionary")]
#[path = "NSDictionary.rs"]
mod __NSDictionary;
#[cfg(any())]
#[path = "NSDistantObject.rs"]
mod __NSDistantObject;
#[cfg(any())]
#[path = "NSDistributedLock.rs"]
mod __NSDistributedLock;
#[cfg(any())]
#[path = "NSDistributedNotificationCenter.rs"]
mod __NSDistributedNotificationCenter;
#[cfg(any())]
#[path = "NSEnergyFormatter.rs"]
mod __NSEnergyFormatter;
#[cfg(any())]
#[path = "NSEnumerator.rs"]
mod __NSEnumerator;
#[cfg(feature = "NSError")]
#[path = "NSError.rs"]
mod __NSError;
#[cfg(any())]
#[path = "NSException.rs"]
mod __NSException;
#[cfg(any())]
#[path = "NSExpression.rs"]
mod __NSExpression;
#[cfg(any())]
#[path = "NSExtensionContext.rs"]
mod __NSExtensionContext;
#[cfg(any())]
#[path = "NSExtensionItem.rs"]
mod __NSExtensionItem;
#[cfg(any())]
#[path = "NSExtensionRequestHandling.rs"]
mod __NSExtensionRequestHandling;
#[cfg(any())]
#[path = "NSFileCoordinator.rs"]
mod __NSFileCoordinator;
#[cfg(any())]
#[path = "NSFileHandle.rs"]
mod __NSFileHandle;
#[cfg(any())]
#[path = "NSFileManager.rs"]
mod __NSFileManager;
#[cfg(any())]
#[path = "NSFilePresenter.rs"]
mod __NSFilePresenter;
#[cfg(any())]
#[path = "NSFileVersion.rs"]
mod __NSFileVersion;
#[cfg(any())]
#[path = "NSFileWrapper.rs"]
mod __NSFileWrapper;
#[cfg(any())]
#[path = "NSFormatter.rs"]
mod __NSFormatter;
#[cfg(any())]
#[path = "NSGarbageCollector.rs"]
mod __NSGarbageCollector;
#[cfg(any())]
#[path = "NSGeometry.rs"]
mod __NSGeometry;
#[cfg(any())]
#[path = "NSHFSFileTypes.rs"]
mod __NSHFSFileTypes;
#[cfg(any())]
#[path = "NSHTTPCookie.rs"]
mod __NSHTTPCookie;
#[cfg(any())]
#[path = "NSHTTPCookieStorage.rs"]
mod __NSHTTPCookieStorage;
#[cfg(any())]
#[path = "NSHashTable.rs"]
mod __NSHashTable;
#[cfg(any())]
#[path = "NSHost.rs"]
mod __NSHost;
#[cfg(any())]
#[path = "NSISO8601DateFormatter.rs"]
mod __NSISO8601DateFormatter;
#[cfg(any())]
#[path = "NSIndexPath.rs"]
mod __NSIndexPath;
#[cfg(any())]
#[path = "NSIndexSet.rs"]
mod __NSIndexSet;
#[cfg(any())]
#[path = "NSInflectionRule.rs"]
mod __NSInflectionRule;
#[cfg(any())]
#[path = "NSInvocation.rs"]
mod __NSInvocation;
#[cfg(feature = "NSItemProvider")]
#[path = "NSItemProvider.rs"]
mod __NSItemProvider;
#[cfg(any())]
#[path = "NSJSONSerialization.rs"]
mod __NSJSONSerialization;
#[cfg(any())]
#[path = "NSKeyValueCoding.rs"]
mod __NSKeyValueCoding;
#[cfg(any())]
#[path = "NSKeyValueObserving.rs"]
mod __NSKeyValueObserving;
#[cfg(any())]
#[path = "NSKeyValueSharedObservers.rs"]
mod __NSKeyValueSharedObservers;
#[cfg(any())]
#[path = "NSKeyedArchiver.rs"]
mod __NSKeyedArchiver;
#[cfg(any())]
#[path = "NSLengthFormatter.rs"]
mod __NSLengthFormatter;
#[cfg(any())]
#[path = "NSLinguisticTagger.rs"]
mod __NSLinguisticTagger;
#[cfg(any())]
#[path = "NSListFormatter.rs"]
mod __NSListFormatter;
#[cfg(feature = "NSLocale")]
#[path = "NSLocale.rs"]
mod __NSLocale;
#[cfg(any())]
#[path = "NSLocalizedNumberFormatRule.rs"]
mod __NSLocalizedNumberFormatRule;
#[cfg(any())]
#[path = "NSLock.rs"]
mod __NSLock;
#[cfg(any())]
#[path = "NSMapTable.rs"]
mod __NSMapTable;
#[cfg(any())]
#[path = "NSMassFormatter.rs"]
mod __NSMassFormatter;
#[cfg(any())]
#[path = "NSMeasurement.rs"]
mod __NSMeasurement;
#[cfg(any())]
#[path = "NSMeasurementFormatter.rs"]
mod __NSMeasurementFormatter;
#[cfg(any())]
#[path = "NSMetadata.rs"]
mod __NSMetadata;
#[cfg(any())]
#[path = "NSMetadataAttributes.rs"]
mod __NSMetadataAttributes;
#[cfg(any())]
#[path = "NSMethodSignature.rs"]
mod __NSMethodSignature;
#[cfg(any())]
#[path = "NSMorphology.rs"]
mod __NSMorphology;
#[cfg(any())]
#[path = "NSNetServices.rs"]
mod __NSNetServices;
#[cfg(any())]
#[path = "NSNotification.rs"]
mod __NSNotification;
#[cfg(any())]
#[path = "NSNotificationQueue.rs"]
mod __NSNotificationQueue;
#[cfg(any())]
#[path = "NSNull.rs"]
mod __NSNull;
#[cfg(any())]
#[path = "NSNumberFormatter.rs"]
mod __NSNumberFormatter;
#[cfg(any())]
#[path = "NSObjCRuntime.rs"]
mod __NSObjCRuntime;
#[cfg(feature = "NSObject")]
#[path = "NSObject.rs"]
mod __NSObject;
#[cfg(any())]
#[path = "NSObjectScripting.rs"]
mod __NSObjectScripting;
#[cfg(any())]
#[path = "NSOperation.rs"]
mod __NSOperation;
#[cfg(any())]
#[path = "NSOrderedCollectionChange.rs"]
mod __NSOrderedCollectionChange;
#[cfg(any())]
#[path = "NSOrderedCollectionDifference.rs"]
mod __NSOrderedCollectionDifference;
#[cfg(any())]
#[path = "NSOrderedSet.rs"]
mod __NSOrderedSet;
#[cfg(any())]
#[path = "NSOrthography.rs"]
mod __NSOrthography;
#[cfg(any())]
#[path = "NSPathUtilities.rs"]
mod __NSPathUtilities;
#[cfg(any())]
#[path = "NSPersonNameComponents.rs"]
mod __NSPersonNameComponents;
#[cfg(any())]
#[path = "NSPersonNameComponentsFormatter.rs"]
mod __NSPersonNameComponentsFormatter;
#[cfg(any())]
#[path = "NSPointerArray.rs"]
mod __NSPointerArray;
#[cfg(any())]
#[path = "NSPointerFunctions.rs"]
mod __NSPointerFunctions;
#[cfg(any())]
#[path = "NSPort.rs"]
mod __NSPort;
#[cfg(any())]
#[path = "NSPortCoder.rs"]
mod __NSPortCoder;
#[cfg(any())]
#[path = "NSPortMessage.rs"]
mod __NSPortMessage;
#[cfg(any())]
#[path = "NSPortNameServer.rs"]
mod __NSPortNameServer;
#[cfg(any())]
#[path = "NSPredicate.rs"]
mod __NSPredicate;
#[cfg(any())]
#[path = "NSProcessInfo.rs"]
mod __NSProcessInfo;
#[cfg(any())]
#[path = "NSProgress.rs"]
mod __NSProgress;
#[cfg(any())]
#[path = "NSPropertyList.rs"]
mod __NSPropertyList;
#[cfg(any())]
#[path = "NSProtocolChecker.rs"]
mod __NSProtocolChecker;
#[cfg(any())]
#[path = "NSProxy.rs"]
mod __NSProxy;
#[cfg(any())]
#[path = "NSRange.rs"]
mod __NSRange;
#[cfg(any())]
#[path = "NSRegularExpression.rs"]
mod __NSRegularExpression;
#[cfg(any())]
#[path = "NSRelativeDateTimeFormatter.rs"]
mod __NSRelativeDateTimeFormatter;
#[cfg(any())]
#[path = "NSRunLoop.rs"]
mod __NSRunLoop;
#[cfg(any())]
#[path = "NSScanner.rs"]
mod __NSScanner;
#[cfg(any())]
#[path = "NSScriptClassDescription.rs"]
mod __NSScriptClassDescription;
#[cfg(any())]
#[path = "NSScriptCoercionHandler.rs"]
mod __NSScriptCoercionHandler;
#[cfg(any())]
#[path = "NSScriptCommand.rs"]
mod __NSScriptCommand;
#[cfg(any())]
#[path = "NSScriptCommandDescription.rs"]
mod __NSScriptCommandDescription;
#[cfg(any())]
#[path = "NSScriptExecutionContext.rs"]
mod __NSScriptExecutionContext;
#[cfg(any())]
#[path = "NSScriptKeyValueCoding.rs"]
mod __NSScriptKeyValueCoding;
#[cfg(any())]
#[path = "NSScriptObjectSpecifiers.rs"]
mod __NSScriptObjectSpecifiers;
#[cfg(any())]
#[path = "NSScriptStandardSuiteCommands.rs"]
mod __NSScriptStandardSuiteCommands;
#[cfg(any())]
#[path = "NSScriptSuiteRegistry.rs"]
mod __NSScriptSuiteRegistry;
#[cfg(any())]
#[path = "NSScriptWhoseTests.rs"]
mod __NSScriptWhoseTests;
#[cfg(any())]
#[path = "NSSet.rs"]
mod __NSSet;
#[cfg(any())]
#[path = "NSSortDescriptor.rs"]
mod __NSSortDescriptor;
#[cfg(any())]
#[path = "NSSpellServer.rs"]
mod __NSSpellServer;
#[cfg(any())]
#[path = "NSStream.rs"]
mod __NSStream;
#[cfg(feature = "NSString")]
#[path = "NSString.rs"]
mod __NSString;
#[cfg(any())]
#[path = "NSTask.rs"]
mod __NSTask;
#[cfg(any())]
#[path = "NSTermOfAddress.rs"]
mod __NSTermOfAddress;
#[cfg(any())]
#[path = "NSTextCheckingResult.rs"]
mod __NSTextCheckingResult;
#[cfg(any())]
#[path = "NSThread.rs"]
mod __NSThread;
#[cfg(any())]
#[path = "NSTimeZone.rs"]
mod __NSTimeZone;
#[cfg(any())]
#[path = "NSTimer.rs"]
mod __NSTimer;
#[cfg(feature = "NSURL")]
#[path = "NSURL.rs"]
mod __NSURL;
#[cfg(any())]
#[path = "NSURLAuthenticationChallenge.rs"]
mod __NSURLAuthenticationChallenge;
#[cfg(any())]
#[path = "NSURLCache.rs"]
mod __NSURLCache;
#[cfg(any())]
#[path = "NSURLConnection.rs"]
mod __NSURLConnection;
#[cfg(any())]
#[path = "NSURLCredential.rs"]
mod __NSURLCredential;
#[cfg(any())]
#[path = "NSURLCredentialStorage.rs"]
mod __NSURLCredentialStorage;
#[cfg(any())]
#[path = "NSURLDownload.rs"]
mod __NSURLDownload;
#[cfg(any())]
#[path = "NSURLError.rs"]
mod __NSURLError;
#[cfg(any())]
#[path = "NSURLHandle.rs"]
mod __NSURLHandle;
#[cfg(any())]
#[path = "NSURLProtectionSpace.rs"]
mod __NSURLProtectionSpace;
#[cfg(any())]
#[path = "NSURLProtocol.rs"]
mod __NSURLProtocol;
#[cfg(any())]
#[path = "NSURLRequest.rs"]
mod __NSURLRequest;
#[cfg(any())]
#[path = "NSURLResponse.rs"]
mod __NSURLResponse;
#[cfg(any())]
#[path = "NSURLSession.rs"]
mod __NSURLSession;
#[cfg(feature = "NSUUID")]
#[path = "NSUUID.rs"]
mod __NSUUID;
#[cfg(any())]
#[path = "NSUbiquitousKeyValueStore.rs"]
mod __NSUbiquitousKeyValueStore;
#[cfg(any())]
#[path = "NSUndoManager.rs"]
mod __NSUndoManager;
#[cfg(any())]
#[path = "NSUnit.rs"]
mod __NSUnit;
#[cfg(any())]
#[path = "NSUserActivity.rs"]
mod __NSUserActivity;
#[cfg(any())]
#[path = "NSUserDefaults.rs"]
mod __NSUserDefaults;
#[cfg(any())]
#[path = "NSUserNotification.rs"]
mod __NSUserNotification;
#[cfg(any())]
#[path = "NSUserScriptTask.rs"]
mod __NSUserScriptTask;
#[cfg(feature = "NSValue")]
#[path = "NSValue.rs"]
mod __NSValue;
#[cfg(any())]
#[path = "NSValueTransformer.rs"]
mod __NSValueTransformer;
#[cfg(any())]
#[path = "NSXMLDTD.rs"]
mod __NSXMLDTD;
#[cfg(any())]
#[path = "NSXMLDTDNode.rs"]
mod __NSXMLDTDNode;
#[cfg(any())]
#[path = "NSXMLDocument.rs"]
mod __NSXMLDocument;
#[cfg(any())]
#[path = "NSXMLElement.rs"]
mod __NSXMLElement;
#[cfg(any())]
#[path = "NSXMLNode.rs"]
mod __NSXMLNode;
#[cfg(any())]
#[path = "NSXMLNodeOptions.rs"]
mod __NSXMLNodeOptions;
#[cfg(any())]
#[path = "NSXMLParser.rs"]
mod __NSXMLParser;
#[cfg(any())]
#[path = "NSXPCConnection.rs"]
mod __NSXPCConnection;
#[cfg(any())]
#[path = "NSZone.rs"]
mod __NSZone;
#[cfg(any())]
pub use self::__FoundationErrors::NSBundleErrorMaximum;
#[cfg(any())]
pub use self::__FoundationErrors::NSBundleErrorMinimum;
#[cfg(any())]
pub use self::__FoundationErrors::NSBundleOnDemandResourceExceededMaximumSizeError;
#[cfg(any())]
pub use self::__FoundationErrors::NSBundleOnDemandResourceInvalidTagError;
#[cfg(any())]
pub use self::__FoundationErrors::NSBundleOnDemandResourceOutOfSpaceError;
#[cfg(any())]
pub use self::__FoundationErrors::NSCloudSharingConflictError;
#[cfg(any())]
pub use self::__FoundationErrors::NSCloudSharingErrorMaximum;
#[cfg(any())]
pub use self::__FoundationErrors::NSCloudSharingErrorMinimum;
#[cfg(any())]
pub use self::__FoundationErrors::NSCloudSharingNetworkFailureError;
#[cfg(any())]
pub use self::__FoundationErrors::NSCloudSharingNoPermissionError;
#[cfg(any())]
pub use self::__FoundationErrors::NSCloudSharingOtherError;
#[cfg(any())]
pub use self::__FoundationErrors::NSCloudSharingQuotaExceededError;
#[cfg(any())]
pub use self::__FoundationErrors::NSCloudSharingTooManyParticipantsError;
#[cfg(any())]
pub use self::__FoundationErrors::NSCoderErrorMaximum;
#[cfg(any())]
pub use self::__FoundationErrors::NSCoderErrorMinimum;
#[cfg(any())]
pub use self::__FoundationErrors::NSCoderInvalidValueError;
#[cfg(any())]
pub use self::__FoundationErrors::NSCoderReadCorruptError;
#[cfg(any())]
pub use self::__FoundationErrors::NSCoderValueNotFoundError;
#[cfg(any())]
pub use self::__FoundationErrors::NSCompressionErrorMaximum;
#[cfg(any())]
pub use self::__FoundationErrors::NSCompressionErrorMinimum;
#[cfg(any())]
pub use self::__FoundationErrors::NSCompressionFailedError;
#[cfg(any())]
pub use self::__FoundationErrors::NSDecompressionFailedError;
#[cfg(any())]
pub use self::__FoundationErrors::NSExecutableArchitectureMismatchError;
#[cfg(any())]
pub use self::__FoundationErrors::NSExecutableErrorMaximum;
#[cfg(any())]
pub use self::__FoundationErrors::NSExecutableErrorMinimum;
#[cfg(any())]
pub use self::__FoundationErrors::NSExecutableLinkError;
#[cfg(any())]
pub use self::__FoundationErrors::NSExecutableLoadError;
#[cfg(any())]
pub use self::__FoundationErrors::NSExecutableNotLoadableError;
#[cfg(any())]
pub use self::__FoundationErrors::NSExecutableRuntimeMismatchError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFeatureUnsupportedError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileErrorMaximum;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileErrorMinimum;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileLockingError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileManagerUnmountBusyError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileManagerUnmountUnknownError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileNoSuchFileError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileReadCorruptFileError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileReadInapplicableStringEncodingError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileReadInvalidFileNameError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileReadNoPermissionError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileReadNoSuchFileError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileReadTooLargeError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileReadUnknownError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileReadUnknownStringEncodingError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileReadUnsupportedSchemeError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileWriteFileExistsError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileWriteInapplicableStringEncodingError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileWriteInvalidFileNameError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileWriteNoPermissionError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileWriteOutOfSpaceError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileWriteUnknownError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileWriteUnsupportedSchemeError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFileWriteVolumeReadOnlyError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFormattingError;
#[cfg(any())]
pub use self::__FoundationErrors::NSFormattingErrorMaximum;
#[cfg(any())]
pub use self::__FoundationErrors::NSFormattingErrorMinimum;
#[cfg(any())]
pub use self::__FoundationErrors::NSKeyValueValidationError;
#[cfg(any())]
pub use self::__FoundationErrors::NSPropertyListErrorMaximum;
#[cfg(any())]
pub use self::__FoundationErrors::NSPropertyListErrorMinimum;
#[cfg(any())]
pub use self::__FoundationErrors::NSPropertyListReadCorruptError;
#[cfg(any())]
pub use self::__FoundationErrors::NSPropertyListReadStreamError;
#[cfg(any())]
pub use self::__FoundationErrors::NSPropertyListReadUnknownVersionError;
#[cfg(any())]
pub use self::__FoundationErrors::NSPropertyListWriteInvalidError;
#[cfg(any())]
pub use self::__FoundationErrors::NSPropertyListWriteStreamError;
#[cfg(any())]
pub use self::__FoundationErrors::NSUbiquitousFileErrorMaximum;
#[cfg(any())]
pub use self::__FoundationErrors::NSUbiquitousFileErrorMinimum;
#[cfg(any())]
pub use self::__FoundationErrors::NSUbiquitousFileNotUploadedDueToQuotaError;
#[cfg(any())]
pub use self::__FoundationErrors::NSUbiquitousFileUbiquityServerNotAvailable;
#[cfg(any())]
pub use self::__FoundationErrors::NSUbiquitousFileUnavailableError;
#[cfg(any())]
pub use self::__FoundationErrors::NSUserActivityConnectionUnavailableError;
#[cfg(any())]
pub use self::__FoundationErrors::NSUserActivityErrorMaximum;
#[cfg(any())]
pub use self::__FoundationErrors::NSUserActivityErrorMinimum;
#[cfg(any())]
pub use self::__FoundationErrors::NSUserActivityHandoffFailedError;
#[cfg(any())]
pub use self::__FoundationErrors::NSUserActivityHandoffUserInfoTooLargeError;
#[cfg(any())]
pub use self::__FoundationErrors::NSUserActivityRemoteApplicationTimedOutError;
#[cfg(any())]
pub use self::__FoundationErrors::NSUserCancelledError;
#[cfg(any())]
pub use self::__FoundationErrors::NSValidationErrorMaximum;
#[cfg(any())]
pub use self::__FoundationErrors::NSValidationErrorMinimum;
#[cfg(any())]
pub use self::__FoundationErrors::NSXPCConnectionCodeSigningRequirementFailure;
#[cfg(any())]
pub use self::__FoundationErrors::NSXPCConnectionErrorMaximum;
#[cfg(any())]
pub use self::__FoundationErrors::NSXPCConnectionErrorMinimum;
#[cfg(any())]
pub use self::__FoundationErrors::NSXPCConnectionInterrupted;
#[cfg(any())]
pub use self::__FoundationErrors::NSXPCConnectionInvalid;
#[cfg(any())]
pub use self::__FoundationErrors::NSXPCConnectionReplyInvalid;
#[cfg(any())]
pub use self::__NSAffineTransform::NSAffineTransform;
#[cfg(all(any(), any()))]
pub use self::__NSAffineTransform::NSAffineTransformStruct;
#[cfg(any())]
pub use self::__NSAppleEventDescriptor::NSAppleEventDescriptor;
#[cfg(any())]
pub use self::__NSAppleEventDescriptor::NSAppleEventSendOptions;
#[cfg(any())]
pub(crate) use self::__NSAppleEventManager::__NSAppleEventManagerSuspension;
#[cfg(any())]
pub use self::__NSAppleEventManager::NSAppleEventManager;
#[cfg(any())]
pub use self::__NSAppleEventManager::NSAppleEventManagerSuspensionID;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSAppleEventManager::NSAppleEventManagerWillProcessFirstEventNotification;
#[cfg(any())]
pub use self::__NSAppleEventManager::NSAppleEventTimeOutDefault;
#[cfg(any())]
pub use self::__NSAppleEventManager::NSAppleEventTimeOutNone;
#[cfg(any())]
pub use self::__NSAppleScript::NSAppleScript;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAppleScript::NSAppleScriptErrorAppName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAppleScript::NSAppleScriptErrorBriefMessage;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAppleScript::NSAppleScriptErrorMessage;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAppleScript::NSAppleScriptErrorNumber;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAppleScript::NSAppleScriptErrorRange;
#[cfg(all(any(), any()))]
pub use self::__NSArchiver::NSArchiver;
#[cfg(any())]
pub use self::__NSArchiver::NSObjectNSArchiverCallback;
#[cfg(all(any(), any()))]
pub use self::__NSArchiver::NSUnarchiver;
#[cfg(feature = "NSArray")]
pub use self::__NSArray::NSArray;
#[cfg(feature = "NSArray")]
pub use self::__NSArray::NSBinarySearchingOptions;
#[cfg(feature = "NSArray")]
pub use self::__NSArray::NSMutableArray;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSAlternateDescriptionAttributeName;
#[cfg(any())]
pub use self::__NSAttributedString::NSAttributedString;
#[cfg(any())]
pub use self::__NSAttributedString::NSAttributedStringEnumerationOptions;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSAttributedStringFormattingContextKey;
#[cfg(any())]
pub use self::__NSAttributedString::NSAttributedStringFormattingOptions;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSAttributedStringKey;
#[cfg(any())]
pub use self::__NSAttributedString::NSAttributedStringMarkdownInterpretedSyntax;
#[cfg(any())]
pub use self::__NSAttributedString::NSAttributedStringMarkdownParsingFailurePolicy;
#[cfg(any())]
pub use self::__NSAttributedString::NSAttributedStringMarkdownParsingOptions;
#[cfg(any())]
pub use self::__NSAttributedString::NSAttributedStringMarkdownSourcePosition;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSImageURLAttributeName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSInflectionAgreementArgumentAttributeName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSInflectionAgreementConceptAttributeName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSInflectionAlternativeAttributeName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSInflectionConceptsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSInflectionReferentConceptAttributeName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSInflectionRuleAttributeName;
#[cfg(any())]
pub use self::__NSAttributedString::NSInlinePresentationIntent;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSInlinePresentationIntentAttributeName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSLanguageIdentifierAttributeName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSListItemDelimiterAttributeName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSLocalizedNumberFormatAttributeName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSMarkdownSourcePositionAttributeName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSMorphologyAttributeName;
#[cfg(any())]
pub use self::__NSAttributedString::NSMutableAttributedString;
#[cfg(any())]
pub use self::__NSAttributedString::NSPresentationIntent;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSPresentationIntentAttributeName;
#[cfg(any())]
pub use self::__NSAttributedString::NSPresentationIntentKind;
#[cfg(any())]
pub use self::__NSAttributedString::NSPresentationIntentTableColumnAlignment;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSAttributedString::NSReplacementIndexAttributeName;
#[cfg(any())]
pub use self::__NSAutoreleasePool::NSAutoreleasePool;
#[cfg(all(any(), any()))]
pub use self::__NSBackgroundActivityScheduler::NSBackgroundActivityCompletionHandler;
#[cfg(any())]
pub use self::__NSBackgroundActivityScheduler::NSBackgroundActivityResult;
#[cfg(any())]
pub use self::__NSBackgroundActivityScheduler::NSBackgroundActivityScheduler;
#[cfg(any())]
pub use self::__NSBundle::NSBundle;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSBundle::NSBundleDidLoadNotification;
#[cfg(any())]
pub use self::__NSBundle::NSBundleExecutableArchitectureARM64;
#[cfg(any())]
pub use self::__NSBundle::NSBundleExecutableArchitectureI386;
#[cfg(any())]
pub use self::__NSBundle::NSBundleExecutableArchitecturePPC;
#[cfg(any())]
pub use self::__NSBundle::NSBundleExecutableArchitecturePPC64;
#[cfg(any())]
pub use self::__NSBundle::NSBundleExecutableArchitectureX86_64;
#[cfg(any())]
pub use self::__NSBundle::NSBundleResourceRequest;
#[cfg(any())]
pub use self::__NSBundle::NSBundleResourceRequestLoadingPriorityUrgent;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSBundle::NSBundleResourceRequestLowDiskSpaceNotification;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSBundle::NSLoadedClasses;
#[cfg(all(any(), any()))]
pub use self::__NSByteCountFormatter::NSByteCountFormatter;
#[cfg(any())]
pub use self::__NSByteCountFormatter::NSByteCountFormatterCountStyle;
#[cfg(any())]
pub use self::__NSByteCountFormatter::NSByteCountFormatterUnits;
#[cfg(any())]
pub use self::__NSByteOrder::NSSwappedDouble;
#[cfg(any())]
pub use self::__NSByteOrder::NSSwappedFloat;
#[cfg(any())]
pub use self::__NSCache::NSCache;
#[cfg(any())]
pub use self::__NSCache::NSCacheDelegate;
#[cfg(any())]
pub use self::__NSCalendar::NSCalendar;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarDayChangedNotification;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifier;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierBangla;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierBuddhist;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierChinese;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierCoptic;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierDangi;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierEthiopicAmeteAlem;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierEthiopicAmeteMihret;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierGregorian;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierGujarati;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierHebrew;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierISO8601;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierIndian;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierIslamic;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierIslamicCivil;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierIslamicTabular;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierIslamicUmmAlQura;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierJapanese;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierKannada;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierMalayalam;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierMarathi;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierOdia;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierPersian;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierRepublicOfChina;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierTamil;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierTelugu;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierVietnamese;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSCalendar::NSCalendarIdentifierVikram;
#[cfg(any())]
pub use self::__NSCalendar::NSCalendarOptions;
#[cfg(any())]
pub use self::__NSCalendar::NSCalendarUnit;
#[cfg(any())]
pub use self::__NSCalendar::NSDateComponentUndefined;
#[cfg(any())]
pub use self::__NSCalendar::NSDateComponents;
#[cfg(any())]
pub use self::__NSCalendar::NSUndefinedDateComponent;
#[cfg(any())]
pub use self::__NSCalendar::NSWrapCalendarComponents;
#[cfg(all(any(), feature = "NSDate"))]
pub use self::__NSCalendarDate::NSCalendarDate;
#[cfg(any())]
pub use self::__NSCharacterSet::NSCharacterSet;
#[cfg(any())]
pub use self::__NSCharacterSet::NSMutableCharacterSet;
#[cfg(any())]
pub use self::__NSCharacterSet::NSOpenStepUnicodeReservedBase;
#[cfg(any())]
pub use self::__NSClassDescription::NSClassDescription;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSClassDescription::NSClassDescriptionNeededForClassNotification;
#[cfg(any())]
pub use self::__NSClassDescription::NSObjectNSClassDescriptionPrimitives;
#[cfg(any())]
pub use self::__NSCoder::NSCoder;
#[cfg(any())]
pub use self::__NSCoder::NSDecodingFailurePolicy;
#[cfg(any())]
pub use self::__NSCoder::NXReadNSObjectFromCoder;
#[cfg(all(any(), any()))]
pub use self::__NSComparisonPredicate::NSComparisonPredicate;
#[cfg(any())]
pub use self::__NSComparisonPredicate::NSComparisonPredicateModifier;
#[cfg(any())]
pub use self::__NSComparisonPredicate::NSComparisonPredicateOptions;
#[cfg(any())]
pub use self::__NSComparisonPredicate::NSPredicateOperatorType;
#[cfg(all(any(), any()))]
pub use self::__NSCompoundPredicate::NSCompoundPredicate;
#[cfg(any())]
pub use self::__NSCompoundPredicate::NSCompoundPredicateType;
#[cfg(any())]
pub use self::__NSConnection::NSConnection;
#[cfg(any())]
pub use self::__NSConnection::NSConnectionDelegate;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSConnection::NSConnectionDidDieNotification;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSConnection::NSConnectionDidInitializeNotification;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSConnection::NSConnectionReplyMode;
#[cfg(any())]
pub use self::__NSConnection::NSDistantObjectRequest;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSConnection::NSFailedAuthenticationException;
#[cfg(feature = "NSData")]
pub use self::__NSData::NSData;
#[cfg(feature = "NSData")]
pub use self::__NSData::NSDataBase64DecodingOptions;
#[cfg(feature = "NSData")]
pub use self::__NSData::NSDataBase64EncodingOptions;
#[cfg(feature = "NSData")]
pub use self::__NSData::NSDataCompressionAlgorithm;
#[cfg(feature = "NSData")]
pub use self::__NSData::NSDataReadingOptions;
#[cfg(feature = "NSData")]
pub use self::__NSData::NSDataSearchOptions;
#[cfg(feature = "NSData")]
pub use self::__NSData::NSDataWritingOptions;
#[cfg(feature = "NSData")]
pub use self::__NSData::NSMutableData;
#[cfg(feature = "NSData")]
pub use self::__NSData::NSPurgeableData;
#[cfg(feature = "NSDate")]
pub use self::__NSDate::NSDate;
#[cfg(feature = "NSDate")]
pub use self::__NSDate::NSTimeInterval;
#[cfg(all(any(), any()))]
pub use self::__NSDateComponentsFormatter::NSDateComponentsFormatter;
#[cfg(any())]
pub use self::__NSDateComponentsFormatter::NSDateComponentsFormatterUnitsStyle;
#[cfg(any())]
pub use self::__NSDateComponentsFormatter::NSDateComponentsFormatterZeroFormattingBehavior;
#[cfg(all(any(), any()))]
pub use self::__NSDateFormatter::NSDateFormatter;
#[cfg(any())]
pub use self::__NSDateFormatter::NSDateFormatterBehavior;
#[cfg(any())]
pub use self::__NSDateFormatter::NSDateFormatterStyle;
#[cfg(any())]
pub use self::__NSDateInterval::NSDateInterval;
#[cfg(all(any(), any()))]
pub use self::__NSDateIntervalFormatter::NSDateIntervalFormatter;
#[cfg(any())]
pub use self::__NSDateIntervalFormatter::NSDateIntervalFormatterStyle;
#[cfg(any())]
pub use self::__NSDebug::NSCountFrames;
#[cfg(any())]
pub use self::__NSDebug::NSDeallocateZombies;
#[cfg(any())]
pub use self::__NSDebug::NSDebugEnabled;
#[cfg(any())]
pub use self::__NSDebug::NSFrameAddress;
#[cfg(any())]
pub use self::__NSDebug::NSIsFreedObject;
#[cfg(any())]
pub use self::__NSDebug::NSKeepAllocationStatistics;
#[cfg(any())]
pub use self::__NSDebug::NSRecordAllocationEvent;
#[cfg(any())]
pub use self::__NSDebug::NSReturnAddress;
#[cfg(any())]
pub use self::__NSDebug::NSZombieEnabled;
#[cfg(any())]
pub use self::__NSDecimal::NSCalculationError;
#[cfg(any())]
pub use self::__NSDecimal::NSDecimalAdd;
#[cfg(any())]
pub use self::__NSDecimal::NSDecimalCompact;
#[cfg(all(any(), any()))]
pub use self::__NSDecimal::NSDecimalCompare;
#[cfg(any())]
pub use self::__NSDecimal::NSDecimalCopy;
#[cfg(any())]
pub use self::__NSDecimal::NSDecimalDivide;
#[cfg(any())]
pub use self::__NSDecimal::NSDecimalMultiply;
#[cfg(any())]
pub use self::__NSDecimal::NSDecimalMultiplyByPowerOf10;
#[cfg(any())]
pub use self::__NSDecimal::NSDecimalNormalize;
#[cfg(any())]
pub use self::__NSDecimal::NSDecimalPower;
#[cfg(any())]
pub use self::__NSDecimal::NSDecimalRound;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSDecimal::NSDecimalString;
#[cfg(any())]
pub use self::__NSDecimal::NSDecimalSubtract;
#[cfg(any())]
pub use self::__NSDecimal::NSRoundingMode;
#[cfg(all(any(), feature = "NSValue"))]
pub use self::__NSDecimalNumber::NSDecimalNumber;
#[cfg(any())]
pub use self::__NSDecimalNumber::NSDecimalNumberBehaviors;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSDecimalNumber::NSDecimalNumberDivideByZeroException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSDecimalNumber::NSDecimalNumberExactnessException;
#[cfg(any())]
pub use self::__NSDecimalNumber::NSDecimalNumberHandler;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSDecimalNumber::NSDecimalNumberOverflowException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSDecimalNumber::NSDecimalNumberUnderflowException;
#[cfg(feature = "NSDictionary")]
pub use self::__NSDictionary::NSDictionary;
#[cfg(feature = "NSDictionary")]
pub use self::__NSDictionary::NSMutableDictionary;
#[cfg(all(any(), any()))]
pub use self::__NSDistantObject::NSDistantObject;
#[cfg(any())]
pub use self::__NSDistributedLock::NSDistributedLock;
#[cfg(all(any(), any()))]
pub use self::__NSDistributedNotificationCenter::NSDistributedNotificationCenter;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSDistributedNotificationCenter::NSDistributedNotificationCenterType;
#[cfg(any())]
pub use self::__NSDistributedNotificationCenter::NSDistributedNotificationOptions;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSDistributedNotificationCenter::NSLocalNotificationCenterType;
#[cfg(any())]
pub use self::__NSDistributedNotificationCenter::NSNotificationDeliverImmediately;
#[cfg(any())]
pub use self::__NSDistributedNotificationCenter::NSNotificationPostToAllSessions;
#[cfg(any())]
pub use self::__NSDistributedNotificationCenter::NSNotificationSuspensionBehavior;
#[cfg(all(any(), any()))]
pub use self::__NSEnergyFormatter::NSEnergyFormatter;
#[cfg(any())]
pub use self::__NSEnergyFormatter::NSEnergyFormatterUnit;
#[cfg(any())]
pub use self::__NSEnumerator::NSEnumerator;
#[cfg(any())]
pub use self::__NSEnumerator::NSFastEnumeration;
#[cfg(feature = "NSError")]
pub use self::__NSError::NSError;
#[cfg(all(feature = "NSError", feature = "NSString"))]
pub use self::__NSError::NSErrorDomain;
#[cfg(all(feature = "NSError", feature = "NSString"))]
pub use self::__NSError::NSErrorUserInfoKey;
#[cfg(all(feature = "NSError", feature = "NSString"))]
pub use self::__NSError::NSLocalizedDescriptionKey;
#[cfg(feature = "NSError")]
pub use self::__NSError::NSObjectNSErrorRecoveryAttempting;
#[cfg(any())]
pub use self::__NSException::NSAssertionHandler;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSException::NSAssertionHandlerKey;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSDestinationInvalidException;
#[cfg(any())]
pub use self::__NSException::NSException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSGenericException;
#[cfg(any())]
pub use self::__NSException::NSGetUncaughtExceptionHandler;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSInconsistentArchiveException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSInternalInconsistencyException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSInvalidArgumentException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSInvalidReceivePortException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSInvalidSendPortException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSMallocException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSObjectInaccessibleException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSObjectNotAvailableException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSOldStyleException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSPortReceiveException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSPortSendException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSPortTimeoutException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSException::NSRangeException;
#[cfg(any())]
pub use self::__NSException::NSSetUncaughtExceptionHandler;
#[cfg(any())]
pub use self::__NSException::NSUncaughtExceptionHandler;
#[cfg(any())]
pub use self::__NSExpression::NSExpression;
#[cfg(any())]
pub use self::__NSExpression::NSExpressionType;
#[cfg(any())]
pub use self::__NSExtensionContext::NSExtensionContext;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSExtensionContext::NSExtensionHostDidBecomeActiveNotification;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSExtensionContext::NSExtensionHostDidEnterBackgroundNotification;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSExtensionContext::NSExtensionHostWillEnterForegroundNotification;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSExtensionContext::NSExtensionHostWillResignActiveNotification;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSExtensionContext::NSExtensionItemsAndErrorsKey;
#[cfg(any())]
pub use self::__NSExtensionItem::NSExtensionItem;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSExtensionItem::NSExtensionItemAttachmentsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSExtensionItem::NSExtensionItemAttributedContentTextKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSExtensionItem::NSExtensionItemAttributedTitleKey;
#[cfg(any())]
pub use self::__NSExtensionRequestHandling::NSExtensionRequestHandling;
#[cfg(any())]
pub use self::__NSFileCoordinator::NSFileAccessIntent;
#[cfg(any())]
pub use self::__NSFileCoordinator::NSFileCoordinator;
#[cfg(any())]
pub use self::__NSFileCoordinator::NSFileCoordinatorReadingOptions;
#[cfg(any())]
pub use self::__NSFileCoordinator::NSFileCoordinatorWritingOptions;
#[cfg(any())]
pub use self::__NSFileHandle::NSFileHandle;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSFileHandle::NSFileHandleConnectionAcceptedNotification;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSFileHandle::NSFileHandleDataAvailableNotification;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileHandle::NSFileHandleNotificationDataItem;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileHandle::NSFileHandleNotificationFileHandleItem;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileHandle::NSFileHandleNotificationMonitorModes;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSFileHandle::NSFileHandleOperationException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSFileHandle::NSFileHandleReadCompletionNotification;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSFileHandle::NSFileHandleReadToEndOfFileCompletionNotification;
#[cfg(any())]
pub use self::__NSFileHandle::NSPipe;
#[cfg(any())]
pub use self::__NSFileManager::NSDirectoryEnumerationOptions;
#[cfg(all(any(), any()))]
pub use self::__NSFileManager::NSDirectoryEnumerator;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileAppendOnly;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileAttributeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileAttributeType;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileBusy;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileCreationDate;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileDeviceIdentifier;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileExtensionHidden;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileGroupOwnerAccountID;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileGroupOwnerAccountName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileHFSCreatorCode;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileHFSTypeCode;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileImmutable;
#[cfg(any())]
pub use self::__NSFileManager::NSFileManager;
#[cfg(any())]
pub use self::__NSFileManager::NSFileManagerDelegate;
#[cfg(any())]
pub use self::__NSFileManager::NSFileManagerItemReplacementOptions;
#[cfg(any())]
pub use self::__NSFileManager::NSFileManagerResumeSyncBehavior;
#[cfg(any())]
pub use self::__NSFileManager::NSFileManagerSupportedSyncControls;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileManagerUnmountDissentingProcessIdentifierErrorKey;
#[cfg(any())]
pub use self::__NSFileManager::NSFileManagerUnmountOptions;
#[cfg(any())]
pub use self::__NSFileManager::NSFileManagerUploadLocalVersionConflictPolicy;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileModificationDate;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileOwnerAccountID;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileOwnerAccountName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFilePosixPermissions;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileProtectionComplete;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileProtectionCompleteUnlessOpen;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileProtectionCompleteUntilFirstUserAuthentication;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileProtectionCompleteWhenUserInactive;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileProtectionKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileProtectionNone;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileProtectionType;
#[cfg(any())]
pub use self::__NSFileManager::NSFileProviderService;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileProviderServiceName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileReferenceCount;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileSize;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileSystemFileNumber;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileSystemFreeNodes;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileSystemFreeSize;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileSystemNodes;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileSystemNumber;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileSystemSize;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileType;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileTypeBlockSpecial;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileTypeCharacterSpecial;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileTypeDirectory;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileTypeRegular;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileTypeSocket;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileTypeSymbolicLink;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSFileManager::NSFileTypeUnknown;
#[cfg(any())]
pub use self::__NSFileManager::NSFoundationVersionWithFileManagerResourceForkSupport;
#[cfg(any())]
pub use self::__NSFileManager::NSURLRelationship;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSFileManager::NSUbiquityIdentityDidChangeNotification;
#[cfg(any())]
pub use self::__NSFileManager::NSVolumeEnumerationOptions;
#[cfg(any())]
pub use self::__NSFilePresenter::NSFilePresenter;
#[cfg(any())]
pub use self::__NSFileVersion::NSFileVersion;
#[cfg(any())]
pub use self::__NSFileVersion::NSFileVersionAddingOptions;
#[cfg(any())]
pub use self::__NSFileVersion::NSFileVersionReplacingOptions;
#[cfg(any())]
pub use self::__NSFileWrapper::NSFileWrapper;
#[cfg(any())]
pub use self::__NSFileWrapper::NSFileWrapperReadingOptions;
#[cfg(any())]
pub use self::__NSFileWrapper::NSFileWrapperWritingOptions;
#[cfg(any())]
pub use self::__NSFormatter::NSFormatter;
#[cfg(any())]
pub use self::__NSFormatter::NSFormattingContext;
#[cfg(any())]
pub use self::__NSFormatter::NSFormattingUnitStyle;
#[cfg(any())]
pub use self::__NSGarbageCollector::NSGarbageCollector;
#[cfg(any())]
pub use self::__NSGeometry::NSAlignmentOptions;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSContainsRect;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSDivideRect;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSEdgeInsets;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSEdgeInsetsEqual;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSEdgeInsetsZero;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSEqualPoints;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSEqualRects;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSEqualSizes;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSInsetRect;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSIntegralRect;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSIntegralRectWithOptions;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSIntersectionRect;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSIntersectsRect;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSIsEmptyRect;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSMouseInRect;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSOffsetRect;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSPointArray;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSGeometry::NSPointFromString;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSPointInRect;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSPointPointer;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSRectArray;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSGeometry::NSRectFromString;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSRectPointer;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSSizeArray;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSGeometry::NSSizeFromString;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSSizePointer;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSGeometry::NSStringFromPoint;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSGeometry::NSStringFromRect;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSGeometry::NSStringFromSize;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSUnionRect;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSZeroPoint;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSZeroRect;
#[cfg(all(any(), any()))]
pub use self::__NSGeometry::NSZeroSize;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHFSFileTypes::NSFileTypeForHFSTypeCode;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHFSFileTypes::NSHFSTypeCodeFromFileType;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHFSFileTypes::NSHFSTypeOfFile;
#[cfg(any())]
pub use self::__NSHTTPCookie::NSHTTPCookie;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieComment;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieCommentURL;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieDiscard;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieDomain;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieExpires;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieMaximumAge;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieOriginURL;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookiePath;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookiePort;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookiePropertyKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieSameSiteLax;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieSameSitePolicy;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieSameSiteStrict;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieSecure;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieSetByJavaScript;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieStringPolicy;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieValue;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHTTPCookie::NSHTTPCookieVersion;
#[cfg(any())]
pub use self::__NSHTTPCookieStorage::NSHTTPCookieAcceptPolicy;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSHTTPCookieStorage::NSHTTPCookieManagerAcceptPolicyChangedNotification;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSHTTPCookieStorage::NSHTTPCookieManagerCookiesChangedNotification;
#[cfg(any())]
pub use self::__NSHTTPCookieStorage::NSHTTPCookieStorage;
#[cfg(all(feature = "NSArray", any()))]
pub use self::__NSHashTable::NSAllHashTableObjects;
#[cfg(any())]
pub use self::__NSHashTable::NSCompareHashTables;
#[cfg(all(any(), any()))]
pub use self::__NSHashTable::NSCopyHashTableWithZone;
#[cfg(any())]
pub use self::__NSHashTable::NSCountHashTable;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHashTable::NSCreateHashTable;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSHashTable::NSCreateHashTableWithZone;
#[cfg(any())]
pub use self::__NSHashTable::NSEndHashTableEnumeration;
#[cfg(any())]
pub use self::__NSHashTable::NSEnumerateHashTable;
#[cfg(any())]
pub use self::__NSHashTable::NSFreeHashTable;
#[cfg(any())]
pub use self::__NSHashTable::NSHashEnumerator;
#[cfg(any())]
pub use self::__NSHashTable::NSHashGet;
#[cfg(any())]
pub use self::__NSHashTable::NSHashInsert;
#[cfg(any())]
pub use self::__NSHashTable::NSHashInsertIfAbsent;
#[cfg(any())]
pub use self::__NSHashTable::NSHashInsertKnownAbsent;
#[cfg(any())]
pub use self::__NSHashTable::NSHashRemove;
#[cfg(any())]
pub use self::__NSHashTable::NSHashTable;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHashTable::NSHashTableCallBacks;
#[cfg(all(any(), any()))]
pub use self::__NSHashTable::NSHashTableCopyIn;
#[cfg(all(any(), any()))]
pub use self::__NSHashTable::NSHashTableObjectPointerPersonality;
#[cfg(any())]
pub use self::__NSHashTable::NSHashTableOptions;
#[cfg(all(any(), any()))]
pub use self::__NSHashTable::NSHashTableStrongMemory;
#[cfg(all(any(), any()))]
pub use self::__NSHashTable::NSHashTableWeakMemory;
#[cfg(all(any(), any()))]
pub use self::__NSHashTable::NSHashTableZeroingWeakMemory;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHashTable::NSIntHashCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHashTable::NSIntegerHashCallBacks;
#[cfg(any())]
pub use self::__NSHashTable::NSNextHashEnumeratorItem;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHashTable::NSNonOwnedPointerHashCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHashTable::NSNonRetainedObjectHashCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHashTable::NSObjectHashCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHashTable::NSOwnedObjectIdentityHashCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHashTable::NSOwnedPointerHashCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHashTable::NSPointerToStructHashCallBacks;
#[cfg(any())]
pub use self::__NSHashTable::NSResetHashTable;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSHashTable::NSStringFromHashTable;
#[cfg(any())]
pub use self::__NSHost::NSHost;
#[cfg(any())]
pub use self::__NSISO8601DateFormatter::NSISO8601DateFormatOptions;
#[cfg(all(any(), any()))]
pub use self::__NSISO8601DateFormatter::NSISO8601DateFormatter;
#[cfg(any())]
pub use self::__NSIndexPath::NSIndexPath;
#[cfg(any())]
pub use self::__NSIndexSet::NSIndexSet;
#[cfg(any())]
pub use self::__NSIndexSet::NSMutableIndexSet;
#[cfg(any())]
pub use self::__NSInflectionRule::NSInflectionRule;
#[cfg(any())]
pub use self::__NSInflectionRule::NSInflectionRuleExplicit;
#[cfg(any())]
pub use self::__NSInvocation::NSInvocation;
#[cfg(feature = "NSItemProvider")]
pub use self::__NSItemProvider::NSItemProvider;
#[cfg(all(
    feature = "NSError",
    feature = "NSItemProvider",
    feature = "NSObject",
    any()
))]
pub use self::__NSItemProvider::NSItemProviderCompletionHandler;
#[cfg(feature = "NSItemProvider")]
pub use self::__NSItemProvider::NSItemProviderErrorCode;
#[cfg(feature = "NSItemProvider")]
pub use self::__NSItemProvider::NSItemProviderFileOptions;
#[cfg(all(
    feature = "NSDictionary",
    feature = "NSError",
    feature = "NSItemProvider",
    feature = "NSObject",
    any()
))]
pub use self::__NSItemProvider::NSItemProviderLoadHandler;
#[cfg(feature = "NSItemProvider")]
pub use self::__NSItemProvider::NSItemProviderReading;
#[cfg(feature = "NSItemProvider")]
pub use self::__NSItemProvider::NSItemProviderRepresentationVisibility;
#[cfg(feature = "NSItemProvider")]
pub use self::__NSItemProvider::NSItemProviderWriting;
#[cfg(any())]
pub use self::__NSJSONSerialization::NSJSONReadingOptions;
#[cfg(any())]
pub use self::__NSJSONSerialization::NSJSONSerialization;
#[cfg(any())]
pub use self::__NSJSONSerialization::NSJSONWritingOptions;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueCoding::NSAverageKeyValueOperator;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueCoding::NSCountKeyValueOperator;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueCoding::NSDistinctUnionOfArraysKeyValueOperator;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueCoding::NSDistinctUnionOfObjectsKeyValueOperator;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueCoding::NSDistinctUnionOfSetsKeyValueOperator;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueCoding::NSKeyValueOperator;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueCoding::NSMaximumKeyValueOperator;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueCoding::NSMinimumKeyValueOperator;
#[cfg(any())]
pub use self::__NSKeyValueCoding::NSObjectNSKeyValueCoding;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueCoding::NSSumKeyValueOperator;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSKeyValueCoding::NSUndefinedKeyException;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueCoding::NSUnionOfArraysKeyValueOperator;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueCoding::NSUnionOfObjectsKeyValueOperator;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueCoding::NSUnionOfSetsKeyValueOperator;
#[cfg(any())]
pub use self::__NSKeyValueObserving::NSKeyValueChange;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueObserving::NSKeyValueChangeIndexesKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueObserving::NSKeyValueChangeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueObserving::NSKeyValueChangeKindKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueObserving::NSKeyValueChangeNewKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueObserving::NSKeyValueChangeNotificationIsPriorKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyValueObserving::NSKeyValueChangeOldKey;
#[cfg(any())]
pub use self::__NSKeyValueObserving::NSKeyValueObservingOptions;
#[cfg(any())]
pub use self::__NSKeyValueObserving::NSKeyValueSetMutationKind;
#[cfg(any())]
pub use self::__NSKeyValueObserving::NSObjectNSKeyValueObserverNotification;
#[cfg(any())]
pub use self::__NSKeyValueObserving::NSObjectNSKeyValueObserverRegistration;
#[cfg(any())]
pub use self::__NSKeyValueObserving::NSObjectNSKeyValueObserving;
#[cfg(any())]
pub use self::__NSKeyValueObserving::NSObjectNSKeyValueObservingCustomization;
#[cfg(any())]
pub use self::__NSKeyValueSharedObservers::NSKeyValueSharedObservers;
#[cfg(any())]
pub use self::__NSKeyValueSharedObservers::NSKeyValueSharedObserversSnapshot;
#[cfg(any())]
pub use self::__NSKeyValueSharedObservers::NSObjectNSKeyValueSharedObserverRegistration;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSKeyedArchiver::NSInvalidArchiveOperationException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSKeyedArchiver::NSInvalidUnarchiveOperationException;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSKeyedArchiver::NSKeyedArchiveRootObjectKey;
#[cfg(all(any(), any()))]
pub use self::__NSKeyedArchiver::NSKeyedArchiver;
#[cfg(any())]
pub use self::__NSKeyedArchiver::NSKeyedArchiverDelegate;
#[cfg(all(any(), any()))]
pub use self::__NSKeyedArchiver::NSKeyedUnarchiver;
#[cfg(any())]
pub use self::__NSKeyedArchiver::NSKeyedUnarchiverDelegate;
#[cfg(any())]
pub use self::__NSKeyedArchiver::NSObjectNSKeyedArchiverObjectSubstitution;
#[cfg(any())]
pub use self::__NSKeyedArchiver::NSObjectNSKeyedUnarchiverObjectSubstitution;
#[cfg(all(any(), any()))]
pub use self::__NSLengthFormatter::NSLengthFormatter;
#[cfg(any())]
pub use self::__NSLengthFormatter::NSLengthFormatterUnit;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTag;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagAdjective;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagAdverb;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagClassifier;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagCloseParenthesis;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagCloseQuote;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagConjunction;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagDash;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagDeterminer;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagIdiom;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagInterjection;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagNoun;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagNumber;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagOpenParenthesis;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagOpenQuote;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagOrganizationName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagOther;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagOtherPunctuation;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagOtherWhitespace;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagOtherWord;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagParagraphBreak;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagParticle;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagPersonalName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagPlaceName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagPreposition;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagPronoun;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagPunctuation;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagScheme;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagSchemeLanguage;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagSchemeLemma;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagSchemeLexicalClass;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagSchemeNameType;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagSchemeNameTypeOrLexicalClass;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagSchemeScript;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagSchemeTokenType;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagSentenceTerminator;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagVerb;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagWhitespace;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagWord;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSLinguisticTagger::NSLinguisticTagWordJoiner;
#[cfg(any())]
pub use self::__NSLinguisticTagger::NSLinguisticTagger;
#[cfg(any())]
pub use self::__NSLinguisticTagger::NSLinguisticTaggerOptions;
#[cfg(any())]
pub use self::__NSLinguisticTagger::NSLinguisticTaggerUnit;
#[cfg(all(any(), any()))]
pub use self::__NSListFormatter::NSListFormatter;
#[cfg(feature = "NSLocale")]
pub use self::__NSLocale::NSLocale;
#[cfg(all(feature = "NSLocale", feature = "NSString"))]
pub use self::__NSLocale::NSLocaleKey;
#[cfg(feature = "NSLocale")]
pub use self::__NSLocale::NSLocaleLanguageDirection;
#[cfg(any())]
pub use self::__NSLocalizedNumberFormatRule::NSLocalizedNumberFormatRule;
#[cfg(any())]
pub use self::__NSLock::NSCondition;
#[cfg(any())]
pub use self::__NSLock::NSConditionLock;
#[cfg(any())]
pub use self::__NSLock::NSLock;
#[cfg(any())]
pub use self::__NSLock::NSLocking;
#[cfg(any())]
pub use self::__NSLock::NSRecursiveLock;
#[cfg(all(feature = "NSArray", any()))]
pub use self::__NSMapTable::NSAllMapTableKeys;
#[cfg(all(feature = "NSArray", any()))]
pub use self::__NSMapTable::NSAllMapTableValues;
#[cfg(any())]
pub use self::__NSMapTable::NSCompareMapTables;
#[cfg(all(any(), any()))]
pub use self::__NSMapTable::NSCopyMapTableWithZone;
#[cfg(any())]
pub use self::__NSMapTable::NSCountMapTable;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSCreateMapTable;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSMapTable::NSCreateMapTableWithZone;
#[cfg(any())]
pub use self::__NSMapTable::NSEndMapTableEnumeration;
#[cfg(any())]
pub use self::__NSMapTable::NSEnumerateMapTable;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSIntMapKeyCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSIntMapValueCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSIntegerMapKeyCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSIntegerMapValueCallBacks;
#[cfg(any())]
pub use self::__NSMapTable::NSMapEnumerator;
#[cfg(any())]
pub use self::__NSMapTable::NSMapGet;
#[cfg(any())]
pub use self::__NSMapTable::NSMapInsert;
#[cfg(any())]
pub use self::__NSMapTable::NSMapInsertIfAbsent;
#[cfg(any())]
pub use self::__NSMapTable::NSMapInsertKnownAbsent;
#[cfg(any())]
pub use self::__NSMapTable::NSMapMember;
#[cfg(any())]
pub use self::__NSMapTable::NSMapRemove;
#[cfg(any())]
pub use self::__NSMapTable::NSMapTable;
#[cfg(all(any(), any()))]
pub use self::__NSMapTable::NSMapTableCopyIn;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSMapTableKeyCallBacks;
#[cfg(all(any(), any()))]
pub use self::__NSMapTable::NSMapTableObjectPointerPersonality;
#[cfg(any())]
pub use self::__NSMapTable::NSMapTableOptions;
#[cfg(all(any(), any()))]
pub use self::__NSMapTable::NSMapTableStrongMemory;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSMapTableValueCallBacks;
#[cfg(all(any(), any()))]
pub use self::__NSMapTable::NSMapTableWeakMemory;
#[cfg(all(any(), any()))]
pub use self::__NSMapTable::NSMapTableZeroingWeakMemory;
#[cfg(any())]
pub use self::__NSMapTable::NSNextMapEnumeratorPair;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSNonOwnedPointerMapKeyCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSNonOwnedPointerMapValueCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSNonOwnedPointerOrNullMapKeyCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSNonRetainedObjectMapKeyCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSNonRetainedObjectMapValueCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSObjectMapKeyCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSObjectMapValueCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSOwnedPointerMapKeyCallBacks;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSOwnedPointerMapValueCallBacks;
#[cfg(any())]
pub use self::__NSMapTable::NSResetMapTable;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMapTable::NSStringFromMapTable;
#[cfg(all(any(), any()))]
pub use self::__NSMassFormatter::NSMassFormatter;
#[cfg(any())]
pub use self::__NSMassFormatter::NSMassFormatterUnit;
#[cfg(all(any(), any()))]
pub use self::__NSMeasurement::NSMeasurement;
#[cfg(all(any(), any()))]
pub use self::__NSMeasurementFormatter::NSMeasurementFormatter;
#[cfg(any())]
pub use self::__NSMeasurementFormatter::NSMeasurementFormatterUnitOptions;
#[cfg(any())]
pub use self::__NSMetadata::NSMetadataItem;
#[cfg(any())]
pub use self::__NSMetadata::NSMetadataQuery;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryAccessibleUbiquitousExternalDocumentsScope;
#[cfg(any())]
pub use self::__NSMetadata::NSMetadataQueryAttributeValueTuple;
#[cfg(any())]
pub use self::__NSMetadata::NSMetadataQueryDelegate;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryDidFinishGatheringNotification;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryDidStartGatheringNotification;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryDidUpdateNotification;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryGatheringProgressNotification;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryIndexedLocalComputerScope;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryIndexedNetworkScope;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryLocalComputerScope;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryNetworkScope;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryResultContentRelevanceAttribute;
#[cfg(any())]
pub use self::__NSMetadata::NSMetadataQueryResultGroup;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryUbiquitousDataScope;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryUbiquitousDocumentsScope;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryUpdateAddedItemsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryUpdateChangedItemsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryUpdateRemovedItemsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadata::NSMetadataQueryUserHomeScope;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAcquisitionMakeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAcquisitionModelKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAlbumKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAltitudeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemApertureKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAppleLoopDescriptorsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAppleLoopsKeyFilterTypeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAppleLoopsLoopModeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAppleLoopsRootKeyKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemApplicationCategoriesKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAttributeChangeDateKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAudiencesKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAudioBitRateKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAudioChannelCountKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAudioEncodingApplicationKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAudioSampleRateKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAudioTrackNumberKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAuthorAddressesKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAuthorEmailAddressesKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemAuthorsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemBitsPerSampleKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemCFBundleIdentifierKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemCameraOwnerKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemCityKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemCodecsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemColorSpaceKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemCommentKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemComposerKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemContactKeywordsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemContentCreationDateKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemContentModificationDateKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemContentTypeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemContentTypeTreeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemContributorsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemCopyrightKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemCountryKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemCoverageKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemCreatorKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemDateAddedKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemDeliveryTypeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemDescriptionKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemDirectorKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemDisplayNameKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemDownloadedDateKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemDueDateKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemDurationSecondsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemEXIFGPSVersionKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemEXIFVersionKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemEditorsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemEmailAddressesKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemEncodingApplicationsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemExecutableArchitecturesKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemExecutablePlatformKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemExposureModeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemExposureProgramKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemExposureTimeSecondsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemExposureTimeStringKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemFNumberKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemFSContentChangeDateKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemFSCreationDateKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemFSNameKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemFSSizeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemFinderCommentKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemFlashOnOffKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemFocalLength35mmKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemFocalLengthKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemFontsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemGPSAreaInformationKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemGPSDOPKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemGPSDateStampKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemGPSDestBearingKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemGPSDestDistanceKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemGPSDestLatitudeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemGPSDestLongitudeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemGPSDifferentalKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemGPSMapDatumKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemGPSMeasureModeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemGPSProcessingMethodKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemGPSStatusKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemGPSTrackKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemGenreKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemHasAlphaChannelKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemHeadlineKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemISOSpeedKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemIdentifierKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemImageDirectionKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemInformationKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemInstantMessageAddressesKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemInstructionsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemIsApplicationManagedKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemIsGeneralMIDISequenceKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemIsLikelyJunkKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemIsUbiquitousKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemKeySignatureKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemKeywordsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemKindKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemLanguagesKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemLastUsedDateKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemLatitudeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemLayerNamesKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemLensModelKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemLongitudeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemLyricistKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemMaxApertureKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemMediaTypesKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemMeteringModeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemMusicalGenreKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemMusicalInstrumentCategoryKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemMusicalInstrumentNameKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemNamedLocationKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemNumberOfPagesKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemOrganizationsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemOrientationKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemOriginalFormatKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemOriginalSourceKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemPageHeightKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemPageWidthKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemParticipantsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemPathKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemPerformersKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemPhoneNumbersKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemPixelCountKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemPixelHeightKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemPixelWidthKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemProducerKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemProfileNameKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemProjectsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemPublishersKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemRecipientAddressesKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemRecipientEmailAddressesKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemRecipientsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemRecordingDateKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemRecordingYearKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemRedEyeOnOffKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemResolutionHeightDPIKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemResolutionWidthDPIKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemRightsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemSecurityMethodKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemSpeedKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemStarRatingKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemStateOrProvinceKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemStreamableKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemSubjectKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemTempoKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemTextContentKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemThemeKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemTimeSignatureKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemTimestampKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemTitleKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemTotalBitRateKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemURLKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemVersionKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemVideoBitRateKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemWhereFromsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataItemWhiteBalanceKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemContainerDisplayNameKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemDownloadRequestedKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemDownloadingErrorKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemDownloadingStatusCurrent;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemDownloadingStatusDownloaded;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemDownloadingStatusKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemDownloadingStatusNotDownloaded;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemHasUnresolvedConflictsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemIsDownloadedKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemIsDownloadingKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemIsExternalDocumentKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemIsSharedKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemIsUploadedKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemIsUploadingKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemPercentDownloadedKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemPercentUploadedKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemURLInLocalContainerKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousItemUploadingErrorKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousSharedItemCurrentUserPermissionsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousSharedItemCurrentUserRoleKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousSharedItemMostRecentEditorNameComponentsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousSharedItemOwnerNameComponentsKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousSharedItemPermissionsReadOnly;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousSharedItemPermissionsReadWrite;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousSharedItemRoleOwner;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSMetadataAttributes::NSMetadataUbiquitousSharedItemRoleParticipant;
#[cfg(any())]
pub use self::__NSMethodSignature::NSMethodSignature;
#[cfg(any())]
pub use self::__NSMorphology::NSGrammaticalCase;
#[cfg(any())]
pub use self::__NSMorphology::NSGrammaticalDefiniteness;
#[cfg(any())]
pub use self::__NSMorphology::NSGrammaticalDetermination;
#[cfg(any())]
pub use self::__NSMorphology::NSGrammaticalGender;
#[cfg(any())]
pub use self::__NSMorphology::NSGrammaticalNumber;
#[cfg(any())]
pub use self::__NSMorphology::NSGrammaticalPartOfSpeech;
#[cfg(any())]
pub use self::__NSMorphology::NSGrammaticalPerson;
#[cfg(any())]
pub use self::__NSMorphology::NSGrammaticalPronounType;
#[cfg(any())]
pub use self::__NSMorphology::NSMorphology;
#[cfg(any())]
pub use self::__NSMorphology::NSMorphologyCustomPronoun;
#[cfg(any())]
pub use self::__NSMorphology::NSMorphologyPronoun;
#[cfg(any())]
pub use self::__NSNetServices::NSNetService;
#[cfg(any())]
pub use self::__NSNetServices::NSNetServiceBrowser;
#[cfg(any())]
pub use self::__NSNetServices::NSNetServiceBrowserDelegate;
#[cfg(any())]
pub use self::__NSNetServices::NSNetServiceDelegate;
#[cfg(any())]
pub use self::__NSNetServices::NSNetServiceOptions;
#[cfg(any())]
pub use self::__NSNetServices::NSNetServicesError;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSNetServices::NSNetServicesErrorCode;
#[cfg(all(feature = "NSError", any(), feature = "NSString"))]
pub use self::__NSNetServices::NSNetServicesErrorDomain;
#[cfg(any())]
pub use self::__NSNotification::NSNotification;
#[cfg(any())]
pub use self::__NSNotification::NSNotificationCenter;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSNotification::NSNotificationName;
#[cfg(any())]
pub use self::__NSNotificationQueue::NSNotificationCoalescing;
#[cfg(any())]
pub use self::__NSNotificationQueue::NSNotificationQueue;
#[cfg(any())]
pub use self::__NSNotificationQueue::NSPostingStyle;
#[cfg(any())]
pub use self::__NSNull::NSNull;
#[cfg(all(any(), any()))]
pub use self::__NSNumberFormatter::NSNumberFormatter;
#[cfg(any())]
pub use self::__NSNumberFormatter::NSNumberFormatterBehavior;
#[cfg(any())]
pub use self::__NSNumberFormatter::NSNumberFormatterPadPosition;
#[cfg(any())]
pub use self::__NSNumberFormatter::NSNumberFormatterRoundingMode;
#[cfg(any())]
pub use self::__NSNumberFormatter::NSNumberFormatterStyle;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSObjCRuntime::NSClassFromString;
#[cfg(all(any(), any()))]
pub use self::__NSObjCRuntime::NSComparator;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSEnumerationOptions;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSObjCRuntime::NSExceptionName;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_0;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_1;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_10;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_10_1;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_10_2;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_10_3;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_10_4;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_10_5;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_10_Max;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_11;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_11_1;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_11_2;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_11_3;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_11_4;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_11_Max;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_1_1;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_1_2;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_1_3;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_1_4;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_2;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_2_1;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_2_2;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_2_3;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_2_4;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_2_5;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_2_6;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_2_7;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_2_8;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_3;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_3_1;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_3_2;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_3_3;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_3_4;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_3_5;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_3_6;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_3_7;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_3_8;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_3_9;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_4;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_4_1;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_4_10;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_4_11;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_4_2;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_4_3;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_4_4_Intel;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_4_4_PowerPC;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_4_5;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_4_6;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_4_7;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_4_8;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_4_9;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_5;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_5_1;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_5_2;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_5_3;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_5_4;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_5_5;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_5_6;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_5_7;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_5_8;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_6;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_6_1;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_6_2;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_6_3;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_6_4;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_6_5;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_6_6;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_6_7;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_6_8;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_7;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_7_1;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_7_2;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_7_3;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_7_4;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_8;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_8_1;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_8_2;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_8_3;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_8_4;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_9;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_9_1;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSFoundationVersionNumber10_9_2;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSGetSizeAndAlignment;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSNotFound;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSObjCRuntime::NSProtocolFromString;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSQualityOfService;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSObjCRuntime::NSRunLoopMode;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSObjCRuntime::NSSelectorFromString;
#[cfg(any())]
pub use self::__NSObjCRuntime::NSSortOptions;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSObjCRuntime::NSStringFromClass;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSObjCRuntime::NSStringFromProtocol;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSObjCRuntime::NSStringFromSelector;
#[cfg(all(feature = "NSObject", any()))]
pub use self::__NSObject::NSAllocateObject;
#[cfg(feature = "NSObject")]
pub use self::__NSObject::NSCoding;
#[cfg(feature = "NSObject")]
pub use self::__NSObject::NSDeallocateObject;
#[cfg(feature = "NSObject")]
pub use self::__NSObject::NSDecrementExtraRefCountWasZero;
#[cfg(feature = "NSObject")]
pub use self::__NSObject::NSDiscardableContent;
#[cfg(feature = "NSObject")]
pub use self::__NSObject::NSExtraRefCount;
#[cfg(feature = "NSObject")]
pub use self::__NSObject::NSIncrementExtraRefCount;
#[cfg(feature = "NSObject")]
pub use self::__NSObject::NSObjectNSCoderMethods;
#[cfg(feature = "NSObject")]
pub use self::__NSObject::NSObjectNSDiscardableContentProxy;
#[cfg(feature = "NSObject")]
pub use self::__NSObject::NSSecureCoding;
#[cfg(all(feature = "NSObject", any()))]
pub use self::__NSObject::NSShouldRetainWithZone;
#[cfg(any())]
pub use self::__NSObjectScripting::NSObjectNSScripting;
#[cfg(any())]
pub use self::__NSOperation::NSBlockOperation;
#[cfg(any())]
pub use self::__NSOperation::NSInvocationOperation;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSOperation::NSInvocationOperationCancelledException;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSOperation::NSInvocationOperationVoidResultException;
#[cfg(any())]
pub use self::__NSOperation::NSOperation;
#[cfg(any())]
pub use self::__NSOperation::NSOperationQueue;
#[cfg(any())]
pub use self::__NSOperation::NSOperationQueueDefaultMaxConcurrentOperationCount;
#[cfg(any())]
pub use self::__NSOperation::NSOperationQueuePriority;
#[cfg(any())]
pub use self::__NSOrderedCollectionChange::NSCollectionChangeType;
#[cfg(any())]
pub use self::__NSOrderedCollectionChange::NSOrderedCollectionChange;
#[cfg(any())]
pub use self::__NSOrderedCollectionDifference::NSOrderedCollectionDifference;
#[cfg(any())]
pub use self::__NSOrderedCollectionDifference::NSOrderedCollectionDifferenceCalculationOptions;
#[cfg(any())]
pub use self::__NSOrderedSet::NSMutableOrderedSet;
#[cfg(any())]
pub use self::__NSOrderedSet::NSOrderedSet;
#[cfg(any())]
pub use self::__NSOrthography::NSOrthography;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSPathUtilities::NSFullUserName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSPathUtilities::NSHomeDirectory;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSPathUtilities::NSHomeDirectoryForUser;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSPathUtilities::NSOpenStepRootDirectory;
#[cfg(any())]
pub use self::__NSPathUtilities::NSSearchPathDirectory;
#[cfg(any())]
pub use self::__NSPathUtilities::NSSearchPathDomainMask;
#[cfg(all(feature = "NSArray", any(), feature = "NSString"))]
pub use self::__NSPathUtilities::NSSearchPathForDirectoriesInDomains;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSPathUtilities::NSTemporaryDirectory;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSPathUtilities::NSUserName;
#[cfg(any())]
pub use self::__NSPersonNameComponents::NSPersonNameComponents;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSPersonNameComponentsFormatter::NSPersonNameComponentDelimiter;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSPersonNameComponentsFormatter::NSPersonNameComponentFamilyName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSPersonNameComponentsFormatter::NSPersonNameComponentGivenName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSPersonNameComponentsFormatter::NSPersonNameComponentKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSPersonNameComponentsFormatter::NSPersonNameComponentMiddleName;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSPersonNameComponentsFormatter::NSPersonNameComponentNickname;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSPersonNameComponentsFormatter::NSPersonNameComponentPrefix;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSPersonNameComponentsFormatter::NSPersonNameComponentSuffix;
#[cfg(all(any(), any()))]
pub use self::__NSPersonNameComponentsFormatter::NSPersonNameComponentsFormatter;
#[cfg(any())]
pub use self::__NSPersonNameComponentsFormatter::NSPersonNameComponentsFormatterOptions;
#[cfg(any())]
pub use self::__NSPersonNameComponentsFormatter::NSPersonNameComponentsFormatterStyle;
#[cfg(any())]
pub use self::__NSPointerArray::NSPointerArray;
#[cfg(any())]
pub use self::__NSPointerFunctions::NSPointerFunctions;
#[cfg(any())]
pub use self::__NSPointerFunctions::NSPointerFunctionsOptions;
#[cfg(any())]
pub use self::__NSPort::NSMachPort;
#[cfg(any())]
pub use self::__NSPort::NSMachPortDelegate;
#[cfg(any())]
pub use self::__NSPort::NSMachPortOptions;
#[cfg(any())]
pub use self::__NSPort::NSMessagePort;
#[cfg(any())]
pub use self::__NSPort::NSPort;
#[cfg(any())]
pub use self::__NSPort::NSPortDelegate;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSPort::NSPortDidBecomeInvalidNotification;
#[cfg(any())]
pub use self::__NSPort::NSSocketNativeHandle;
#[cfg(any())]
pub use self::__NSPort::NSSocketPort;
#[cfg(all(any(), any()))]
pub use self::__NSPortCoder::NSPortCoder;
#[cfg(any())]
pub use self::__NSPortMessage::NSPortMessage;
#[cfg(any())]
pub use self::__NSPortNameServer::NSMachBootstrapServer;
#[cfg(any())]
pub use self::__NSPortNameServer::NSMessagePortNameServer;
#[cfg(any())]
pub use self::__NSPortNameServer::NSPortNameServer;
#[cfg(any())]
pub use self::__NSPortNameServer::NSSocketPortNameServer;
#[cfg(any())]
pub use self::__NSPredicate::NSPredicate;
#[cfg(any())]
pub use self::__NSProcessInfo::NSActivityOptions;
#[cfg(any())]
pub use self::__NSProcessInfo::NSHPUXOperatingSystem;
#[cfg(any())]
pub use self::__NSProcessInfo::NSMACHOperatingSystem;
#[cfg(any())]
pub use self::__NSProcessInfo::NSOSF1OperatingSystem;
#[cfg(any())]
pub use self::__NSProcessInfo::NSOperatingSystemVersion;
#[cfg(any())]
pub use self::__NSProcessInfo::NSProcessInfo;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSProcessInfo::NSProcessInfoPowerStateDidChangeNotification;
#[cfg(any())]
pub use self::__NSProcessInfo::NSProcessInfoThermalState;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSProcessInfo::NSProcessInfoThermalStateDidChangeNotification;
#[cfg(any())]
pub use self::__NSProcessInfo::NSSolarisOperatingSystem;
#[cfg(any())]
pub use self::__NSProcessInfo::NSSunOSOperatingSystem;
#[cfg(any())]
pub use self::__NSProcessInfo::NSWindows95OperatingSystem;
#[cfg(any())]
pub use self::__NSProcessInfo::NSWindowsNTOperatingSystem;
#[cfg(any())]
pub use self::__NSProgress::NSProgress;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressEstimatedTimeRemainingKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressFileAnimationImageKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressFileAnimationImageOriginalRectKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressFileCompletedCountKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressFileIconKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressFileOperationKind;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressFileOperationKindCopying;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressFileOperationKindDecompressingAfterDownloading;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressFileOperationKindDownloading;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressFileOperationKindDuplicating;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressFileOperationKindKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressFileOperationKindReceiving;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressFileOperationKindUploading;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressFileTotalCountKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressFileURLKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressKind;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressKindFile;
#[cfg(all(any(), any()))]
pub use self::__NSProgress::NSProgressPublishingHandler;
#[cfg(any())]
pub use self::__NSProgress::NSProgressReporting;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressThroughputKey;
#[cfg(all(any(), any()))]
pub use self::__NSProgress::NSProgressUnpublishingHandler;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSProgress::NSProgressUserInfoKey;
#[cfg(any())]
pub use self::__NSPropertyList::NSPropertyListFormat;
#[cfg(any())]
pub use self::__NSPropertyList::NSPropertyListMutabilityOptions;
#[cfg(any())]
pub use self::__NSPropertyList::NSPropertyListReadOptions;
#[cfg(any())]
pub use self::__NSPropertyList::NSPropertyListSerialization;
#[cfg(any())]
pub use self::__NSPropertyList::NSPropertyListWriteOptions;
#[cfg(all(any(), any()))]
pub use self::__NSProtocolChecker::NSProtocolChecker;
#[cfg(any())]
pub use self::__NSRange::NSIntersectionRange;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSRange::NSRangeFromString;
#[cfg(any())]
pub use self::__NSRange::NSRangePointer;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSRange::NSStringFromRange;
#[cfg(any())]
pub use self::__NSRange::NSUnionRange;
#[cfg(any())]
pub use self::__NSRegularExpression::NSDataDetector;
#[cfg(any())]
pub use self::__NSRegularExpression::NSMatchingFlags;
#[cfg(any())]
pub use self::__NSRegularExpression::NSMatchingOptions;
#[cfg(any())]
pub use self::__NSRegularExpression::NSRegularExpression;
#[cfg(any())]
pub use self::__NSRegularExpression::NSRegularExpressionOptions;
#[cfg(all(any(), any()))]
pub use self::__NSRelativeDateTimeFormatter::NSRelativeDateTimeFormatter;
#[cfg(any())]
pub use self::__NSRelativeDateTimeFormatter::NSRelativeDateTimeFormatterStyle;
#[cfg(any())]
pub use self::__NSRelativeDateTimeFormatter::NSRelativeDateTimeFormatterUnitsStyle;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSRunLoop::NSDefaultRunLoopMode;
#[cfg(any())]
pub use self::__NSRunLoop::NSObjectNSDelayedPerforming;
#[cfg(any())]
pub use self::__NSRunLoop::NSRunLoop;
#[cfg(all(any(), any(), feature = "NSString"))]
pub use self::__NSRunLoop::NSRunLoopCommonModes;
#[cfg(any())]
pub use self::__NSScanner::NSScanner;
#[cfg(any())]
pub use self::__NSScriptClassDescription::NSObjectNSScriptClassDescription;
#[cfg(all(any(), any()))]
pub use self::__NSScriptClassDescription::NSScriptClassDescription;
#[cfg(any())]
pub use self::__NSScriptCoercionHandler::NSScriptCoercionHandler;
#[cfg(any())]
pub use self::__NSScriptCommand::NSArgumentEvaluationScriptError;
#[cfg(any())]
pub use self::__NSScriptCommand::NSArgumentsWrongScriptError;
#[cfg(any())]
pub use self::__NSScriptCommand::NSCannotCreateScriptCommandError;
#[cfg(any())]
pub use self::__NSScriptCommand::NSInternalScriptError;
#[cfg(any())]
pub use self::__NSScriptCommand::NSKeySpecifierEvaluationScriptError;
#[cfg(any())]
pub use self::__NSScriptCommand::NSNoScriptError;
#[cfg(any())]
pub use self::__NSScriptCommand::NSOperationNotSupportedForKeyScriptError;
#[cfg(any())]
pub use self::__NSScriptCommand::NSReceiverEvaluationScriptError;
#[cfg(any())]
pub use self::__NSScriptCommand::NSReceiversCantHandleCommandScriptError;
#[cfg(any())]
pub use self::__NSScriptCommand::NSRequiredArgumentsMissingScriptError;
#[cfg(any())]
pub use self::__NSScriptCommand::NSScriptCommand;
#[cfg(any())]
pub use self::__NSScriptCommand::NSUnknownKeyScriptError;
#[cfg(any())]
pub use self::__NSScriptCommandDescription::NSScriptCommandDescription;
#[cfg(any())]
pub use self::__NSScriptExecutionContext::NSScriptExecutionContext;
#[cfg(any())]
pub use self::__NSScriptKeyValueCoding::NSObjectNSScriptKeyValueCoding;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSScriptKeyValueCoding::NSOperationNotSupportedForKeyException;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSContainerSpecifierError;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSIndexSpecifier;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSInsertionPosition;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSInternalSpecifierError;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSInvalidIndexSpecifierError;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSMiddleSpecifier;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSNameSpecifier;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSNoSpecifierError;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSNoTopLevelContainersSpecifierError;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSObjectNSScriptObjectSpecifiers;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSOperationNotSupportedForKeySpecifierError;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSPositionalSpecifier;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSPropertySpecifier;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSRandomSpecifier;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSRangeSpecifier;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSRelativePosition;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSRelativeSpecifier;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSScriptObjectSpecifier;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSUniqueIDSpecifier;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSUnknownKeySpecifierError;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSWhoseSpecifier;
#[cfg(any())]
pub use self::__NSScriptObjectSpecifiers::NSWhoseSubelementIdentifier;
#[cfg(all(any(), any()))]
pub use self::__NSScriptStandardSuiteCommands::NSCloneCommand;
#[cfg(all(any(), any()))]
pub use self::__NSScriptStandardSuiteCommands::NSCloseCommand;
#[cfg(all(any(), any()))]
pub use self::__NSScriptStandardSuiteCommands::NSCountCommand;
#[cfg(all(any(), any()))]
pub use self::__NSScriptStandardSuiteCommands::NSCreateCommand;
#[cfg(all(any(), any()))]
pub use self::__NSScriptStandardSuiteCommands::NSDeleteCommand;
#[cfg(all(any(), any()))]
pub use self::__NSScriptStandardSuiteCommands::NSExistsCommand;
#[cfg(all(any(), any()))]
pub use self::__NSScriptStandardSuiteCommands::NSGetCommand;
#[cfg(all(any(), any()))]
pub use self::__NSScriptStandardSuiteCommands::NSMoveCommand;
#[cfg(all(any(), any()))]
pub use self::__NSScriptStandardSuiteCommands::NSQuitCommand;
#[cfg(any())]
pub use self::__NSScriptStandardSuiteCommands::NSSaveOptions;
#[cfg(all(any(), any()))]
pub use self::__NSScriptStandardSuiteCommands::NSSetCommand;
#[cfg(any())]
pub use self::__NSScriptSuiteRegistry::NSScriptSuiteRegistry;
#[cfg(any())]
pub use self::__NSScriptWhoseTests::NSLogicalTest;
#[cfg(any())]
pub use self::__NSScriptWhoseTests::NSObjectNSComparisonMethods;
#[cfg(any())]
pub use self::__NSScriptWhoseTests::NSObjectNSScriptingComparisonMethods;
#[cfg(any())]
pub use self::__NSScriptWhoseTests::NSScriptWhoseTest;
#[cfg(any())]
pub use self::__NSScriptWhoseTests::NSSpecifierTest;
#[cfg(any())]
pub use self::__NSScriptWhoseTests::NSTestComparisonOperation;
#[cfg(any())]
pub use self::__NSSet::NSCountedSet;
#[cfg(any())]
pub use self::__NSSet::NSMutableSet;
#[cfg(any())]
pub use self::__NSSet::NSSet;
#[cfg(any())]
pub use self::__NSSortDescriptor::NSSortDescriptor;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSSpellServer::NSGrammarCorrections;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSSpellServer::NSGrammarRange;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSSpellServer::NSGrammarUserDescription;
#[cfg(any())]
pub use self::__NSSpellServer::NSSpellServer;
#[cfg(any())]
pub use self::__NSSpellServer::NSSpellServerDelegate;
#[cfg(any())]
pub use self::__NSStream::NSInputStream;
#[cfg(any())]
pub use self::__NSStream::NSOutputStream;
#[cfg(any())]
pub use self::__NSStream::NSStream;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamDataWrittenToMemoryStreamKey;
#[cfg(any())]
pub use self::__NSStream::NSStreamDelegate;
#[cfg(any())]
pub use self::__NSStream::NSStreamEvent;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamFileCurrentOffsetKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamNetworkServiceType;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamNetworkServiceTypeBackground;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamNetworkServiceTypeCallSignaling;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamNetworkServiceTypeValue;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamNetworkServiceTypeVideo;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamNetworkServiceTypeVoIP;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamNetworkServiceTypeVoice;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamPropertyKey;
#[cfg(all(feature = "NSError", any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSOCKSErrorDomain;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSOCKSProxyConfiguration;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSOCKSProxyConfigurationKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSOCKSProxyHostKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSOCKSProxyPasswordKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSOCKSProxyPortKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSOCKSProxyUserKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSOCKSProxyVersion;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSOCKSProxyVersion4;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSOCKSProxyVersion5;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSOCKSProxyVersionKey;
#[cfg(all(feature = "NSError", any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSocketSSLErrorDomain;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSocketSecurityLevel;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSocketSecurityLevelKey;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSocketSecurityLevelNegotiatedSSL;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSocketSecurityLevelNone;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSocketSecurityLevelSSLv2;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSocketSecurityLevelSSLv3;
#[cfg(all(any(), feature = "NSString"))]
pub use self::__NSStream::NSStreamSocketSecurityLevelTLSv1;
#[cfg(any())]
pub use self::__NSStream::NSStreamStatus;
#[cfg(feature = "NSString")]
pub use self::__NSString::unichar;
#[cfg(feature = "NSString")]
pub use self::__NSString::NSConstantString;
#[cfg(feature = "NSString")]
pub use self::__NSString::NSMutableString;
#[cfg(feature = "NSString")]
pub use self::__NSString::NSSimpleCString;
#[cfg(feature = "NSString")]
pub use self::__NSString::NSString;
#[cfg(feature = "NSString")]
pub use self::__NSString::NSStringCompareOptions;
#[cfg(feature = "NSString")]
pub use self::__NSString::NSStringEncoding;
#[cfg(feature = "NSString")]
pub use self::__NSString::NSStringEncodingConversionOptions;
#[cfg(feature = "NSString")]
pub use self::__NSString::NSStringEncodingDetectionOptionsKey;
#[cfg(feature = "NSString")]
pub use self::__NSString::NSStringEnumerationOptions;
#[cfg(feature = "NSString")]
pub use self::__NSString::NSStringTransform;
#[cfg(any())]
pub use self::__NSTask::NSTask;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSTask::NSTaskDidTerminateNotification;
#[cfg(any())]
pub use self::__NSTask::NSTaskTerminationReason;
#[cfg(any())]
pub use self::__NSTermOfAddress::NSTermOfAddress;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSTextCheckingResult::NSTextCheckingAirlineKey;
#[cfg(any())]
pub use self::__NSTextCheckingResult::NSTextCheckingAllCustomTypes;
#[cfg(any())]
pub use self::__NSTextCheckingResult::NSTextCheckingAllSystemTypes;
#[cfg(any())]
pub use self::__NSTextCheckingResult::NSTextCheckingAllTypes;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSTextCheckingResult::NSTextCheckingCityKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSTextCheckingResult::NSTextCheckingCountryKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSTextCheckingResult::NSTextCheckingFlightKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSTextCheckingResult::NSTextCheckingJobTitleKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSTextCheckingResult::NSTextCheckingKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSTextCheckingResult::NSTextCheckingNameKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSTextCheckingResult::NSTextCheckingOrganizationKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSTextCheckingResult::NSTextCheckingPhoneKey;
#[cfg(any())]
pub use self::__NSTextCheckingResult::NSTextCheckingResult;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSTextCheckingResult::NSTextCheckingStateKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSTextCheckingResult::NSTextCheckingStreetKey;
#[cfg(any())]
pub use self::__NSTextCheckingResult::NSTextCheckingType;
#[cfg(any())]
pub use self::__NSTextCheckingResult::NSTextCheckingTypes;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSTextCheckingResult::NSTextCheckingZIPKey;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSThread::NSDidBecomeSingleThreadedNotification;
#[cfg(any())]
pub use self::__NSThread::NSObjectNSThreadPerformAdditions;
#[cfg(any())]
pub use self::__NSThread::NSThread;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSThread::NSThreadWillExitNotification;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSThread::NSWillBecomeMultiThreadedNotification;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSTimeZone::NSSystemTimeZoneDidChangeNotification;
#[cfg(any())]
pub use self::__NSTimeZone::NSTimeZone;
#[cfg(any())]
pub use self::__NSTimeZone::NSTimeZoneNameStyle;
#[cfg(any())]
pub use self::__NSTimer::NSTimer;
#[cfg(any())]
pub use self::__NSURLAuthenticationChallenge::NSURLAuthenticationChallenge;
#[cfg(any())]
pub use self::__NSURLAuthenticationChallenge::NSURLAuthenticationChallengeSender;
#[cfg(any())]
pub use self::__NSURLCache::NSCachedURLResponse;
#[cfg(any())]
pub use self::__NSURLCache::NSURLCache;
#[cfg(any())]
pub use self::__NSURLCache::NSURLCacheStoragePolicy;
#[cfg(any())]
pub use self::__NSURLConnection::NSURLConnection;
#[cfg(any())]
pub use self::__NSURLConnection::NSURLConnectionDataDelegate;
#[cfg(any())]
pub use self::__NSURLConnection::NSURLConnectionDelegate;
#[cfg(any())]
pub use self::__NSURLConnection::NSURLConnectionDownloadDelegate;
#[cfg(any())]
pub use self::__NSURLCredential::NSURLCredential;
#[cfg(any())]
pub use self::__NSURLCredential::NSURLCredentialPersistence;
#[cfg(any())]
pub use self::__NSURLCredentialStorage::NSURLCredentialStorage;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSURLCredentialStorage::NSURLCredentialStorageChangedNotification;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLCredentialStorage::NSURLCredentialStorageRemoveSynchronizableCredentials;
#[cfg(any())]
pub use self::__NSURLDownload::NSURLDownload;
#[cfg(any())]
pub use self::__NSURLDownload::NSURLDownloadDelegate;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLError::NSErrorFailingURLStringKey;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorAppTransportSecurityRequiresSecureConnection;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorBackgroundSessionInUseByAnotherProcess;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorBackgroundSessionRequiresSharedContainer;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorBackgroundSessionWasDisconnected;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLError::NSURLErrorBackgroundTaskCancelledReasonKey;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorBadServerResponse;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorBadURL;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCallIsActive;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCancelled;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCancelledReasonBackgroundUpdatesDisabled;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCancelledReasonInsufficientSystemResources;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCancelledReasonUserForceQuitApplication;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCannotCloseFile;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCannotConnectToHost;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCannotCreateFile;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCannotDecodeContentData;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCannotDecodeRawData;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCannotFindHost;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCannotLoadFromNetwork;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCannotMoveFile;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCannotOpenFile;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCannotParseResponse;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCannotRemoveFile;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorCannotWriteToFile;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorClientCertificateRejected;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorClientCertificateRequired;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorDNSLookupFailed;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorDataLengthExceedsMaximum;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorDataNotAllowed;
#[cfg(all(feature = "NSError", feature = "NSString", any()))]
pub use self::__NSURLError::NSURLErrorDomain;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorDownloadDecodingFailedMidStream;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorDownloadDecodingFailedToComplete;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLError::NSURLErrorFailingURLErrorKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLError::NSURLErrorFailingURLPeerTrustErrorKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLError::NSURLErrorFailingURLStringErrorKey;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorFileDoesNotExist;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorFileIsDirectory;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorFileOutsideSafeArea;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorHTTPTooManyRedirects;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorInternationalRoamingOff;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorNetworkConnectionLost;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorNetworkUnavailableReason;
#[cfg(all(feature = "NSError", feature = "NSString", any()))]
pub use self::__NSURLError::NSURLErrorNetworkUnavailableReasonKey;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorNoPermissionsToReadFile;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorNotConnectedToInternet;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorRedirectToNonExistentLocation;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorRequestBodyStreamExhausted;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorResourceUnavailable;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorSecureConnectionFailed;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorServerCertificateHasBadDate;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorServerCertificateHasUnknownRoot;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorServerCertificateNotYetValid;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorServerCertificateUntrusted;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorTimedOut;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorUnknown;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorUnsupportedURL;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorUserAuthenticationRequired;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorUserCancelledAuthentication;
#[cfg(any())]
pub use self::__NSURLError::NSURLErrorZeroByteResource;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLHandle::NSFTPPropertyActiveTransferModeKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLHandle::NSFTPPropertyFTPProxy;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLHandle::NSFTPPropertyFileOffsetKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLHandle::NSFTPPropertyUserLoginKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLHandle::NSFTPPropertyUserPasswordKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLHandle::NSHTTPPropertyErrorPageDataKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLHandle::NSHTTPPropertyHTTPProxy;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLHandle::NSHTTPPropertyRedirectionHeadersKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLHandle::NSHTTPPropertyServerHTTPVersionKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLHandle::NSHTTPPropertyStatusCodeKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLHandle::NSHTTPPropertyStatusReasonKey;
#[cfg(any())]
pub use self::__NSURLHandle::NSURLHandle;
#[cfg(any())]
pub use self::__NSURLHandle::NSURLHandleClient;
#[cfg(any())]
pub use self::__NSURLHandle::NSURLHandleStatus;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLAuthenticationMethodClientCertificate;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLAuthenticationMethodDefault;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLAuthenticationMethodHTMLForm;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLAuthenticationMethodHTTPBasic;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLAuthenticationMethodHTTPDigest;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLAuthenticationMethodNTLM;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLAuthenticationMethodNegotiate;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLAuthenticationMethodServerTrust;
#[cfg(any())]
pub use self::__NSURLProtectionSpace::NSURLProtectionSpace;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLProtectionSpaceFTP;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLProtectionSpaceFTPProxy;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLProtectionSpaceHTTP;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLProtectionSpaceHTTPProxy;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLProtectionSpaceHTTPS;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLProtectionSpaceHTTPSProxy;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLProtectionSpace::NSURLProtectionSpaceSOCKSProxy;
#[cfg(any())]
pub use self::__NSURLProtocol::NSURLProtocol;
#[cfg(any())]
pub use self::__NSURLProtocol::NSURLProtocolClient;
#[cfg(any())]
pub use self::__NSURLRequest::NSMutableURLRequest;
#[cfg(any())]
pub use self::__NSURLRequest::NSURLRequest;
#[cfg(any())]
pub use self::__NSURLRequest::NSURLRequestAttribution;
#[cfg(any())]
pub use self::__NSURLRequest::NSURLRequestCachePolicy;
#[cfg(any())]
pub use self::__NSURLRequest::NSURLRequestNetworkServiceType;
#[cfg(any())]
pub use self::__NSURLResponse::NSHTTPURLResponse;
#[cfg(any())]
pub use self::__NSURLResponse::NSURLResponse;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSession;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionAuthChallengeDisposition;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionConfiguration;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionDataDelegate;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionDataTask;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionDelayedRequestDisposition;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionDelegate;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionDownloadDelegate;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionDownloadTask;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLSession::NSURLSessionDownloadTaskResumeData;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionMultipathServiceType;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionResponseDisposition;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionStreamDelegate;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionStreamTask;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionTask;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionTaskDelegate;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionTaskMetrics;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionTaskMetricsDomainResolutionProtocol;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionTaskMetricsResourceFetchType;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionTaskPriorityDefault;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionTaskPriorityHigh;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionTaskPriorityLow;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionTaskState;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionTaskTransactionMetrics;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionTransferSizeUnknown;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionUploadTask;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSURLSession::NSURLSessionUploadTaskResumeData;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionWebSocketCloseCode;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionWebSocketDelegate;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionWebSocketMessage;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionWebSocketMessageType;
#[cfg(any())]
pub use self::__NSURLSession::NSURLSessionWebSocketTask;
#[cfg(any())]
pub use self::__NSUbiquitousKeyValueStore::NSUbiquitousKeyValueStore;
#[cfg(any())]
pub use self::__NSUbiquitousKeyValueStore::NSUbiquitousKeyValueStoreAccountChange;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUbiquitousKeyValueStore::NSUbiquitousKeyValueStoreChangeReasonKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUbiquitousKeyValueStore::NSUbiquitousKeyValueStoreChangedKeysKey;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSUbiquitousKeyValueStore::NSUbiquitousKeyValueStoreDidChangeExternallyNotification;
#[cfg(any())]
pub use self::__NSUbiquitousKeyValueStore::NSUbiquitousKeyValueStoreInitialSyncChange;
#[cfg(any())]
pub use self::__NSUbiquitousKeyValueStore::NSUbiquitousKeyValueStoreQuotaViolationChange;
#[cfg(any())]
pub use self::__NSUbiquitousKeyValueStore::NSUbiquitousKeyValueStoreServerChange;
#[cfg(any())]
pub use self::__NSUndoManager::NSUndoCloseGroupingRunLoopOrdering;
#[cfg(any())]
pub use self::__NSUndoManager::NSUndoManager;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSUndoManager::NSUndoManagerCheckpointNotification;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSUndoManager::NSUndoManagerDidCloseUndoGroupNotification;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSUndoManager::NSUndoManagerDidOpenUndoGroupNotification;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSUndoManager::NSUndoManagerDidRedoChangeNotification;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSUndoManager::NSUndoManagerDidUndoChangeNotification;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUndoManager::NSUndoManagerGroupIsDiscardableKey;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUndoManager::NSUndoManagerUserInfoKey;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSUndoManager::NSUndoManagerWillCloseUndoGroupNotification;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSUndoManager::NSUndoManagerWillRedoChangeNotification;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSUndoManager::NSUndoManagerWillUndoChangeNotification;
#[cfg(any())]
pub use self::__NSUnit::NSDimension;
#[cfg(any())]
pub use self::__NSUnit::NSUnit;
#[cfg(any())]
pub use self::__NSUnit::NSUnitAcceleration;
#[cfg(any())]
pub use self::__NSUnit::NSUnitAngle;
#[cfg(any())]
pub use self::__NSUnit::NSUnitArea;
#[cfg(any())]
pub use self::__NSUnit::NSUnitConcentrationMass;
#[cfg(any())]
pub use self::__NSUnit::NSUnitConverter;
#[cfg(any())]
pub use self::__NSUnit::NSUnitConverterLinear;
#[cfg(any())]
pub use self::__NSUnit::NSUnitDispersion;
#[cfg(any())]
pub use self::__NSUnit::NSUnitDuration;
#[cfg(any())]
pub use self::__NSUnit::NSUnitElectricCharge;
#[cfg(any())]
pub use self::__NSUnit::NSUnitElectricCurrent;
#[cfg(any())]
pub use self::__NSUnit::NSUnitElectricPotentialDifference;
#[cfg(any())]
pub use self::__NSUnit::NSUnitElectricResistance;
#[cfg(any())]
pub use self::__NSUnit::NSUnitEnergy;
#[cfg(any())]
pub use self::__NSUnit::NSUnitFrequency;
#[cfg(any())]
pub use self::__NSUnit::NSUnitFuelEfficiency;
#[cfg(any())]
pub use self::__NSUnit::NSUnitIlluminance;
#[cfg(any())]
pub use self::__NSUnit::NSUnitInformationStorage;
#[cfg(any())]
pub use self::__NSUnit::NSUnitLength;
#[cfg(any())]
pub use self::__NSUnit::NSUnitMass;
#[cfg(any())]
pub use self::__NSUnit::NSUnitPower;
#[cfg(any())]
pub use self::__NSUnit::NSUnitPressure;
#[cfg(any())]
pub use self::__NSUnit::NSUnitSpeed;
#[cfg(any())]
pub use self::__NSUnit::NSUnitTemperature;
#[cfg(any())]
pub use self::__NSUnit::NSUnitVolume;
#[cfg(any())]
pub use self::__NSUserActivity::NSUserActivity;
#[cfg(any())]
pub use self::__NSUserActivity::NSUserActivityDelegate;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserActivity::NSUserActivityPersistentIdentifier;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserActivity::NSUserActivityTypeBrowsingWeb;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSAMPMDesignation;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSArgumentDomain;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSCurrencySymbol;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSDateFormatString;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSDateTimeOrdering;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSDecimalDigits;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSDecimalSeparator;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSEarlierTimeDesignations;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSGlobalDomain;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSHourNameDesignations;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSInternationalCurrencyString;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSLaterTimeDesignations;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSMonthNameArray;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSNegativeCurrencyFormatString;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSNextDayDesignations;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSNextNextDayDesignations;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSPositiveCurrencyFormatString;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSPriorDayDesignations;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSRegistrationDomain;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSShortDateFormatString;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSShortMonthNameArray;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSShortTimeDateFormatString;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSShortWeekDayNameArray;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSThisDayDesignations;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSThousandsSeparator;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSTimeDateFormatString;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSTimeFormatString;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSUbiquitousUserDefaultsCompletedInitialSyncNotification;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSUbiquitousUserDefaultsDidChangeAccountsNotification;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSUbiquitousUserDefaultsNoCloudAccountNotification;
#[cfg(any())]
pub use self::__NSUserDefaults::NSUserDefaults;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSUserDefaultsDidChangeNotification;
#[cfg(all(any(), feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSUserDefaultsSizeLimitExceededNotification;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSWeekDayNameArray;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserDefaults::NSYearMonthWeekDesignations;
#[cfg(any())]
pub use self::__NSUserNotification::NSUserNotification;
#[cfg(any())]
pub use self::__NSUserNotification::NSUserNotificationAction;
#[cfg(any())]
pub use self::__NSUserNotification::NSUserNotificationActivationType;
#[cfg(any())]
pub use self::__NSUserNotification::NSUserNotificationCenter;
#[cfg(any())]
pub use self::__NSUserNotification::NSUserNotificationCenterDelegate;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSUserNotification::NSUserNotificationDefaultSoundName;
#[cfg(any())]
pub use self::__NSUserScriptTask::NSUserAppleScriptTask;
#[cfg(all(any(), feature = "NSError", any(), any()))]
pub use self::__NSUserScriptTask::NSUserAppleScriptTaskCompletionHandler;
#[cfg(any())]
pub use self::__NSUserScriptTask::NSUserAutomatorTask;
#[cfg(all(feature = "NSError", any(), any()))]
pub use self::__NSUserScriptTask::NSUserAutomatorTaskCompletionHandler;
#[cfg(any())]
pub use self::__NSUserScriptTask::NSUserScriptTask;
#[cfg(all(feature = "NSError", any(), any()))]
pub use self::__NSUserScriptTask::NSUserScriptTaskCompletionHandler;
#[cfg(any())]
pub use self::__NSUserScriptTask::NSUserUnixTask;
#[cfg(all(feature = "NSError", any(), any()))]
pub use self::__NSUserScriptTask::NSUserUnixTaskCompletionHandler;
#[cfg(feature = "NSValue")]
pub use self::__NSValue::NSNumber;
#[cfg(feature = "NSValue")]
pub use self::__NSValue::NSValue;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSValueTransformer::NSIsNilTransformerName;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSValueTransformer::NSIsNotNilTransformerName;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSValueTransformer::NSKeyedUnarchiveFromDataTransformerName;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSValueTransformer::NSNegateBooleanTransformerName;
#[cfg(any())]
pub use self::__NSValueTransformer::NSSecureUnarchiveFromDataTransformer;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSValueTransformer::NSSecureUnarchiveFromDataTransformerName;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSValueTransformer::NSUnarchiveFromDataTransformerName;
#[cfg(any())]
pub use self::__NSValueTransformer::NSValueTransformer;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSValueTransformer::NSValueTransformerName;
#[cfg(all(any(), any()))]
pub use self::__NSXMLDTDNode::NSXMLDTDNode;
#[cfg(any())]
pub use self::__NSXMLDTDNode::NSXMLDTDNodeKind;
#[cfg(all(any(), any()))]
pub use self::__NSXMLDocument::NSXMLDocument;
#[cfg(any())]
pub use self::__NSXMLDocument::NSXMLDocumentContentKind;
#[cfg(all(any(), any()))]
pub use self::__NSXMLElement::NSXMLElement;
#[cfg(any())]
pub use self::__NSXMLNode::NSXMLNode;
#[cfg(any())]
pub use self::__NSXMLNode::NSXMLNodeKind;
#[cfg(any())]
pub use self::__NSXMLNodeOptions::NSXMLNodeOptions;
#[cfg(any())]
pub use self::__NSXMLParser::NSXMLParser;
#[cfg(any())]
pub use self::__NSXMLParser::NSXMLParserDelegate;
#[cfg(any())]
pub use self::__NSXMLParser::NSXMLParserError;
#[cfg(all(feature = "NSError", feature = "NSString", any()))]
pub use self::__NSXMLParser::NSXMLParserErrorDomain;
#[cfg(any())]
pub use self::__NSXMLParser::NSXMLParserExternalEntityResolvingPolicy;
#[cfg(all(any(), any()))]
pub use self::__NSXPCConnection::NSXPCCoder;
#[cfg(any())]
pub use self::__NSXPCConnection::NSXPCConnection;
#[cfg(any())]
pub use self::__NSXPCConnection::NSXPCConnectionOptions;
#[cfg(any())]
pub use self::__NSXPCConnection::NSXPCInterface;
#[cfg(any())]
pub use self::__NSXPCConnection::NSXPCListener;
#[cfg(any())]
pub use self::__NSXPCConnection::NSXPCListenerDelegate;
#[cfg(any())]
pub use self::__NSXPCConnection::NSXPCListenerEndpoint;
#[cfg(any())]
pub use self::__NSXPCConnection::NSXPCProxyCreating;
#[cfg(any())]
pub use self::__NSZone::NSAllocateCollectable;
#[cfg(any())]
pub use self::__NSZone::NSAllocateMemoryPages;
#[cfg(any())]
pub use self::__NSZone::NSCollectorDisabledOption;
#[cfg(any())]
pub use self::__NSZone::NSCopyMemoryPages;
#[cfg(any())]
pub use self::__NSZone::NSCreateZone;
#[cfg(any())]
pub use self::__NSZone::NSDeallocateMemoryPages;
#[cfg(any())]
pub use self::__NSZone::NSDefaultMallocZone;
#[cfg(any())]
pub use self::__NSZone::NSLogPageSize;
#[cfg(any())]
pub use self::__NSZone::NSPageSize;
#[cfg(any())]
pub use self::__NSZone::NSRealMemoryAvailable;
#[cfg(any())]
pub use self::__NSZone::NSReallocateCollectable;
#[cfg(any())]
pub use self::__NSZone::NSRecycleZone;
#[cfg(any())]
pub use self::__NSZone::NSRoundDownToMultipleOfPageSize;
#[cfg(any())]
pub use self::__NSZone::NSRoundUpToMultipleOfPageSize;
#[cfg(any())]
pub use self::__NSZone::NSScannedOption;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSZone::NSSetZoneName;
#[cfg(any())]
pub use self::__NSZone::NSZoneCalloc;
#[cfg(any())]
pub use self::__NSZone::NSZoneFree;
#[cfg(any())]
pub use self::__NSZone::NSZoneFromPointer;
#[cfg(any())]
pub use self::__NSZone::NSZoneMalloc;
#[cfg(all(feature = "NSString", any()))]
pub use self::__NSZone::NSZoneName;
#[cfg(any())]
pub use self::__NSZone::NSZoneRealloc;
#[cfg(feature = "NSURL")]
pub use self::__NSURL::NSFileSecurity;
#[cfg(feature = "NSURL")]
pub use self::__NSURL::NSURLBookmarkCreationOptions;
#[cfg(feature = "NSURL")]
pub use self::__NSURL::NSURLBookmarkFileCreationOptions;
#[cfg(feature = "NSURL")]
pub use self::__NSURL::NSURLBookmarkResolutionOptions;
#[cfg(feature = "NSURL")]
pub use self::__NSURL::NSURLComponents;
#[cfg(all(feature = "NSString", feature = "NSURL"))]
pub use self::__NSURL::NSURLFileProtectionType;
#[cfg(all(feature = "NSString", feature = "NSURL"))]
pub use self::__NSURL::NSURLFileResourceType;
#[cfg(feature = "NSURL")]
pub use self::__NSURL::NSURLQueryItem;
#[cfg(all(feature = "NSString", feature = "NSURL"))]
pub use self::__NSURL::NSURLResourceKey;
#[cfg(all(feature = "NSString", feature = "NSURL"))]
pub use self::__NSURL::NSURLThumbnailDictionaryItem;
#[cfg(all(feature = "NSString", feature = "NSURL"))]
pub use self::__NSURL::NSURLUbiquitousItemDownloadingStatus;
#[cfg(all(feature = "NSString", feature = "NSURL"))]
pub use self::__NSURL::NSURLUbiquitousSharedItemPermissions;
#[cfg(all(feature = "NSString", feature = "NSURL"))]
pub use self::__NSURL::NSURLUbiquitousSharedItemRole;
#[cfg(feature = "NSURL")]
pub use self::__NSURL::NSURL;
#[cfg(feature = "NSUUID")]
pub use self::__NSUUID::NSUUID;
#[cfg(all(any(), any()))]
pub use self::__NSXMLDTD::NSXMLDTD;
