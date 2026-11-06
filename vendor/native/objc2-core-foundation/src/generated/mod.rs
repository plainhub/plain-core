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
#[cfg_attr(
    target_vendor = "apple",
    link(name = "CoreFoundation", kind = "framework")
)]
extern "C" {}
#[cfg(feature = "CFArray")]
#[path = "CFArray.rs"]
mod __CFArray;
#[cfg(any())]
#[path = "CFAttributedString.rs"]
mod __CFAttributedString;
#[cfg(any())]
#[path = "CFAvailability.rs"]
mod __CFAvailability;
#[cfg(any())]
#[path = "CFBag.rs"]
mod __CFBag;
#[cfg(any())]
#[path = "CFBinaryHeap.rs"]
mod __CFBinaryHeap;
#[cfg(any())]
#[path = "CFBitVector.rs"]
mod __CFBitVector;
#[cfg(any())]
#[path = "CFBundle.rs"]
mod __CFBundle;
#[cfg(any())]
#[path = "CFByteOrder.rs"]
mod __CFByteOrder;
#[cfg(feature = "CFCGTypes")]
#[path = "CFCGTypes.rs"]
mod __CFCGTypes;
#[cfg(any())]
#[path = "CFCalendar.rs"]
mod __CFCalendar;
#[cfg(any())]
#[path = "CFCharacterSet.rs"]
mod __CFCharacterSet;
#[cfg(feature = "CFData")]
#[path = "CFData.rs"]
mod __CFData;
#[cfg(feature = "CFDate")]
#[path = "CFDate.rs"]
mod __CFDate;
#[cfg(any())]
#[path = "CFDateFormatter.rs"]
mod __CFDateFormatter;
#[cfg(feature = "CFDictionary")]
#[path = "CFDictionary.rs"]
mod __CFDictionary;
#[cfg(any())]
#[path = "CFError.rs"]
mod __CFError;
#[cfg(any())]
#[path = "CFFileDescriptor.rs"]
mod __CFFileDescriptor;
#[cfg(any())]
#[path = "CFFileSecurity.rs"]
mod __CFFileSecurity;
#[cfg(any())]
#[path = "CFLocale.rs"]
mod __CFLocale;
#[cfg(any())]
#[path = "CFMachPort.rs"]
mod __CFMachPort;
#[cfg(any())]
#[path = "CFMessagePort.rs"]
mod __CFMessagePort;
#[cfg(any())]
#[path = "CFNotificationCenter.rs"]
mod __CFNotificationCenter;
#[cfg(any())]
#[path = "CFNumber.rs"]
mod __CFNumber;
#[cfg(any())]
#[path = "CFNumberFormatter.rs"]
mod __CFNumberFormatter;
#[cfg(any())]
#[path = "CFPlugIn.rs"]
mod __CFPlugIn;
#[cfg(any())]
#[path = "CFPlugInCOM.rs"]
mod __CFPlugInCOM;
#[cfg(any())]
#[path = "CFPreferences.rs"]
mod __CFPreferences;
#[cfg(any())]
#[path = "CFPropertyList.rs"]
mod __CFPropertyList;
#[cfg(feature = "CFRunLoop")]
#[path = "CFRunLoop.rs"]
mod __CFRunLoop;
#[cfg(any())]
#[path = "CFSet.rs"]
mod __CFSet;
#[cfg(any())]
#[path = "CFSocket.rs"]
mod __CFSocket;
#[cfg(any())]
#[path = "CFStream.rs"]
mod __CFStream;
#[cfg(feature = "CFString")]
#[path = "CFString.rs"]
mod __CFString;
#[cfg(any())]
#[path = "CFStringEncodingExt.rs"]
mod __CFStringEncodingExt;
#[cfg(any())]
#[path = "CFStringTokenizer.rs"]
mod __CFStringTokenizer;
#[cfg(any())]
#[path = "CFTimeZone.rs"]
mod __CFTimeZone;
#[cfg(any())]
#[path = "CFTree.rs"]
mod __CFTree;
#[cfg(any())]
#[path = "CFURL.rs"]
mod __CFURL;
#[cfg(any())]
#[path = "CFURLAccess.rs"]
mod __CFURLAccess;
#[cfg(any())]
#[path = "CFURLEnumerator.rs"]
mod __CFURLEnumerator;
#[cfg(any())]
#[path = "CFUUID.rs"]
mod __CFUUID;
#[cfg(any())]
#[path = "CFUserNotification.rs"]
mod __CFUserNotification;
#[cfg(any())]
#[path = "CFUtilities.rs"]
mod __CFUtilities;
#[cfg(any())]
#[path = "CFXMLNode.rs"]
mod __CFXMLNode;
#[cfg(any())]
#[path = "CFXMLParser.rs"]
mod __CFXMLParser;
#[cfg(feature = "CFArray")]
pub use self::__CFArray::kCFTypeArrayCallBacks;
#[cfg(feature = "CFArray")]
pub use self::__CFArray::CFArray;
#[cfg(feature = "CFArray")]
pub use self::__CFArray::CFArrayApplierFunction;
#[cfg(feature = "CFArray")]
pub use self::__CFArray::CFArrayCallBacks;
#[cfg(feature = "CFArray")]
pub use self::__CFArray::CFArrayCopyDescriptionCallBack;
#[cfg(feature = "CFArray")]
pub use self::__CFArray::CFArrayEqualCallBack;
#[cfg(feature = "CFArray")]
pub use self::__CFArray::CFArrayReleaseCallBack;
#[cfg(feature = "CFArray")]
pub use self::__CFArray::CFArrayRetainCallBack;
#[cfg(feature = "CFArray")]
pub use self::__CFArray::CFMutableArray;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedString;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringBeginEditing;
#[cfg(all(any(), feature = "CFDictionary"))]
pub use self::__CFAttributedString::CFAttributedStringCreate;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringCreateCopy;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringCreateMutable;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringCreateMutableCopy;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringCreateWithSubstring;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringEndEditing;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringGetAttribute;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringGetAttributeAndLongestEffectiveRange;
#[cfg(all(any(), feature = "CFDictionary"))]
pub use self::__CFAttributedString::CFAttributedStringGetAttributes;
#[cfg(all(any(), feature = "CFDictionary"))]
pub use self::__CFAttributedString::CFAttributedStringGetAttributesAndLongestEffectiveRange;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringGetBidiLevelsAndResolvedDirections;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringGetLength;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringGetMutableString;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringGetStatisticalWritingDirections;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringGetString;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringRemoveAttribute;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringReplaceAttributedString;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringReplaceString;
#[cfg(any())]
pub use self::__CFAttributedString::CFAttributedStringSetAttribute;
#[cfg(all(any(), feature = "CFDictionary"))]
pub use self::__CFAttributedString::CFAttributedStringSetAttributes;
#[cfg(any())]
pub use self::__CFAttributedString::CFMutableAttributedString;
#[cfg(any())]
pub use self::__CFBag::kCFCopyStringBagCallBacks;
#[cfg(any())]
pub use self::__CFBag::kCFTypeBagCallBacks;
#[cfg(any())]
pub use self::__CFBag::CFBag;
#[cfg(any())]
pub use self::__CFBag::CFBagAddValue;
#[cfg(any())]
pub use self::__CFBag::CFBagApplierFunction;
#[cfg(any())]
pub use self::__CFBag::CFBagApplyFunction;
#[cfg(any())]
pub use self::__CFBag::CFBagCallBacks;
#[cfg(any())]
pub use self::__CFBag::CFBagContainsValue;
#[cfg(any())]
pub use self::__CFBag::CFBagCopyDescriptionCallBack;
#[cfg(any())]
pub use self::__CFBag::CFBagCreate;
#[cfg(any())]
pub use self::__CFBag::CFBagCreateCopy;
#[cfg(any())]
pub use self::__CFBag::CFBagCreateMutable;
#[cfg(any())]
pub use self::__CFBag::CFBagCreateMutableCopy;
#[cfg(any())]
pub use self::__CFBag::CFBagEqualCallBack;
#[cfg(any())]
pub use self::__CFBag::CFBagGetCount;
#[cfg(any())]
pub use self::__CFBag::CFBagGetCountOfValue;
#[cfg(any())]
pub use self::__CFBag::CFBagGetValue;
#[cfg(any())]
pub use self::__CFBag::CFBagGetValueIfPresent;
#[cfg(any())]
pub use self::__CFBag::CFBagGetValues;
#[cfg(any())]
pub use self::__CFBag::CFBagHashCallBack;
#[cfg(any())]
pub use self::__CFBag::CFBagReleaseCallBack;
#[cfg(any())]
pub use self::__CFBag::CFBagRemoveAllValues;
#[cfg(any())]
pub use self::__CFBag::CFBagRemoveValue;
#[cfg(any())]
pub use self::__CFBag::CFBagReplaceValue;
#[cfg(any())]
pub use self::__CFBag::CFBagRetainCallBack;
#[cfg(any())]
pub use self::__CFBag::CFBagSetValue;
#[cfg(any())]
pub use self::__CFBag::CFMutableBag;
#[cfg(any())]
pub use self::__CFBinaryHeap::kCFStringBinaryHeapCallBacks;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeap;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapAddValue;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapApplierFunction;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapApplyFunction;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapCallBacks;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapCompareContext;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapContainsValue;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapCreate;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapCreateCopy;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapGetCount;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapGetCountOfValue;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapGetMinimum;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapGetMinimumIfPresent;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapGetValues;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapRemoveAllValues;
#[cfg(any())]
pub use self::__CFBinaryHeap::CFBinaryHeapRemoveMinimumValue;
#[cfg(any())]
pub use self::__CFBitVector::CFBit;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVector;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorContainsBit;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorCreate;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorCreateCopy;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorCreateMutable;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorCreateMutableCopy;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorFlipBitAtIndex;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorFlipBits;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorGetBitAtIndex;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorGetBits;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorGetCount;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorGetCountOfBit;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorGetFirstIndexOfBit;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorGetLastIndexOfBit;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorSetAllBits;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorSetBitAtIndex;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorSetBits;
#[cfg(any())]
pub use self::__CFBitVector::CFBitVectorSetCount;
#[cfg(any())]
pub use self::__CFBitVector::CFMutableBitVector;
#[cfg(any())]
pub use self::__CFBundle::kCFBundleDevelopmentRegionKey;
#[cfg(any())]
pub use self::__CFBundle::kCFBundleExecutableArchitectureARM64;
#[cfg(any())]
pub use self::__CFBundle::kCFBundleExecutableArchitectureI386;
#[cfg(any())]
pub use self::__CFBundle::kCFBundleExecutableArchitecturePPC;
#[cfg(any())]
pub use self::__CFBundle::kCFBundleExecutableArchitecturePPC64;
#[cfg(any())]
pub use self::__CFBundle::kCFBundleExecutableArchitectureX86_64;
#[cfg(any())]
pub use self::__CFBundle::kCFBundleExecutableKey;
#[cfg(any())]
pub use self::__CFBundle::kCFBundleIdentifierKey;
#[cfg(any())]
pub use self::__CFBundle::kCFBundleInfoDictionaryVersionKey;
#[cfg(any())]
pub use self::__CFBundle::kCFBundleLocalizationsKey;
#[cfg(any())]
pub use self::__CFBundle::kCFBundleNameKey;
#[cfg(any())]
pub use self::__CFBundle::kCFBundleVersionKey;
#[cfg(any())]
pub use self::__CFBundle::CFBundle;
#[cfg(any())]
pub use self::__CFBundle::CFBundleCloseBundleResourceMap;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleCopyAuxiliaryExecutableURL;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleCopyBuiltInPlugInsURL;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFBundle::CFBundleCopyBundleLocalizations;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleCopyBundleURL;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFBundle::CFBundleCopyExecutableArchitectures;
#[cfg(all(feature = "CFArray", any(), any()))]
pub use self::__CFBundle::CFBundleCopyExecutableArchitecturesForURL;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleCopyExecutableURL;
#[cfg(all(any(), feature = "CFDictionary", any()))]
pub use self::__CFBundle::CFBundleCopyInfoDictionaryForURL;
#[cfg(all(any(), feature = "CFDictionary", any()))]
pub use self::__CFBundle::CFBundleCopyInfoDictionaryInDirectory;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFBundle::CFBundleCopyLocalizationsForPreferences;
#[cfg(all(feature = "CFArray", any(), any()))]
pub use self::__CFBundle::CFBundleCopyLocalizationsForURL;
#[cfg(any())]
pub use self::__CFBundle::CFBundleCopyLocalizedString;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFBundle::CFBundleCopyLocalizedStringForLocalizations;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFBundle::CFBundleCopyPreferredLocalizationsFromArray;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleCopyPrivateFrameworksURL;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleCopyResourceURL;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleCopyResourceURLForLocalization;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleCopyResourceURLInDirectory;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFBundle::CFBundleCopyResourceURLsOfType;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFBundle::CFBundleCopyResourceURLsOfTypeForLocalization;
#[cfg(all(feature = "CFArray", any(), any()))]
pub use self::__CFBundle::CFBundleCopyResourceURLsOfTypeInDirectory;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleCopyResourcesDirectoryURL;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleCopySharedFrameworksURL;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleCopySharedSupportURL;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleCopySupportFilesDirectoryURL;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleCreate;
#[cfg(all(feature = "CFArray", any(), any()))]
pub use self::__CFBundle::CFBundleCreateBundlesFromDirectory;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFBundle::CFBundleGetAllBundles;
#[cfg(any())]
pub use self::__CFBundle::CFBundleGetBundleWithIdentifier;
#[cfg(any())]
pub use self::__CFBundle::CFBundleGetDataPointerForName;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFBundle::CFBundleGetDataPointersForNames;
#[cfg(any())]
pub use self::__CFBundle::CFBundleGetDevelopmentRegion;
#[cfg(any())]
pub use self::__CFBundle::CFBundleGetFunctionPointerForName;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFBundle::CFBundleGetFunctionPointersForNames;
#[cfg(any())]
pub use self::__CFBundle::CFBundleGetIdentifier;
#[cfg(all(any(), feature = "CFDictionary"))]
pub use self::__CFBundle::CFBundleGetInfoDictionary;
#[cfg(all(any(), feature = "CFDictionary"))]
pub use self::__CFBundle::CFBundleGetLocalInfoDictionary;
#[cfg(any())]
pub use self::__CFBundle::CFBundleGetMainBundle;
#[cfg(any())]
pub use self::__CFBundle::CFBundleGetPackageInfo;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleGetPackageInfoInDirectory;
#[cfg(any())]
pub use self::__CFBundle::CFBundleGetPlugIn;
#[cfg(any())]
pub use self::__CFBundle::CFBundleGetValueForInfoDictionaryKey;
#[cfg(any())]
pub use self::__CFBundle::CFBundleGetVersionNumber;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleIsArchitectureLoadable;
#[cfg(any())]
pub use self::__CFBundle::CFBundleIsExecutableLoadable;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleIsExecutableLoadableForURL;
#[cfg(any())]
pub use self::__CFBundle::CFBundleIsExecutableLoaded;
#[cfg(any())]
pub use self::__CFBundle::CFBundleLoadExecutable;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundleLoadExecutableAndReturnError;
#[cfg(any())]
pub use self::__CFBundle::CFBundleOpenBundleResourceFiles;
#[cfg(any())]
pub use self::__CFBundle::CFBundleOpenBundleResourceMap;
#[cfg(all(any(), any()))]
pub use self::__CFBundle::CFBundlePreflightExecutable;
#[cfg(any())]
pub use self::__CFBundle::CFBundleUnloadExecutable;
#[cfg(any())]
pub use self::__CFBundle::CFPlugIn;
#[cfg(any())]
pub use self::__CFByteOrder::CFByteOrder;
#[cfg(any())]
pub use self::__CFByteOrder::CFSwappedFloat32;
#[cfg(any())]
pub use self::__CFByteOrder::CFSwappedFloat64;
#[cfg(feature = "CFCGTypes")]
pub use self::__CFCGTypes::CGAffineTransform;
#[cfg(feature = "CFCGTypes")]
pub use self::__CFCGTypes::CGAffineTransformComponents;
#[cfg(feature = "CFCGTypes")]
pub use self::__CFCGTypes::CGRectEdge;
#[cfg(feature = "CFCGTypes")]
pub use self::__CFCGTypes::CGVector;
#[cfg(any())]
pub use self::__CFCalendar::kCFCalendarComponentsWrap;
#[cfg(any())]
pub use self::__CFCalendar::CFCalendar;
#[cfg(any())]
pub use self::__CFCalendar::CFCalendarCopyCurrent;
#[cfg(all(any(), any()))]
pub use self::__CFCalendar::CFCalendarCopyLocale;
#[cfg(all(any(), feature = "CFDate"))]
pub use self::__CFCalendar::CFCalendarCopyTimeZone;
#[cfg(all(any(), any()))]
pub use self::__CFCalendar::CFCalendarCreateWithIdentifier;
#[cfg(any())]
pub use self::__CFCalendar::CFCalendarGetFirstWeekday;
#[cfg(all(any(), any()))]
pub use self::__CFCalendar::CFCalendarGetIdentifier;
#[cfg(any())]
pub use self::__CFCalendar::CFCalendarGetMaximumRangeOfUnit;
#[cfg(any())]
pub use self::__CFCalendar::CFCalendarGetMinimumDaysInFirstWeek;
#[cfg(any())]
pub use self::__CFCalendar::CFCalendarGetMinimumRangeOfUnit;
#[cfg(all(any(), feature = "CFDate"))]
pub use self::__CFCalendar::CFCalendarGetOrdinalityOfUnit;
#[cfg(all(any(), feature = "CFDate"))]
pub use self::__CFCalendar::CFCalendarGetRangeOfUnit;
#[cfg(all(any(), feature = "CFDate"))]
pub use self::__CFCalendar::CFCalendarGetTimeRangeOfUnit;
#[cfg(any())]
pub use self::__CFCalendar::CFCalendarSetFirstWeekday;
#[cfg(all(any(), any()))]
pub use self::__CFCalendar::CFCalendarSetLocale;
#[cfg(any())]
pub use self::__CFCalendar::CFCalendarSetMinimumDaysInFirstWeek;
#[cfg(all(any(), feature = "CFDate"))]
pub use self::__CFCalendar::CFCalendarSetTimeZone;
#[cfg(any())]
pub use self::__CFCalendar::CFCalendarUnit;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSet;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetAddCharactersInRange;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetAddCharactersInString;
#[cfg(all(any(), feature = "CFData"))]
pub use self::__CFCharacterSet::CFCharacterSetCreateBitmapRepresentation;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetCreateCopy;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetCreateInvertedSet;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetCreateMutable;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetCreateMutableCopy;
#[cfg(all(any(), feature = "CFData"))]
pub use self::__CFCharacterSet::CFCharacterSetCreateWithBitmapRepresentation;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetCreateWithCharactersInRange;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetCreateWithCharactersInString;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetGetPredefined;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetHasMemberInPlane;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetIntersect;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetInvert;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetIsCharacterMember;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetIsLongCharacterMember;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetIsSupersetOfSet;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetPredefinedSet;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetRemoveCharactersInRange;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetRemoveCharactersInString;
#[cfg(any())]
pub use self::__CFCharacterSet::CFCharacterSetUnion;
#[cfg(any())]
pub use self::__CFCharacterSet::CFMutableCharacterSet;
#[cfg(feature = "CFData")]
pub use self::__CFData::CFData;
#[cfg(feature = "CFData")]
pub use self::__CFData::CFDataSearchFlags;
#[cfg(feature = "CFData")]
pub use self::__CFData::CFMutableData;
#[cfg(feature = "CFDate")]
pub use self::__CFDate::kCFAbsoluteTimeIntervalSince1970;
#[cfg(feature = "CFDate")]
pub use self::__CFDate::CFAbsoluteTime;
#[cfg(feature = "CFDate")]
pub use self::__CFDate::CFAbsoluteTimeGetCurrent;
#[cfg(feature = "CFDate")]
pub use self::__CFDate::CFDate;
#[cfg(feature = "CFDate")]
pub use self::__CFDate::CFGregorianDate;
#[cfg(feature = "CFDate")]
pub use self::__CFDate::CFGregorianUnitFlags;
#[cfg(feature = "CFDate")]
pub use self::__CFDate::CFGregorianUnits;
#[cfg(feature = "CFDate")]
pub use self::__CFDate::CFTimeInterval;
#[cfg(feature = "CFDate")]
pub use self::__CFDate::CFTimeZone;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterAMSymbol;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterCalendar;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterCalendarName;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterDefaultDate;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterDefaultFormat;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterDoesRelativeDateFormattingKey;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterEraSymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterGregorianStartDate;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterIsLenient;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterLongEraSymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterMonthSymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterPMSymbol;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterQuarterSymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterShortMonthSymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterShortQuarterSymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterShortStandaloneMonthSymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterShortStandaloneQuarterSymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterShortStandaloneWeekdaySymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterShortWeekdaySymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterStandaloneMonthSymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterStandaloneQuarterSymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterStandaloneWeekdaySymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterTimeZone;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterTwoDigitStartDate;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterVeryShortMonthSymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterVeryShortStandaloneMonthSymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterVeryShortStandaloneWeekdaySymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterVeryShortWeekdaySymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::kCFDateFormatterWeekdaySymbols;
#[cfg(any())]
pub use self::__CFDateFormatter::CFDateFormatter;
#[cfg(any())]
pub use self::__CFDateFormatter::CFDateFormatterCopyProperty;
#[cfg(all(any(), any()))]
pub use self::__CFDateFormatter::CFDateFormatterCreate;
#[cfg(all(any(), any()))]
pub use self::__CFDateFormatter::CFDateFormatterCreateDateFormatFromTemplate;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFDateFormatter::CFDateFormatterCreateDateFromString;
#[cfg(any())]
pub use self::__CFDateFormatter::CFDateFormatterCreateISO8601Formatter;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFDateFormatter::CFDateFormatterCreateStringWithAbsoluteTime;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFDateFormatter::CFDateFormatterCreateStringWithDate;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFDateFormatter::CFDateFormatterGetAbsoluteTimeFromString;
#[cfg(any())]
pub use self::__CFDateFormatter::CFDateFormatterGetDateStyle;
#[cfg(any())]
pub use self::__CFDateFormatter::CFDateFormatterGetFormat;
#[cfg(all(any(), any()))]
pub use self::__CFDateFormatter::CFDateFormatterGetLocale;
#[cfg(any())]
pub use self::__CFDateFormatter::CFDateFormatterGetTimeStyle;
#[cfg(any())]
pub use self::__CFDateFormatter::CFDateFormatterKey;
#[cfg(any())]
pub use self::__CFDateFormatter::CFDateFormatterSetFormat;
#[cfg(any())]
pub use self::__CFDateFormatter::CFDateFormatterSetProperty;
#[cfg(any())]
pub use self::__CFDateFormatter::CFDateFormatterStyle;
#[cfg(any())]
pub use self::__CFDateFormatter::CFISO8601DateFormatOptions;
#[cfg(feature = "CFDictionary")]
pub use self::__CFDictionary::kCFTypeDictionaryKeyCallBacks;
#[cfg(feature = "CFDictionary")]
pub use self::__CFDictionary::kCFTypeDictionaryValueCallBacks;
#[cfg(feature = "CFDictionary")]
pub use self::__CFDictionary::CFDictionary;
#[cfg(feature = "CFDictionary")]
pub use self::__CFDictionary::CFDictionaryApplierFunction;
#[cfg(feature = "CFDictionary")]
pub use self::__CFDictionary::CFDictionaryCopyDescriptionCallBack;
#[cfg(feature = "CFDictionary")]
pub use self::__CFDictionary::CFDictionaryEqualCallBack;
#[cfg(feature = "CFDictionary")]
pub use self::__CFDictionary::CFDictionaryHashCallBack;
#[cfg(feature = "CFDictionary")]
pub use self::__CFDictionary::CFDictionaryKeyCallBacks;
#[cfg(feature = "CFDictionary")]
pub use self::__CFDictionary::CFDictionaryReleaseCallBack;
#[cfg(feature = "CFDictionary")]
pub use self::__CFDictionary::CFDictionaryRetainCallBack;
#[cfg(feature = "CFDictionary")]
pub use self::__CFDictionary::CFDictionaryValueCallBacks;
#[cfg(feature = "CFDictionary")]
pub use self::__CFDictionary::CFMutableDictionary;
#[cfg(any())]
pub use self::__CFError::kCFErrorDescriptionKey;
#[cfg(any())]
pub use self::__CFError::kCFErrorDomainCocoa;
#[cfg(any())]
pub use self::__CFError::kCFErrorDomainMach;
#[cfg(any())]
pub use self::__CFError::kCFErrorDomainOSStatus;
#[cfg(any())]
pub use self::__CFError::kCFErrorDomainPOSIX;
#[cfg(any())]
pub use self::__CFError::kCFErrorFilePathKey;
#[cfg(any())]
pub use self::__CFError::kCFErrorLocalizedDescriptionKey;
#[cfg(any())]
pub use self::__CFError::kCFErrorLocalizedFailureKey;
#[cfg(any())]
pub use self::__CFError::kCFErrorLocalizedFailureReasonKey;
#[cfg(any())]
pub use self::__CFError::kCFErrorLocalizedRecoverySuggestionKey;
#[cfg(any())]
pub use self::__CFError::kCFErrorURLKey;
#[cfg(any())]
pub use self::__CFError::kCFErrorUnderlyingErrorKey;
#[cfg(any())]
pub use self::__CFError::CFError;
#[cfg(any())]
pub use self::__CFError::CFErrorCopyDescription;
#[cfg(any())]
pub use self::__CFError::CFErrorCopyFailureReason;
#[cfg(any())]
pub use self::__CFError::CFErrorCopyRecoverySuggestion;
#[cfg(all(feature = "CFDictionary", any()))]
pub use self::__CFError::CFErrorCopyUserInfo;
#[cfg(all(feature = "CFDictionary", any()))]
pub use self::__CFError::CFErrorCreate;
#[cfg(any())]
pub use self::__CFError::CFErrorCreateWithUserInfoKeysAndValues;
#[cfg(any())]
pub use self::__CFError::CFErrorDomain;
#[cfg(any())]
pub use self::__CFError::CFErrorGetCode;
#[cfg(any())]
pub use self::__CFError::CFErrorGetDomain;
#[cfg(any())]
pub use self::__CFFileDescriptor::kCFFileDescriptorReadCallBack;
#[cfg(any())]
pub use self::__CFFileDescriptor::kCFFileDescriptorWriteCallBack;
#[cfg(any())]
pub use self::__CFFileDescriptor::CFFileDescriptor;
#[cfg(any())]
pub use self::__CFFileDescriptor::CFFileDescriptorCallBack;
#[cfg(any())]
pub use self::__CFFileDescriptor::CFFileDescriptorContext;
#[cfg(any())]
pub use self::__CFFileDescriptor::CFFileDescriptorCreate;
#[cfg(all(any(), feature = "CFRunLoop"))]
pub use self::__CFFileDescriptor::CFFileDescriptorCreateRunLoopSource;
#[cfg(any())]
pub use self::__CFFileDescriptor::CFFileDescriptorDisableCallBacks;
#[cfg(any())]
pub use self::__CFFileDescriptor::CFFileDescriptorEnableCallBacks;
#[cfg(any())]
pub use self::__CFFileDescriptor::CFFileDescriptorGetContext;
#[cfg(any())]
pub use self::__CFFileDescriptor::CFFileDescriptorGetNativeDescriptor;
#[cfg(any())]
pub use self::__CFFileDescriptor::CFFileDescriptorInvalidate;
#[cfg(any())]
pub use self::__CFFileDescriptor::CFFileDescriptorIsValid;
#[cfg(any())]
pub use self::__CFFileDescriptor::CFFileDescriptorNativeDescriptor;
#[cfg(any())]
pub use self::__CFFileSecurity::CFFileSecurity;
#[cfg(any())]
pub use self::__CFFileSecurity::CFFileSecurityClearOptions;
#[cfg(any())]
pub use self::__CFFileSecurity::CFFileSecurityClearProperties;
#[cfg(all(any(), any()))]
pub use self::__CFFileSecurity::CFFileSecurityCopyGroupUUID;
#[cfg(all(any(), any()))]
pub use self::__CFFileSecurity::CFFileSecurityCopyOwnerUUID;
#[cfg(any())]
pub use self::__CFFileSecurity::CFFileSecurityCreate;
#[cfg(any())]
pub use self::__CFFileSecurity::CFFileSecurityCreateCopy;
#[cfg(all(any(), any()))]
pub use self::__CFFileSecurity::CFFileSecurityGetGroup;
#[cfg(all(any(), any()))]
pub use self::__CFFileSecurity::CFFileSecurityGetMode;
#[cfg(all(any(), any()))]
pub use self::__CFFileSecurity::CFFileSecurityGetOwner;
#[cfg(all(any(), any()))]
pub use self::__CFFileSecurity::CFFileSecuritySetGroup;
#[cfg(all(any(), any()))]
pub use self::__CFFileSecurity::CFFileSecuritySetGroupUUID;
#[cfg(all(any(), any()))]
pub use self::__CFFileSecurity::CFFileSecuritySetMode;
#[cfg(all(any(), any()))]
pub use self::__CFFileSecurity::CFFileSecuritySetOwner;
#[cfg(all(any(), any()))]
pub use self::__CFFileSecurity::CFFileSecuritySetOwnerUUID;
#[cfg(any())]
pub use self::__CFLocale::kCFBanglaCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFBuddhistCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFChineseCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFDangiCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFGregorianCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFGujaratiCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFHebrewCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFISO8601Calendar;
#[cfg(any())]
pub use self::__CFLocale::kCFIndianCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFIslamicCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFIslamicCivilCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFIslamicTabularCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFIslamicUmmAlQuraCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFJapaneseCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFKannadaCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleAlternateQuotationBeginDelimiterKey;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleAlternateQuotationEndDelimiterKey;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleCalendarIdentifier;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleCollationIdentifier;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleCollatorIdentifier;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleCountryCode;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleCurrencyCode;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleCurrencySymbol;
#[cfg(all(any(), any()))]
pub use self::__CFLocale::kCFLocaleCurrentLocaleDidChangeNotification;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleDecimalSeparator;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleExemplarCharacterSet;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleGroupingSeparator;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleIdentifier;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleLanguageCode;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleMeasurementSystem;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleQuotationBeginDelimiterKey;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleQuotationEndDelimiterKey;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleScriptCode;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleUsesMetricSystem;
#[cfg(any())]
pub use self::__CFLocale::kCFLocaleVariantCode;
#[cfg(any())]
pub use self::__CFLocale::kCFMalayalamCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFMarathiCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFOdiaCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFPersianCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFRepublicOfChinaCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFTamilCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFTeluguCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFVietnameseCalendar;
#[cfg(any())]
pub use self::__CFLocale::kCFVikramCalendar;
#[cfg(any())]
pub use self::__CFLocale::CFCalendarIdentifier;
#[cfg(any())]
pub use self::__CFLocale::CFLocale;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFLocale::CFLocaleCopyAvailableLocaleIdentifiers;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFLocale::CFLocaleCopyCommonISOCurrencyCodes;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleCopyCurrent;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleCopyDisplayNameForPropertyValue;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFLocale::CFLocaleCopyISOCountryCodes;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFLocale::CFLocaleCopyISOCurrencyCodes;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFLocale::CFLocaleCopyISOLanguageCodes;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFLocale::CFLocaleCopyPreferredLanguages;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleCreate;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleCreateCanonicalLanguageIdentifierFromString;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleCreateCanonicalLocaleIdentifierFromScriptManagerCodes;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleCreateCanonicalLocaleIdentifierFromString;
#[cfg(all(feature = "CFDictionary", any()))]
pub use self::__CFLocale::CFLocaleCreateComponentsFromLocaleIdentifier;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleCreateCopy;
#[cfg(all(feature = "CFDictionary", any()))]
pub use self::__CFLocale::CFLocaleCreateLocaleIdentifierFromComponents;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleCreateLocaleIdentifierFromWindowsLocaleCode;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleGetIdentifier;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleGetLanguageCharacterDirection;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleGetLanguageLineDirection;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleGetSystem;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleGetValue;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleGetWindowsLocaleCodeFromLocaleIdentifier;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleIdentifier;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleKey;
#[cfg(any())]
pub use self::__CFLocale::CFLocaleLanguageDirection;
#[cfg(any())]
pub use self::__CFMachPort::CFMachPort;
#[cfg(any())]
pub use self::__CFMachPort::CFMachPortCallBack;
#[cfg(any())]
pub use self::__CFMachPort::CFMachPortContext;
#[cfg(any())]
pub use self::__CFMachPort::CFMachPortCreate;
#[cfg(all(any(), feature = "CFRunLoop"))]
pub use self::__CFMachPort::CFMachPortCreateRunLoopSource;
#[cfg(all(any(), any()))]
pub use self::__CFMachPort::CFMachPortCreateWithPort;
#[cfg(any())]
pub use self::__CFMachPort::CFMachPortGetContext;
#[cfg(any())]
pub use self::__CFMachPort::CFMachPortGetInvalidationCallBack;
#[cfg(all(any(), any()))]
pub use self::__CFMachPort::CFMachPortGetPort;
#[cfg(any())]
pub use self::__CFMachPort::CFMachPortInvalidate;
#[cfg(any())]
pub use self::__CFMachPort::CFMachPortInvalidationCallBack;
#[cfg(any())]
pub use self::__CFMachPort::CFMachPortIsValid;
#[cfg(any())]
pub use self::__CFMachPort::CFMachPortSetInvalidationCallBack;
#[cfg(any())]
pub use self::__CFMessagePort::kCFMessagePortBecameInvalidError;
#[cfg(any())]
pub use self::__CFMessagePort::kCFMessagePortIsInvalid;
#[cfg(any())]
pub use self::__CFMessagePort::kCFMessagePortReceiveTimeout;
#[cfg(any())]
pub use self::__CFMessagePort::kCFMessagePortSendTimeout;
#[cfg(any())]
pub use self::__CFMessagePort::kCFMessagePortSuccess;
#[cfg(any())]
pub use self::__CFMessagePort::kCFMessagePortTransportError;
#[cfg(any())]
pub use self::__CFMessagePort::CFMessagePort;
#[cfg(all(feature = "CFData", any()))]
pub use self::__CFMessagePort::CFMessagePortCallBack;
#[cfg(any())]
pub use self::__CFMessagePort::CFMessagePortContext;
#[cfg(all(feature = "CFData", any()))]
pub use self::__CFMessagePort::CFMessagePortCreateLocal;
#[cfg(any())]
pub use self::__CFMessagePort::CFMessagePortCreateRemote;
#[cfg(all(any(), feature = "CFRunLoop"))]
pub use self::__CFMessagePort::CFMessagePortCreateRunLoopSource;
#[cfg(any())]
pub use self::__CFMessagePort::CFMessagePortGetContext;
#[cfg(any())]
pub use self::__CFMessagePort::CFMessagePortGetInvalidationCallBack;
#[cfg(any())]
pub use self::__CFMessagePort::CFMessagePortGetName;
#[cfg(any())]
pub use self::__CFMessagePort::CFMessagePortInvalidate;
#[cfg(any())]
pub use self::__CFMessagePort::CFMessagePortInvalidationCallBack;
#[cfg(any())]
pub use self::__CFMessagePort::CFMessagePortIsRemote;
#[cfg(any())]
pub use self::__CFMessagePort::CFMessagePortIsValid;
#[cfg(all(feature = "CFData", feature = "CFDate", any()))]
pub use self::__CFMessagePort::CFMessagePortSendRequest;
#[cfg(all(any(), any()))]
pub use self::__CFMessagePort::CFMessagePortSetDispatchQueue;
#[cfg(any())]
pub use self::__CFMessagePort::CFMessagePortSetInvalidationCallBack;
#[cfg(any())]
pub use self::__CFMessagePort::CFMessagePortSetName;
#[cfg(any())]
pub use self::__CFNotificationCenter::kCFNotificationDeliverImmediately;
#[cfg(any())]
pub use self::__CFNotificationCenter::kCFNotificationPostToAllSessions;
#[cfg(all(feature = "CFDictionary", any()))]
pub use self::__CFNotificationCenter::CFNotificationCallback;
#[cfg(any())]
pub use self::__CFNotificationCenter::CFNotificationCenter;
#[cfg(all(feature = "CFDictionary", any()))]
pub use self::__CFNotificationCenter::CFNotificationCenterAddObserver;
#[cfg(any())]
pub use self::__CFNotificationCenter::CFNotificationCenterGetDarwinNotifyCenter;
#[cfg(any())]
pub use self::__CFNotificationCenter::CFNotificationCenterGetDistributedCenter;
#[cfg(any())]
pub use self::__CFNotificationCenter::CFNotificationCenterGetLocalCenter;
#[cfg(all(feature = "CFDictionary", any()))]
pub use self::__CFNotificationCenter::CFNotificationCenterPostNotification;
#[cfg(all(feature = "CFDictionary", any()))]
pub use self::__CFNotificationCenter::CFNotificationCenterPostNotificationWithOptions;
#[cfg(any())]
pub use self::__CFNotificationCenter::CFNotificationCenterRemoveEveryObserver;
#[cfg(any())]
pub use self::__CFNotificationCenter::CFNotificationCenterRemoveObserver;
#[cfg(any())]
pub use self::__CFNotificationCenter::CFNotificationName;
#[cfg(any())]
pub use self::__CFNotificationCenter::CFNotificationSuspensionBehavior;
#[cfg(any())]
pub use self::__CFNumber::kCFBooleanFalse;
#[cfg(any())]
pub use self::__CFNumber::kCFBooleanTrue;
#[cfg(any())]
pub use self::__CFNumber::kCFNumberNaN;
#[cfg(any())]
pub use self::__CFNumber::kCFNumberNegativeInfinity;
#[cfg(any())]
pub use self::__CFNumber::kCFNumberPositiveInfinity;
#[cfg(any())]
pub use self::__CFNumber::CFBoolean;
#[cfg(any())]
pub use self::__CFNumber::CFBooleanGetValue;
#[cfg(any())]
pub use self::__CFNumber::CFNumber;
#[cfg(any())]
pub use self::__CFNumber::CFNumberCompare;
#[cfg(any())]
pub use self::__CFNumber::CFNumberCreate;
#[cfg(any())]
pub use self::__CFNumber::CFNumberGetByteSize;
#[cfg(any())]
pub use self::__CFNumber::CFNumberGetType;
#[cfg(any())]
pub use self::__CFNumber::CFNumberGetValue;
#[cfg(any())]
pub use self::__CFNumber::CFNumberIsFloatType;
#[cfg(any())]
pub use self::__CFNumber::CFNumberType;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterAlwaysShowDecimalSeparator;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterCurrencyCode;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterCurrencyDecimalSeparator;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterCurrencyGroupingSeparator;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterCurrencySymbol;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterDecimalSeparator;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterDefaultFormat;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterExponentSymbol;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterFormatWidth;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterGroupingSeparator;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterGroupingSize;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterInfinitySymbol;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterInternationalCurrencySymbol;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterIsLenient;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterMaxFractionDigits;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterMaxIntegerDigits;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterMaxSignificantDigits;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterMinFractionDigits;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterMinGroupingDigits;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterMinIntegerDigits;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterMinSignificantDigits;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterMinusSign;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterMultiplier;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterNaNSymbol;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterNegativePrefix;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterNegativeSuffix;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterPaddingCharacter;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterPaddingPosition;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterPerMillSymbol;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterPercentSymbol;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterPlusSign;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterPositivePrefix;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterPositiveSuffix;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterRoundingIncrement;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterRoundingMode;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterSecondaryGroupingSize;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterUseGroupingSeparator;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterUseSignificantDigits;
#[cfg(any())]
pub use self::__CFNumberFormatter::kCFNumberFormatterZeroSymbol;
#[cfg(any())]
pub use self::__CFNumberFormatter::CFNumberFormatter;
#[cfg(any())]
pub use self::__CFNumberFormatter::CFNumberFormatterCopyProperty;
#[cfg(all(any(), any()))]
pub use self::__CFNumberFormatter::CFNumberFormatterCreate;
#[cfg(all(any(), any()))]
pub use self::__CFNumberFormatter::CFNumberFormatterCreateNumberFromString;
#[cfg(all(any(), any()))]
pub use self::__CFNumberFormatter::CFNumberFormatterCreateStringWithNumber;
#[cfg(all(any(), any()))]
pub use self::__CFNumberFormatter::CFNumberFormatterCreateStringWithValue;
#[cfg(any())]
pub use self::__CFNumberFormatter::CFNumberFormatterGetDecimalInfoForCurrencyCode;
#[cfg(any())]
pub use self::__CFNumberFormatter::CFNumberFormatterGetFormat;
#[cfg(all(any(), any()))]
pub use self::__CFNumberFormatter::CFNumberFormatterGetLocale;
#[cfg(any())]
pub use self::__CFNumberFormatter::CFNumberFormatterGetStyle;
#[cfg(all(any(), any()))]
pub use self::__CFNumberFormatter::CFNumberFormatterGetValueFromString;
#[cfg(any())]
pub use self::__CFNumberFormatter::CFNumberFormatterKey;
#[cfg(any())]
pub use self::__CFNumberFormatter::CFNumberFormatterOptionFlags;
#[cfg(any())]
pub use self::__CFNumberFormatter::CFNumberFormatterPadPosition;
#[cfg(any())]
pub use self::__CFNumberFormatter::CFNumberFormatterRoundingMode;
#[cfg(any())]
pub use self::__CFNumberFormatter::CFNumberFormatterSetFormat;
#[cfg(any())]
pub use self::__CFNumberFormatter::CFNumberFormatterSetProperty;
#[cfg(any())]
pub use self::__CFNumberFormatter::CFNumberFormatterStyle;
#[cfg(any())]
pub use self::__CFPlugIn::kCFPlugInDynamicRegisterFunctionKey;
#[cfg(any())]
pub use self::__CFPlugIn::kCFPlugInDynamicRegistrationKey;
#[cfg(any())]
pub use self::__CFPlugIn::kCFPlugInFactoriesKey;
#[cfg(any())]
pub use self::__CFPlugIn::kCFPlugInTypesKey;
#[cfg(any())]
pub use self::__CFPlugIn::kCFPlugInUnloadFunctionKey;
#[cfg(all(any(), any()))]
pub use self::__CFPlugIn::CFPlugInAddInstanceForFactory;
#[cfg(all(any(), any(), any()))]
pub use self::__CFPlugIn::CFPlugInCreate;
#[cfg(all(any(), any()))]
pub use self::__CFPlugIn::CFPlugInDynamicRegisterFunction;
#[cfg(all(any(), any()))]
pub use self::__CFPlugIn::CFPlugInFactoryFunction;
#[cfg(all(feature = "CFArray", any(), any()))]
pub use self::__CFPlugIn::CFPlugInFindFactoriesForPlugInType;
#[cfg(all(feature = "CFArray", any(), any(), any()))]
pub use self::__CFPlugIn::CFPlugInFindFactoriesForPlugInTypeInPlugIn;
#[cfg(all(any(), any()))]
pub use self::__CFPlugIn::CFPlugInGetBundle;
#[cfg(any())]
pub use self::__CFPlugIn::CFPlugInInstance;
#[cfg(all(any(), any()))]
pub use self::__CFPlugIn::CFPlugInInstanceCreate;
#[cfg(any())]
pub use self::__CFPlugIn::CFPlugInInstanceCreateWithInstanceDataSize;
#[cfg(any())]
pub use self::__CFPlugIn::CFPlugInInstanceDeallocateInstanceDataFunction;
#[cfg(any())]
pub use self::__CFPlugIn::CFPlugInInstanceGetFactoryName;
#[cfg(any())]
pub use self::__CFPlugIn::CFPlugInInstanceGetInstanceData;
#[cfg(any())]
pub use self::__CFPlugIn::CFPlugInInstanceGetInterfaceFunction;
#[cfg(any())]
pub use self::__CFPlugIn::CFPlugInInstanceGetInterfaceFunctionTable;
#[cfg(all(any(), any()))]
pub use self::__CFPlugIn::CFPlugInIsLoadOnDemand;
#[cfg(all(any(), any()))]
pub use self::__CFPlugIn::CFPlugInRegisterFactoryFunction;
#[cfg(all(any(), any(), any()))]
pub use self::__CFPlugIn::CFPlugInRegisterFactoryFunctionByName;
#[cfg(all(any(), any()))]
pub use self::__CFPlugIn::CFPlugInRegisterPlugInType;
#[cfg(all(any(), any()))]
pub use self::__CFPlugIn::CFPlugInRemoveInstanceForFactory;
#[cfg(all(any(), any()))]
pub use self::__CFPlugIn::CFPlugInSetLoadOnDemand;
#[cfg(all(any(), any()))]
pub use self::__CFPlugIn::CFPlugInUnloadFunction;
#[cfg(all(any(), any()))]
pub use self::__CFPlugIn::CFPlugInUnregisterFactory;
#[cfg(all(any(), any()))]
pub use self::__CFPlugIn::CFPlugInUnregisterPlugInType;
#[cfg(all(any(), any()))]
pub use self::__CFPlugInCOM::IUnknownVTbl;
#[cfg(any())]
pub use self::__CFPlugInCOM::HRESULT;
#[cfg(any())]
pub use self::__CFPlugInCOM::LPVOID;
#[cfg(all(any(), any()))]
pub use self::__CFPlugInCOM::REFIID;
#[cfg(any())]
pub use self::__CFPlugInCOM::ULONG;
#[cfg(any())]
pub use self::__CFPreferences::kCFPreferencesAnyApplication;
#[cfg(any())]
pub use self::__CFPreferences::kCFPreferencesAnyHost;
#[cfg(any())]
pub use self::__CFPreferences::kCFPreferencesAnyUser;
#[cfg(any())]
pub use self::__CFPreferences::kCFPreferencesCurrentApplication;
#[cfg(any())]
pub use self::__CFPreferences::kCFPreferencesCurrentHost;
#[cfg(any())]
pub use self::__CFPreferences::kCFPreferencesCurrentUser;
#[cfg(any())]
pub use self::__CFPreferences::CFPreferencesAddSuitePreferencesToApp;
#[cfg(any())]
pub use self::__CFPreferences::CFPreferencesAppSynchronize;
#[cfg(any())]
pub use self::__CFPreferences::CFPreferencesAppValueIsForced;
#[cfg(any())]
pub use self::__CFPreferences::CFPreferencesCopyAppValue;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFPreferences::CFPreferencesCopyApplicationList;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFPreferences::CFPreferencesCopyKeyList;
#[cfg(all(feature = "CFArray", feature = "CFDictionary", any()))]
pub use self::__CFPreferences::CFPreferencesCopyMultiple;
#[cfg(any())]
pub use self::__CFPreferences::CFPreferencesCopyValue;
#[cfg(any())]
pub use self::__CFPreferences::CFPreferencesGetAppBooleanValue;
#[cfg(any())]
pub use self::__CFPreferences::CFPreferencesGetAppIntegerValue;
#[cfg(any())]
pub use self::__CFPreferences::CFPreferencesRemoveSuitePreferencesFromApp;
#[cfg(any())]
pub use self::__CFPreferences::CFPreferencesSetAppValue;
#[cfg(all(feature = "CFArray", feature = "CFDictionary", any()))]
pub use self::__CFPreferences::CFPreferencesSetMultiple;
#[cfg(any())]
pub use self::__CFPreferences::CFPreferencesSetValue;
#[cfg(any())]
pub use self::__CFPreferences::CFPreferencesSynchronize;
#[cfg(any())]
pub use self::__CFPropertyList::kCFPropertyListReadCorruptError;
#[cfg(any())]
pub use self::__CFPropertyList::kCFPropertyListReadStreamError;
#[cfg(any())]
pub use self::__CFPropertyList::kCFPropertyListReadUnknownVersionError;
#[cfg(any())]
pub use self::__CFPropertyList::kCFPropertyListWriteStreamError;
#[cfg(all(feature = "CFData", any(), any()))]
pub use self::__CFPropertyList::CFPropertyListCreateData;
#[cfg(any())]
pub use self::__CFPropertyList::CFPropertyListCreateDeepCopy;
#[cfg(all(any(), any()))]
pub use self::__CFPropertyList::CFPropertyListCreateFromStream;
#[cfg(all(feature = "CFData", any()))]
pub use self::__CFPropertyList::CFPropertyListCreateFromXMLData;
#[cfg(all(feature = "CFData", any(), any()))]
pub use self::__CFPropertyList::CFPropertyListCreateWithData;
#[cfg(all(any(), any(), any()))]
pub use self::__CFPropertyList::CFPropertyListCreateWithStream;
#[cfg(all(feature = "CFData", any()))]
pub use self::__CFPropertyList::CFPropertyListCreateXMLData;
#[cfg(any())]
pub use self::__CFPropertyList::CFPropertyListFormat;
#[cfg(any())]
pub use self::__CFPropertyList::CFPropertyListIsValid;
#[cfg(any())]
pub use self::__CFPropertyList::CFPropertyListMutabilityOptions;
#[cfg(all(any(), any(), any()))]
pub use self::__CFPropertyList::CFPropertyListWrite;
#[cfg(all(any(), any()))]
pub use self::__CFPropertyList::CFPropertyListWriteToStream;
#[cfg(feature = "CFRunLoop")]
pub use self::__CFRunLoop::CFRunLoop;
#[cfg(feature = "CFRunLoop")]
pub use self::__CFRunLoop::CFRunLoopActivity;
#[cfg(feature = "CFRunLoop")]
pub use self::__CFRunLoop::CFRunLoopMode;
#[cfg(feature = "CFRunLoop")]
pub use self::__CFRunLoop::CFRunLoopObserver;
#[cfg(feature = "CFRunLoop")]
pub use self::__CFRunLoop::CFRunLoopObserverCallBack;
#[cfg(feature = "CFRunLoop")]
pub use self::__CFRunLoop::CFRunLoopObserverContext;
#[cfg(feature = "CFRunLoop")]
pub use self::__CFRunLoop::CFRunLoopRunResult;
#[cfg(feature = "CFRunLoop")]
pub use self::__CFRunLoop::CFRunLoopSource;
#[cfg(feature = "CFRunLoop")]
pub use self::__CFRunLoop::CFRunLoopSourceContext;
#[cfg(all(feature = "CFRunLoop", any()))]
pub use self::__CFRunLoop::CFRunLoopSourceContext1;
#[cfg(feature = "CFRunLoop")]
pub use self::__CFRunLoop::CFRunLoopTimer;
#[cfg(feature = "CFRunLoop")]
pub use self::__CFRunLoop::CFRunLoopTimerCallBack;
#[cfg(feature = "CFRunLoop")]
pub use self::__CFRunLoop::CFRunLoopTimerContext;
#[cfg(any())]
pub use self::__CFSet::kCFCopyStringSetCallBacks;
#[cfg(any())]
pub use self::__CFSet::kCFTypeSetCallBacks;
#[cfg(any())]
pub use self::__CFSet::CFMutableSet;
#[cfg(any())]
pub use self::__CFSet::CFSet;
#[cfg(any())]
pub use self::__CFSet::CFSetAddValue;
#[cfg(any())]
pub use self::__CFSet::CFSetApplierFunction;
#[cfg(any())]
pub use self::__CFSet::CFSetApplyFunction;
#[cfg(any())]
pub use self::__CFSet::CFSetCallBacks;
#[cfg(any())]
pub use self::__CFSet::CFSetContainsValue;
#[cfg(any())]
pub use self::__CFSet::CFSetCopyDescriptionCallBack;
#[cfg(any())]
pub use self::__CFSet::CFSetCreate;
#[cfg(any())]
pub use self::__CFSet::CFSetCreateCopy;
#[cfg(any())]
pub use self::__CFSet::CFSetCreateMutable;
#[cfg(any())]
pub use self::__CFSet::CFSetCreateMutableCopy;
#[cfg(any())]
pub use self::__CFSet::CFSetEqualCallBack;
#[cfg(any())]
pub use self::__CFSet::CFSetGetCount;
#[cfg(any())]
pub use self::__CFSet::CFSetGetCountOfValue;
#[cfg(any())]
pub use self::__CFSet::CFSetGetValue;
#[cfg(any())]
pub use self::__CFSet::CFSetGetValueIfPresent;
#[cfg(any())]
pub use self::__CFSet::CFSetGetValues;
#[cfg(any())]
pub use self::__CFSet::CFSetHashCallBack;
#[cfg(any())]
pub use self::__CFSet::CFSetReleaseCallBack;
#[cfg(any())]
pub use self::__CFSet::CFSetRemoveAllValues;
#[cfg(any())]
pub use self::__CFSet::CFSetRemoveValue;
#[cfg(any())]
pub use self::__CFSet::CFSetReplaceValue;
#[cfg(any())]
pub use self::__CFSet::CFSetRetainCallBack;
#[cfg(any())]
pub use self::__CFSet::CFSetSetValue;
#[cfg(any())]
pub use self::__CFSocket::kCFSocketAutomaticallyReenableAcceptCallBack;
#[cfg(any())]
pub use self::__CFSocket::kCFSocketAutomaticallyReenableDataCallBack;
#[cfg(any())]
pub use self::__CFSocket::kCFSocketAutomaticallyReenableReadCallBack;
#[cfg(any())]
pub use self::__CFSocket::kCFSocketAutomaticallyReenableWriteCallBack;
#[cfg(any())]
pub use self::__CFSocket::kCFSocketCloseOnInvalidate;
#[cfg(any())]
pub use self::__CFSocket::kCFSocketCommandKey;
#[cfg(any())]
pub use self::__CFSocket::kCFSocketErrorKey;
#[cfg(any())]
pub use self::__CFSocket::kCFSocketLeaveErrors;
#[cfg(any())]
pub use self::__CFSocket::kCFSocketNameKey;
#[cfg(any())]
pub use self::__CFSocket::kCFSocketRegisterCommand;
#[cfg(any())]
pub use self::__CFSocket::kCFSocketResultKey;
#[cfg(any())]
pub use self::__CFSocket::kCFSocketRetrieveCommand;
#[cfg(any())]
pub use self::__CFSocket::kCFSocketValueKey;
#[cfg(any())]
pub use self::__CFSocket::CFSocket;
#[cfg(all(feature = "CFData", any()))]
pub use self::__CFSocket::CFSocketCallBack;
#[cfg(any())]
pub use self::__CFSocket::CFSocketCallBackType;
#[cfg(all(feature = "CFData", feature = "CFDate", any()))]
pub use self::__CFSocket::CFSocketConnectToAddress;
#[cfg(any())]
pub use self::__CFSocket::CFSocketContext;
#[cfg(all(feature = "CFData", any()))]
pub use self::__CFSocket::CFSocketCopyAddress;
#[cfg(all(feature = "CFData", any()))]
pub use self::__CFSocket::CFSocketCopyPeerAddress;
#[cfg(all(feature = "CFData", feature = "CFDate", any()))]
pub use self::__CFSocket::CFSocketCopyRegisteredSocketSignature;
#[cfg(all(feature = "CFData", feature = "CFDate", any()))]
pub use self::__CFSocket::CFSocketCopyRegisteredValue;
#[cfg(all(feature = "CFData", any()))]
pub use self::__CFSocket::CFSocketCreate;
#[cfg(all(feature = "CFData", feature = "CFDate", any()))]
pub use self::__CFSocket::CFSocketCreateConnectedToSocketSignature;
#[cfg(all(feature = "CFRunLoop", any()))]
pub use self::__CFSocket::CFSocketCreateRunLoopSource;
#[cfg(all(feature = "CFData", any()))]
pub use self::__CFSocket::CFSocketCreateWithNative;
#[cfg(all(feature = "CFData", any()))]
pub use self::__CFSocket::CFSocketCreateWithSocketSignature;
#[cfg(any())]
pub use self::__CFSocket::CFSocketDisableCallBacks;
#[cfg(any())]
pub use self::__CFSocket::CFSocketEnableCallBacks;
#[cfg(any())]
pub use self::__CFSocket::CFSocketError;
#[cfg(any())]
pub use self::__CFSocket::CFSocketGetContext;
#[cfg(any())]
pub use self::__CFSocket::CFSocketGetDefaultNameRegistryPortNumber;
#[cfg(any())]
pub use self::__CFSocket::CFSocketGetNative;
#[cfg(any())]
pub use self::__CFSocket::CFSocketGetSocketFlags;
#[cfg(any())]
pub use self::__CFSocket::CFSocketInvalidate;
#[cfg(any())]
pub use self::__CFSocket::CFSocketIsValid;
#[cfg(any())]
pub use self::__CFSocket::CFSocketNativeHandle;
#[cfg(all(feature = "CFData", feature = "CFDate", any()))]
pub use self::__CFSocket::CFSocketRegisterSocketSignature;
#[cfg(all(feature = "CFData", feature = "CFDate", any()))]
pub use self::__CFSocket::CFSocketRegisterValue;
#[cfg(all(feature = "CFData", feature = "CFDate", any()))]
pub use self::__CFSocket::CFSocketSendData;
#[cfg(all(feature = "CFData", any()))]
pub use self::__CFSocket::CFSocketSetAddress;
#[cfg(any())]
pub use self::__CFSocket::CFSocketSetDefaultNameRegistryPortNumber;
#[cfg(any())]
pub use self::__CFSocket::CFSocketSetSocketFlags;
#[cfg(all(feature = "CFData", any()))]
pub use self::__CFSocket::CFSocketSignature;
#[cfg(all(feature = "CFData", feature = "CFDate", any()))]
pub use self::__CFSocket::CFSocketUnregister;
#[cfg(any())]
pub use self::__CFStream::kCFStreamErrorDomainSOCKS;
#[cfg(any())]
pub use self::__CFStream::kCFStreamErrorDomainSSL;
#[cfg(any())]
pub use self::__CFStream::kCFStreamPropertyAppendToFile;
#[cfg(any())]
pub use self::__CFStream::kCFStreamPropertyDataWritten;
#[cfg(any())]
pub use self::__CFStream::kCFStreamPropertyFileCurrentOffset;
#[cfg(any())]
pub use self::__CFStream::kCFStreamPropertySOCKSPassword;
#[cfg(any())]
pub use self::__CFStream::kCFStreamPropertySOCKSProxy;
#[cfg(any())]
pub use self::__CFStream::kCFStreamPropertySOCKSProxyHost;
#[cfg(any())]
pub use self::__CFStream::kCFStreamPropertySOCKSProxyPort;
#[cfg(any())]
pub use self::__CFStream::kCFStreamPropertySOCKSUser;
#[cfg(any())]
pub use self::__CFStream::kCFStreamPropertySOCKSVersion;
#[cfg(any())]
pub use self::__CFStream::kCFStreamPropertyShouldCloseNativeSocket;
#[cfg(any())]
pub use self::__CFStream::kCFStreamPropertySocketNativeHandle;
#[cfg(any())]
pub use self::__CFStream::kCFStreamPropertySocketRemoteHostName;
#[cfg(any())]
pub use self::__CFStream::kCFStreamPropertySocketRemotePortNumber;
#[cfg(any())]
pub use self::__CFStream::kCFStreamPropertySocketSecurityLevel;
#[cfg(any())]
pub use self::__CFStream::kCFStreamSocketSOCKSVersion4;
#[cfg(any())]
pub use self::__CFStream::kCFStreamSocketSOCKSVersion5;
#[cfg(any())]
pub use self::__CFStream::kCFStreamSocketSecurityLevelNegotiatedSSL;
#[cfg(any())]
pub use self::__CFStream::kCFStreamSocketSecurityLevelNone;
#[cfg(any())]
pub use self::__CFStream::kCFStreamSocketSecurityLevelSSLv2;
#[cfg(any())]
pub use self::__CFStream::kCFStreamSocketSecurityLevelSSLv3;
#[cfg(any())]
pub use self::__CFStream::kCFStreamSocketSecurityLevelTLSv1;
#[cfg(any())]
pub use self::__CFStream::CFReadStream;
#[cfg(any())]
pub use self::__CFStream::CFReadStreamClientCallBack;
#[cfg(any())]
pub use self::__CFStream::CFReadStreamClose;
#[cfg(all(any(), any()))]
pub use self::__CFStream::CFReadStreamCopyDispatchQueue;
#[cfg(all(any(), any()))]
pub use self::__CFStream::CFReadStreamCopyError;
#[cfg(any())]
pub use self::__CFStream::CFReadStreamCopyProperty;
#[cfg(any())]
pub use self::__CFStream::CFReadStreamCreateWithBytesNoCopy;
#[cfg(all(any(), any()))]
pub use self::__CFStream::CFReadStreamCreateWithFile;
#[cfg(any())]
pub use self::__CFStream::CFReadStreamGetBuffer;
#[cfg(any())]
pub use self::__CFStream::CFReadStreamGetError;
#[cfg(any())]
pub use self::__CFStream::CFReadStreamGetStatus;
#[cfg(any())]
pub use self::__CFStream::CFReadStreamHasBytesAvailable;
#[cfg(any())]
pub use self::__CFStream::CFReadStreamOpen;
#[cfg(any())]
pub use self::__CFStream::CFReadStreamRead;
#[cfg(all(feature = "CFRunLoop", any()))]
pub use self::__CFStream::CFReadStreamScheduleWithRunLoop;
#[cfg(any())]
pub use self::__CFStream::CFReadStreamSetClient;
#[cfg(all(any(), any()))]
pub use self::__CFStream::CFReadStreamSetDispatchQueue;
#[cfg(any())]
pub use self::__CFStream::CFReadStreamSetProperty;
#[cfg(all(feature = "CFRunLoop", any()))]
pub use self::__CFStream::CFReadStreamUnscheduleFromRunLoop;
#[cfg(any())]
pub use self::__CFStream::CFStreamClientContext;
#[cfg(any())]
pub use self::__CFStream::CFStreamCreateBoundPair;
#[cfg(all(feature = "CFData", any(), any()))]
pub use self::__CFStream::CFStreamCreatePairWithPeerSocketSignature;
#[cfg(all(any(), any()))]
pub use self::__CFStream::CFStreamCreatePairWithSocket;
#[cfg(any())]
pub use self::__CFStream::CFStreamCreatePairWithSocketToHost;
#[cfg(any())]
pub use self::__CFStream::CFStreamError;
#[cfg(any())]
pub use self::__CFStream::CFStreamErrorDomain;
#[cfg(any())]
pub use self::__CFStream::CFStreamEventType;
#[cfg(any())]
pub use self::__CFStream::CFStreamPropertyKey;
#[cfg(any())]
pub use self::__CFStream::CFStreamStatus;
#[cfg(any())]
pub use self::__CFStream::CFWriteStream;
#[cfg(any())]
pub use self::__CFStream::CFWriteStreamCanAcceptBytes;
#[cfg(any())]
pub use self::__CFStream::CFWriteStreamClientCallBack;
#[cfg(any())]
pub use self::__CFStream::CFWriteStreamClose;
#[cfg(all(any(), any()))]
pub use self::__CFStream::CFWriteStreamCopyDispatchQueue;
#[cfg(all(any(), any()))]
pub use self::__CFStream::CFWriteStreamCopyError;
#[cfg(any())]
pub use self::__CFStream::CFWriteStreamCopyProperty;
#[cfg(any())]
pub use self::__CFStream::CFWriteStreamCreateWithAllocatedBuffers;
#[cfg(any())]
pub use self::__CFStream::CFWriteStreamCreateWithBuffer;
#[cfg(all(any(), any()))]
pub use self::__CFStream::CFWriteStreamCreateWithFile;
#[cfg(any())]
pub use self::__CFStream::CFWriteStreamGetError;
#[cfg(any())]
pub use self::__CFStream::CFWriteStreamGetStatus;
#[cfg(any())]
pub use self::__CFStream::CFWriteStreamOpen;
#[cfg(all(feature = "CFRunLoop", any()))]
pub use self::__CFStream::CFWriteStreamScheduleWithRunLoop;
#[cfg(any())]
pub use self::__CFStream::CFWriteStreamSetClient;
#[cfg(all(any(), any()))]
pub use self::__CFStream::CFWriteStreamSetDispatchQueue;
#[cfg(any())]
pub use self::__CFStream::CFWriteStreamSetProperty;
#[cfg(all(feature = "CFRunLoop", any()))]
pub use self::__CFStream::CFWriteStreamUnscheduleFromRunLoop;
#[cfg(any())]
pub use self::__CFStream::CFWriteStreamWrite;
#[cfg(feature = "CFString")]
pub use self::__CFString::CFShow;
#[cfg(feature = "CFString")]
pub use self::__CFString::CFShowStr;
#[cfg(feature = "CFString")]
pub use self::__CFString::CFStringBuiltInEncodings;
#[cfg(feature = "CFString")]
pub use self::__CFString::CFStringCompareFlags;
#[cfg(feature = "CFString")]
pub use self::__CFString::CFStringEncoding;
#[cfg(feature = "CFString")]
pub use self::__CFString::CFStringInlineBuffer;
#[cfg(feature = "CFString")]
pub use self::__CFString::CFStringNormalizationForm;
#[cfg(any())]
pub use self::__CFStringEncodingExt::CFStringEncodings;
#[cfg(any())]
pub use self::__CFStringTokenizer::kCFStringTokenizerAttributeLanguage;
#[cfg(any())]
pub use self::__CFStringTokenizer::kCFStringTokenizerAttributeLatinTranscription;
#[cfg(any())]
pub use self::__CFStringTokenizer::kCFStringTokenizerUnitLineBreak;
#[cfg(any())]
pub use self::__CFStringTokenizer::kCFStringTokenizerUnitParagraph;
#[cfg(any())]
pub use self::__CFStringTokenizer::kCFStringTokenizerUnitSentence;
#[cfg(any())]
pub use self::__CFStringTokenizer::kCFStringTokenizerUnitWord;
#[cfg(any())]
pub use self::__CFStringTokenizer::kCFStringTokenizerUnitWordBoundary;
#[cfg(any())]
pub use self::__CFStringTokenizer::CFStringTokenizer;
#[cfg(any())]
pub use self::__CFStringTokenizer::CFStringTokenizerAdvanceToNextToken;
#[cfg(any())]
pub use self::__CFStringTokenizer::CFStringTokenizerCopyBestStringLanguage;
#[cfg(any())]
pub use self::__CFStringTokenizer::CFStringTokenizerCopyCurrentTokenAttribute;
#[cfg(all(any(), any()))]
pub use self::__CFStringTokenizer::CFStringTokenizerCreate;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFStringTokenizer::CFStringTokenizerGetCurrentSubTokens;
#[cfg(any())]
pub use self::__CFStringTokenizer::CFStringTokenizerGetCurrentTokenRange;
#[cfg(any())]
pub use self::__CFStringTokenizer::CFStringTokenizerGoToTokenAtIndex;
#[cfg(any())]
pub use self::__CFStringTokenizer::CFStringTokenizerSetString;
#[cfg(any())]
pub use self::__CFStringTokenizer::CFStringTokenizerTokenType;
#[cfg(all(any(), any()))]
pub use self::__CFTimeZone::kCFTimeZoneSystemTimeZoneDidChangeNotification;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFTimeZone::CFTimeZoneCopyAbbreviation;
#[cfg(all(feature = "CFDictionary", any()))]
pub use self::__CFTimeZone::CFTimeZoneCopyAbbreviationDictionary;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFTimeZone::CFTimeZoneCopyDefault;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFTimeZone::CFTimeZoneCopyKnownNames;
#[cfg(all(feature = "CFDate", any(), any()))]
pub use self::__CFTimeZone::CFTimeZoneCopyLocalizedName;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFTimeZone::CFTimeZoneCopySystem;
#[cfg(all(feature = "CFData", feature = "CFDate", any()))]
pub use self::__CFTimeZone::CFTimeZoneCreate;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFTimeZone::CFTimeZoneCreateWithName;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFTimeZone::CFTimeZoneCreateWithTimeIntervalFromGMT;
#[cfg(all(feature = "CFData", feature = "CFDate", any()))]
pub use self::__CFTimeZone::CFTimeZoneGetData;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFTimeZone::CFTimeZoneGetDaylightSavingTimeOffset;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFTimeZone::CFTimeZoneGetName;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFTimeZone::CFTimeZoneGetNextDaylightSavingTimeTransition;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFTimeZone::CFTimeZoneGetSecondsFromGMT;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFTimeZone::CFTimeZoneIsDaylightSavingTime;
#[cfg(any())]
pub use self::__CFTimeZone::CFTimeZoneNameStyle;
#[cfg(any())]
pub use self::__CFTimeZone::CFTimeZoneResetSystem;
#[cfg(all(feature = "CFDictionary", any()))]
pub use self::__CFTimeZone::CFTimeZoneSetAbbreviationDictionary;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFTimeZone::CFTimeZoneSetDefault;
#[cfg(any())]
pub use self::__CFTree::CFTree;
#[cfg(any())]
pub use self::__CFTree::CFTreeAppendChild;
#[cfg(any())]
pub use self::__CFTree::CFTreeApplierFunction;
#[cfg(any())]
pub use self::__CFTree::CFTreeApplyFunctionToChildren;
#[cfg(any())]
pub use self::__CFTree::CFTreeContext;
#[cfg(any())]
pub use self::__CFTree::CFTreeCopyDescriptionCallBack;
#[cfg(any())]
pub use self::__CFTree::CFTreeCreate;
#[cfg(any())]
pub use self::__CFTree::CFTreeFindRoot;
#[cfg(any())]
pub use self::__CFTree::CFTreeGetChildAtIndex;
#[cfg(any())]
pub use self::__CFTree::CFTreeGetChildCount;
#[cfg(any())]
pub use self::__CFTree::CFTreeGetChildren;
#[cfg(any())]
pub use self::__CFTree::CFTreeGetContext;
#[cfg(any())]
pub use self::__CFTree::CFTreeGetFirstChild;
#[cfg(any())]
pub use self::__CFTree::CFTreeGetNextSibling;
#[cfg(any())]
pub use self::__CFTree::CFTreeGetParent;
#[cfg(any())]
pub use self::__CFTree::CFTreeInsertSibling;
#[cfg(any())]
pub use self::__CFTree::CFTreePrependChild;
#[cfg(any())]
pub use self::__CFTree::CFTreeReleaseCallBack;
#[cfg(any())]
pub use self::__CFTree::CFTreeRemove;
#[cfg(any())]
pub use self::__CFTree::CFTreeRemoveAllChildren;
#[cfg(any())]
pub use self::__CFTree::CFTreeRetainCallBack;
#[cfg(any())]
pub use self::__CFTree::CFTreeSetContext;
#[cfg(any())]
pub use self::__CFTree::CFTreeSortChildren;
#[cfg(any())]
pub use self::__CFURLAccess::kCFURLFileDirectoryContents;
#[cfg(any())]
pub use self::__CFURLAccess::kCFURLFileExists;
#[cfg(any())]
pub use self::__CFURLAccess::kCFURLFileLastModificationTime;
#[cfg(any())]
pub use self::__CFURLAccess::kCFURLFileLength;
#[cfg(any())]
pub use self::__CFURLAccess::kCFURLFileOwnerID;
#[cfg(any())]
pub use self::__CFURLAccess::kCFURLFilePOSIXMode;
#[cfg(any())]
pub use self::__CFURLAccess::kCFURLHTTPStatusCode;
#[cfg(any())]
pub use self::__CFURLAccess::kCFURLHTTPStatusLine;
#[cfg(all(
    feature = "CFArray",
    feature = "CFData",
    feature = "CFDictionary",
    any(),
    any()
))]
pub use self::__CFURLAccess::CFURLCreateDataAndPropertiesFromResource;
#[cfg(all(any(), any()))]
pub use self::__CFURLAccess::CFURLCreatePropertyFromResource;
#[cfg(all(any(), any()))]
pub use self::__CFURLAccess::CFURLDestroyResource;
#[cfg(any())]
pub use self::__CFURLAccess::CFURLError;
#[cfg(all(feature = "CFData", feature = "CFDictionary", any(), any()))]
pub use self::__CFURLAccess::CFURLWriteDataAndPropertiesToResource;
#[cfg(any())]
pub use self::__CFURLEnumerator::CFURLEnumerator;
#[cfg(all(feature = "CFArray", any(), any()))]
pub use self::__CFURLEnumerator::CFURLEnumeratorCreateForDirectoryURL;
#[cfg(all(feature = "CFArray", any()))]
pub use self::__CFURLEnumerator::CFURLEnumeratorCreateForMountedVolumes;
#[cfg(any())]
pub use self::__CFURLEnumerator::CFURLEnumeratorGetDescendentLevel;
#[cfg(all(any(), any(), any()))]
pub use self::__CFURLEnumerator::CFURLEnumeratorGetNextURL;
#[cfg(any())]
pub use self::__CFURLEnumerator::CFURLEnumeratorGetSourceDidChange;
#[cfg(any())]
pub use self::__CFURLEnumerator::CFURLEnumeratorOptions;
#[cfg(any())]
pub use self::__CFURLEnumerator::CFURLEnumeratorResult;
#[cfg(any())]
pub use self::__CFURLEnumerator::CFURLEnumeratorSkipDescendents;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationAlertHeaderKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationAlertMessageKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationAlertTopMostKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationAlternateButtonTitleKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationAlternateResponse;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationCancelResponse;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationCautionAlertLevel;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationCheckBoxTitlesKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationDefaultButtonTitleKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationDefaultResponse;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationIconURLKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationKeyboardTypesKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationLocalizationURLKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationNoDefaultButtonFlag;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationNoteAlertLevel;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationOtherButtonTitleKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationOtherResponse;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationPlainAlertLevel;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationPopUpSelectionKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationPopUpTitlesKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationProgressIndicatorValueKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationSoundURLKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationStopAlertLevel;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationTextFieldTitlesKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationTextFieldValuesKey;
#[cfg(any())]
pub use self::__CFUserNotification::kCFUserNotificationUseRadioButtonsFlag;
#[cfg(any())]
pub use self::__CFUserNotification::CFUserNotification;
#[cfg(any())]
pub use self::__CFUserNotification::CFUserNotificationCallBack;
#[cfg(any())]
pub use self::__CFUserNotification::CFUserNotificationCancel;
#[cfg(all(feature = "CFDate", feature = "CFDictionary", any()))]
pub use self::__CFUserNotification::CFUserNotificationCreate;
#[cfg(all(feature = "CFRunLoop", any()))]
pub use self::__CFUserNotification::CFUserNotificationCreateRunLoopSource;
#[cfg(all(feature = "CFDate", any(), any()))]
pub use self::__CFUserNotification::CFUserNotificationDisplayAlert;
#[cfg(all(feature = "CFDate", any(), any()))]
pub use self::__CFUserNotification::CFUserNotificationDisplayNotice;
#[cfg(all(feature = "CFDictionary", any()))]
pub use self::__CFUserNotification::CFUserNotificationGetResponseDictionary;
#[cfg(any())]
pub use self::__CFUserNotification::CFUserNotificationGetResponseValue;
#[cfg(all(feature = "CFDate", any()))]
pub use self::__CFUserNotification::CFUserNotificationReceiveResponse;
#[cfg(all(feature = "CFDate", feature = "CFDictionary", any()))]
pub use self::__CFUserNotification::CFUserNotificationUpdate;
#[cfg(all(any(), any()))]
pub use self::__CFUtilities::CFCopyHomeDirectoryURL;
#[cfg(any())]
pub use self::__CFXMLNode::kCFXMLNodeCurrentVersion;
#[cfg(any())]
pub use self::__CFXMLNode::CFXMLAttributeDeclarationInfo;
#[cfg(any())]
pub use self::__CFXMLNode::CFXMLAttributeListDeclarationInfo;
#[cfg(all(feature = "CFString", any(), any()))]
pub use self::__CFXMLNode::CFXMLDocumentInfo;
#[cfg(all(any(), any()))]
pub use self::__CFXMLNode::CFXMLDocumentTypeInfo;
#[cfg(all(feature = "CFArray", feature = "CFDictionary", any()))]
pub use self::__CFXMLNode::CFXMLElementInfo;
#[cfg(any())]
pub use self::__CFXMLNode::CFXMLElementTypeDeclarationInfo;
#[cfg(all(any(), any()))]
pub use self::__CFXMLNode::CFXMLEntityInfo;
#[cfg(any())]
pub use self::__CFXMLNode::CFXMLEntityReferenceInfo;
#[cfg(any())]
pub use self::__CFXMLNode::CFXMLEntityTypeCode;
#[cfg(all(any(), any()))]
pub use self::__CFXMLNode::CFXMLExternalID;
#[cfg(any())]
pub use self::__CFXMLNode::CFXMLNode;
#[cfg(any())]
pub use self::__CFXMLNode::CFXMLNodeCreate;
#[cfg(any())]
pub use self::__CFXMLNode::CFXMLNodeCreateCopy;
#[cfg(any())]
pub use self::__CFXMLNode::CFXMLNodeGetInfoPtr;
#[cfg(any())]
pub use self::__CFXMLNode::CFXMLNodeGetString;
#[cfg(any())]
pub use self::__CFXMLNode::CFXMLNodeGetTypeCode;
#[cfg(any())]
pub use self::__CFXMLNode::CFXMLNodeGetVersion;
#[cfg(any())]
pub use self::__CFXMLNode::CFXMLNodeTypeCode;
#[cfg(all(any(), any()))]
pub use self::__CFXMLNode::CFXMLNotationInfo;
#[cfg(any())]
pub use self::__CFXMLNode::CFXMLProcessingInstructionInfo;
#[cfg(all(any(), any()))]
pub use self::__CFXMLNode::CFXMLTree;
#[cfg(all(any(), any()))]
pub use self::__CFXMLNode::CFXMLTreeCreateWithNode;
#[cfg(all(any(), any()))]
pub use self::__CFXMLNode::CFXMLTreeGetNode;
#[cfg(any())]
pub use self::__CFXMLParser::kCFXMLTreeErrorDescription;
#[cfg(any())]
pub use self::__CFXMLParser::kCFXMLTreeErrorLineNumber;
#[cfg(any())]
pub use self::__CFXMLParser::kCFXMLTreeErrorLocation;
#[cfg(any())]
pub use self::__CFXMLParser::kCFXMLTreeErrorStatusCode;
#[cfg(all(feature = "CFDictionary", any()))]
pub use self::__CFXMLParser::CFXMLCreateStringByEscapingEntities;
#[cfg(all(feature = "CFDictionary", any()))]
pub use self::__CFXMLParser::CFXMLCreateStringByUnescapingEntities;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParser;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserAbort;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserAddChildCallBack;
#[cfg(all(feature = "CFData", any(), any(), any()))]
pub use self::__CFXMLParser::CFXMLParserCallBacks;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserContext;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserCopyDescriptionCallBack;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserCopyErrorDescription;
#[cfg(all(feature = "CFData", any(), any(), any()))]
pub use self::__CFXMLParser::CFXMLParserCreate;
#[cfg(all(feature = "CFData", any(), any(), any()))]
pub use self::__CFXMLParser::CFXMLParserCreateWithDataFromURL;
#[cfg(all(any(), any()))]
pub use self::__CFXMLParser::CFXMLParserCreateXMLStructureCallBack;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserEndXMLStructureCallBack;
#[cfg(all(feature = "CFData", any(), any(), any()))]
pub use self::__CFXMLParser::CFXMLParserGetCallBacks;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserGetContext;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserGetDocument;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserGetLineNumber;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserGetLocation;
#[cfg(all(any(), any()))]
pub use self::__CFXMLParser::CFXMLParserGetSourceURL;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserGetStatusCode;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserHandleErrorCallBack;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserOptions;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserParse;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserReleaseCallBack;
#[cfg(all(feature = "CFData", any(), any(), any()))]
pub use self::__CFXMLParser::CFXMLParserResolveExternalEntityCallBack;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserRetainCallBack;
#[cfg(any())]
pub use self::__CFXMLParser::CFXMLParserStatusCode;
#[cfg(all(feature = "CFData", any(), any(), any(), any()))]
pub use self::__CFXMLParser::CFXMLTreeCreateFromData;
#[cfg(all(
    feature = "CFData",
    feature = "CFDictionary",
    any(),
    any(),
    any(),
    any()
))]
pub use self::__CFXMLParser::CFXMLTreeCreateFromDataWithError;
#[cfg(all(any(), any(), any(), any()))]
pub use self::__CFXMLParser::CFXMLTreeCreateWithDataFromURL;
#[cfg(all(feature = "CFData", any(), any(), any()))]
pub use self::__CFXMLParser::CFXMLTreeCreateXMLData;
#[cfg(any())]
pub use self::__CFURL::kCFURLAddedToDirectoryDateKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLApplicationIsScriptableKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLAttributeModificationDateKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLCanonicalPathKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLContentAccessDateKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLContentModificationDateKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLCreationDateKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLCustomIconKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLDirectoryEntryCountKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLDocumentIdentifierKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLEffectiveIconKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileAllocatedSizeKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileContentIdentifierKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileIdentifierKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileProtectionComplete;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileProtectionCompleteUnlessOpen;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileProtectionCompleteUntilFirstUserAuthentication;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileProtectionCompleteWhenUserInactive;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileProtectionKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileProtectionNone;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileResourceIdentifierKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileResourceTypeBlockSpecial;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileResourceTypeCharacterSpecial;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileResourceTypeDirectory;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileResourceTypeKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileResourceTypeNamedPipe;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileResourceTypeRegular;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileResourceTypeSocket;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileResourceTypeSymbolicLink;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileResourceTypeUnknown;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileSecurityKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLFileSizeKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLGenerationIdentifierKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLHasHiddenExtensionKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsAliasFileKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsApplicationKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsDirectoryKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsExcludedFromBackupKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsExecutableKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsHiddenKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsMountTriggerKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsPackageKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsPurgeableKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsReadableKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsRegularFileKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsSparseKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsSymbolicLinkKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsSystemImmutableKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsUbiquitousItemKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsUserImmutableKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsVolumeKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLIsWritableKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLKeysOfUnsetValuesKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLLabelColorKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLLabelNumberKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLLinkCountKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLLocalizedLabelKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLLocalizedNameKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLLocalizedTypeDescriptionKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLMayHaveExtendedAttributesKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLMayShareFileContentKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLNameKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLParentDirectoryURLKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLPathKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLPreferredIOBlockSizeKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLQuarantinePropertiesKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLTagNamesKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLTotalFileAllocatedSizeKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLTotalFileSizeKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLTypeIdentifierKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemDownloadingErrorKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemDownloadingStatusCurrent;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemDownloadingStatusDownloaded;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemDownloadingStatusKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemDownloadingStatusNotDownloaded;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemHasUnresolvedConflictsKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemIsDownloadedKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemIsDownloadingKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemIsExcludedFromSyncKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemIsSyncPausedKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemIsUploadedKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemIsUploadingKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemPercentDownloadedKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemPercentUploadedKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemSupportedSyncControlsKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLUbiquitousItemUploadingErrorKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeAvailableCapacityForImportantUsageKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeAvailableCapacityForOpportunisticUsageKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeAvailableCapacityKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeCreationDateKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeIdentifierKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeIsAutomountedKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeIsBrowsableKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeIsEjectableKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeIsEncryptedKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeIsInternalKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeIsJournalingKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeIsLocalKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeIsReadOnlyKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeIsRemovableKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeIsRootFileSystemKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeLocalizedFormatDescriptionKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeLocalizedNameKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeMaximumFileSizeKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeMountFromLocationKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeNameKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeResourceCountKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSubtypeKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsAccessPermissionsKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsAdvisoryFileLockingKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsCasePreservedNamesKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsCaseSensitiveNamesKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsCompressionKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsExclusiveRenamingKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsExtendedSecurityKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsFileCloningKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsFileProtectionKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsHardLinksKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsImmutableFilesKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsJournalingKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsPersistentIDsKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsRenamingKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsRootDirectoryDatesKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsSparseFilesKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsSwapRenamingKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsSymbolicLinksKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsVolumeSizesKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeSupportsZeroRunsKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeTotalCapacityKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeTypeNameKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeURLForRemountingKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeURLKey;
#[cfg(any())]
pub use self::__CFURL::kCFURLVolumeUUIDStringKey;
#[cfg(any())]
pub use self::__CFURL::CFURLBookmarkCreationOptions;
#[cfg(any())]
pub use self::__CFURL::CFURLBookmarkFileCreationOptions;
#[cfg(any())]
pub use self::__CFURL::CFURLBookmarkResolutionOptions;
#[cfg(any())]
pub use self::__CFURL::CFURLCanBeDecomposed;
#[cfg(any())]
pub use self::__CFURL::CFURLClearResourcePropertyCache;
#[cfg(any())]
pub use self::__CFURL::CFURLClearResourcePropertyCacheForKey;
#[cfg(any())]
pub use self::__CFURL::CFURLComponentType;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyAbsoluteURL;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyFileSystemPath;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyFragment;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyHostName;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyLastPathComponent;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyNetLocation;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyParameterString;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyPassword;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyPath;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyPathExtension;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyQueryString;
#[cfg(all(feature = "CFArray", feature = "CFDictionary", any(), any()))]
pub use self::__CFURL::CFURLCopyResourcePropertiesForKeys;
#[cfg(all(any(), any()))]
pub use self::__CFURL::CFURLCopyResourcePropertyForKey;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyResourceSpecifier;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyScheme;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyStrictPath;
#[cfg(any())]
pub use self::__CFURL::CFURLCopyUserName;
#[cfg(all(feature = "CFString", any()))]
pub use self::__CFURL::CFURLCreateAbsoluteURLWithBytes;
#[cfg(all(feature = "CFArray", feature = "CFData", any(), any()))]
pub use self::__CFURL::CFURLCreateBookmarkData;
#[cfg(all(feature = "CFData", any()))]
pub use self::__CFURL::CFURLCreateBookmarkDataFromAliasRecord;
#[cfg(all(feature = "CFData", any(), any()))]
pub use self::__CFURL::CFURLCreateBookmarkDataFromFile;
#[cfg(all(feature = "CFArray", feature = "CFData", any(), any()))]
pub use self::__CFURL::CFURLCreateByResolvingBookmarkData;
#[cfg(any())]
pub use self::__CFURL::CFURLCreateCopyAppendingPathComponent;
#[cfg(any())]
pub use self::__CFURL::CFURLCreateCopyAppendingPathExtension;
#[cfg(any())]
pub use self::__CFURL::CFURLCreateCopyDeletingLastPathComponent;
#[cfg(any())]
pub use self::__CFURL::CFURLCreateCopyDeletingPathExtension;
#[cfg(all(feature = "CFData", feature = "CFString", any()))]
pub use self::__CFURL::CFURLCreateData;
#[cfg(all(any(), any()))]
pub use self::__CFURL::CFURLCreateFilePathURL;
#[cfg(all(any(), any()))]
pub use self::__CFURL::CFURLCreateFileReferenceURL;
#[cfg(any())]
pub use self::__CFURL::CFURLCreateFromFileSystemRepresentation;
#[cfg(any())]
pub use self::__CFURL::CFURLCreateFromFileSystemRepresentationRelativeToBase;
#[cfg(all(
    feature = "CFArray",
    feature = "CFData",
    feature = "CFDictionary",
    any()
))]
pub use self::__CFURL::CFURLCreateResourcePropertiesForKeysFromBookmarkData;
#[cfg(all(feature = "CFData", any()))]
pub use self::__CFURL::CFURLCreateResourcePropertyForKeyFromBookmarkData;
#[cfg(all(feature = "CFString", any()))]
pub use self::__CFURL::CFURLCreateStringByAddingPercentEscapes;
#[cfg(any())]
pub use self::__CFURL::CFURLCreateStringByReplacingPercentEscapes;
#[cfg(all(feature = "CFString", any()))]
pub use self::__CFURL::CFURLCreateStringByReplacingPercentEscapesUsingEncoding;
#[cfg(all(feature = "CFString", any()))]
pub use self::__CFURL::CFURLCreateWithBytes;
#[cfg(any())]
pub use self::__CFURL::CFURLCreateWithFileSystemPath;
#[cfg(any())]
pub use self::__CFURL::CFURLCreateWithFileSystemPathRelativeToBase;
#[cfg(any())]
pub use self::__CFURL::CFURLCreateWithString;
#[cfg(any())]
pub use self::__CFURL::CFURLGetBaseURL;
#[cfg(any())]
pub use self::__CFURL::CFURLGetByteRangeForComponent;
#[cfg(any())]
pub use self::__CFURL::CFURLGetBytes;
#[cfg(any())]
pub use self::__CFURL::CFURLGetFileSystemRepresentation;
#[cfg(any())]
pub use self::__CFURL::CFURLGetPortNumber;
#[cfg(any())]
pub use self::__CFURL::CFURLGetString;
#[cfg(any())]
pub use self::__CFURL::CFURLHasDirectoryPath;
#[cfg(any())]
pub use self::__CFURL::CFURLIsFileReferenceURL;
#[cfg(any())]
pub use self::__CFURL::CFURLPathStyle;
#[cfg(all(any(), any()))]
pub use self::__CFURL::CFURLResourceIsReachable;
#[cfg(all(feature = "CFDictionary", any(), any()))]
pub use self::__CFURL::CFURLSetResourcePropertiesForKeys;
#[cfg(all(any(), any()))]
pub use self::__CFURL::CFURLSetResourcePropertyForKey;
#[cfg(any())]
pub use self::__CFURL::CFURLSetTemporaryResourcePropertyForKey;
#[cfg(any())]
pub use self::__CFURL::CFURLStartAccessingSecurityScopedResource;
#[cfg(any())]
pub use self::__CFURL::CFURLStopAccessingSecurityScopedResource;
#[cfg(all(feature = "CFData", any(), any()))]
pub use self::__CFURL::CFURLWriteBookmarkDataToFile;
#[cfg(any())]
pub use self::__CFURL::CFURL;
#[cfg(any())]
pub use self::__CFUUID::CFUUIDBytes;
#[cfg(any())]
pub use self::__CFUUID::CFUUIDCreate;
#[cfg(any())]
pub use self::__CFUUID::CFUUIDCreateFromString;
#[cfg(any())]
pub use self::__CFUUID::CFUUIDCreateFromUUIDBytes;
#[cfg(any())]
pub use self::__CFUUID::CFUUIDCreateString;
#[cfg(any())]
pub use self::__CFUUID::CFUUIDCreateWithBytes;
#[cfg(any())]
pub use self::__CFUUID::CFUUIDGetConstantUUIDWithBytes;
#[cfg(any())]
pub use self::__CFUUID::CFUUIDGetUUIDBytes;
#[cfg(any())]
pub use self::__CFUUID::CFUUID;
use crate::*;
use core::cell::UnsafeCell;
use core::ffi::*;
use core::marker::{PhantomData, PhantomPinned};
use core::ptr::NonNull;
#[cfg(feature = "objc2")]
use objc2::__framework_prelude::*;
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfallocatortypeid?language=objc)"]
pub type CFAllocatorTypeID = c_ulonglong;
extern "C" {}
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfstring?language=objc)"]
#[doc = ""]
#[doc = " This is toll-free bridged with `NSString`."]
#[doc(alias = "CFStringRef")]
#[repr(C)]
pub struct CFString {
    inner: [u8; 0],
    _p: UnsafeCell<PhantomData<(*const UnsafeCell<()>, PhantomPinned)>>,
}
cf_type ! (unsafe impl CFString { });
#[cfg(feature = "objc2")]
cf_objc2_type!(
    unsafe impl RefEncode<"__CFString"> for CFString {}
);
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfmutablestring?language=objc)"]
#[doc = ""]
#[doc = " This is toll-free bridged with `NSMutableString`."]
#[doc(alias = "CFMutableStringRef")]
#[repr(C)]
pub struct CFMutableString {
    inner: [u8; 0],
    _p: UnsafeCell<PhantomData<(*const UnsafeCell<()>, PhantomPinned)>>,
}
cf_type ! (unsafe impl CFMutableString : CFString { });
#[cfg(feature = "objc2")]
cf_objc2_type!(
    unsafe impl RefEncode<"__CFString"> for CFMutableString {}
);
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfpropertylist?language=objc)"]
#[doc(alias = "CFPropertyListRef")]
pub type CFPropertyList = CFType;
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfcomparisonresult?language=objc)"]
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CFComparisonResult(pub CFIndex);
impl CFComparisonResult {
    #[doc(alias = "kCFCompareLessThan")]
    pub const CompareLessThan: Self = Self(-1);
    #[doc(alias = "kCFCompareEqualTo")]
    pub const CompareEqualTo: Self = Self(0);
    #[doc(alias = "kCFCompareGreaterThan")]
    pub const CompareGreaterThan: Self = Self(1);
}
#[cfg(feature = "objc2")]
unsafe impl Encode for CFComparisonResult {
    const ENCODING: Encoding = CFIndex::ENCODING;
}
#[cfg(feature = "objc2")]
unsafe impl RefEncode for CFComparisonResult {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfcomparatorfunction?language=objc)"]
pub type CFComparatorFunction = Option<
    unsafe extern "C-unwind" fn(*const c_void, *const c_void, *mut c_void) -> CFComparisonResult,
>;
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfrange?language=objc)"]
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CFRange {
    pub location: CFIndex,
    pub length: CFIndex,
}
#[cfg(feature = "objc2")]
unsafe impl Encode for CFRange {
    const ENCODING: Encoding = Encoding::Struct("?", &[<CFIndex>::ENCODING, <CFIndex>::ENCODING]);
}
#[cfg(feature = "objc2")]
unsafe impl RefEncode for CFRange {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfnull?language=objc)"]
#[doc = ""]
#[doc = " This is toll-free bridged with `NSNull`."]
#[doc(alias = "CFNullRef")]
#[repr(C)]
pub struct CFNull {
    inner: [u8; 0],
    _p: UnsafeCell<PhantomData<(*const UnsafeCell<()>, PhantomPinned)>>,
}
cf_type ! (unsafe impl CFNull { });
#[cfg(feature = "objc2")]
cf_objc2_type!(
    unsafe impl RefEncode<"__CFNull"> for CFNull {}
);
unsafe impl ConcreteType for CFNull {
    #[doc(alias = "CFNullGetTypeID")]
    #[inline]
    fn type_id() -> CFTypeID {
        extern "C-unwind" {
            fn CFNullGetTypeID() -> CFTypeID;
        }
        unsafe { CFNullGetTypeID() }
    }
}
extern "C" {}
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfallocator?language=objc)"]
#[doc(alias = "CFAllocatorRef")]
#[repr(C)]
pub struct CFAllocator {
    inner: [u8; 0],
    _p: UnsafeCell<PhantomData<(*const UnsafeCell<()>, PhantomPinned)>>,
}
cf_type ! (unsafe impl CFAllocator { });
#[cfg(feature = "objc2")]
cf_objc2_type!(
    unsafe impl RefEncode<"__CFAllocator"> for CFAllocator {}
);
extern "C" {}
extern "C" {}
extern "C" {}
extern "C" {}
extern "C" {
    #[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/kcfallocatornull?language=objc)"]
    pub static kCFAllocatorNull: Option<&'static CFAllocator>;
}
extern "C" {}
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfallocatorretaincallback?language=objc)"]
pub type CFAllocatorRetainCallBack =
    Option<unsafe extern "C-unwind" fn(*const c_void) -> *const c_void>;
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfallocatorreleasecallback?language=objc)"]
pub type CFAllocatorReleaseCallBack = Option<unsafe extern "C-unwind" fn(*const c_void)>;
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfallocatorcopydescriptioncallback?language=objc)"]
pub type CFAllocatorCopyDescriptionCallBack =
    Option<unsafe extern "C-unwind" fn(*const c_void) -> *const CFString>;
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfallocatorallocatecallback?language=objc)"]
pub type CFAllocatorAllocateCallBack =
    Option<unsafe extern "C-unwind" fn(CFIndex, CFOptionFlags, *mut c_void) -> *mut c_void>;
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfallocatorreallocatecallback?language=objc)"]
pub type CFAllocatorReallocateCallBack = Option<
    unsafe extern "C-unwind" fn(*mut c_void, CFIndex, CFOptionFlags, *mut c_void) -> *mut c_void,
>;
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfallocatordeallocatecallback?language=objc)"]
pub type CFAllocatorDeallocateCallBack =
    Option<unsafe extern "C-unwind" fn(*mut c_void, *mut c_void)>;
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfallocatorpreferredsizecallback?language=objc)"]
pub type CFAllocatorPreferredSizeCallBack =
    Option<unsafe extern "C-unwind" fn(CFIndex, CFOptionFlags, *mut c_void) -> CFIndex>;
#[doc = " [Apple's documentation](https://developer.apple.com/documentation/corefoundation/cfallocatorcontext?language=objc)"]
#[repr(C)]
#[allow(unpredictable_function_pointer_comparisons)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CFAllocatorContext {
    pub version: CFIndex,
    pub info: *mut c_void,
    pub retain: CFAllocatorRetainCallBack,
    pub release: CFAllocatorReleaseCallBack,
    pub copyDescription: CFAllocatorCopyDescriptionCallBack,
    pub allocate: CFAllocatorAllocateCallBack,
    pub reallocate: CFAllocatorReallocateCallBack,
    pub deallocate: CFAllocatorDeallocateCallBack,
    pub preferredSize: CFAllocatorPreferredSizeCallBack,
}
#[cfg(feature = "objc2")]
unsafe impl Encode for CFAllocatorContext {
    const ENCODING: Encoding = Encoding::Struct(
        "?",
        &[
            <CFIndex>::ENCODING,
            <*mut c_void>::ENCODING,
            <CFAllocatorRetainCallBack>::ENCODING,
            <CFAllocatorReleaseCallBack>::ENCODING,
            <CFAllocatorCopyDescriptionCallBack>::ENCODING,
            <CFAllocatorAllocateCallBack>::ENCODING,
            <CFAllocatorReallocateCallBack>::ENCODING,
            <CFAllocatorDeallocateCallBack>::ENCODING,
            <CFAllocatorPreferredSizeCallBack>::ENCODING,
        ],
    );
}
#[cfg(feature = "objc2")]
unsafe impl RefEncode for CFAllocatorContext {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}
unsafe impl ConcreteType for CFAllocator {
    #[doc(alias = "CFAllocatorGetTypeID")]
    #[inline]
    fn type_id() -> CFTypeID {
        extern "C-unwind" {
            fn CFAllocatorGetTypeID() -> CFTypeID;
        }
        unsafe { CFAllocatorGetTypeID() }
    }
}
impl CFAllocator {
    #[doc(alias = "CFAllocatorSetDefault")]
    #[inline]
    pub fn set_default(allocator: Option<&CFAllocator>) {
        extern "C-unwind" {
            fn CFAllocatorSetDefault(allocator: Option<&CFAllocator>);
        }
        unsafe { CFAllocatorSetDefault(allocator) }
    }
    #[doc(alias = "CFAllocatorGetDefault")]
    #[inline]
    pub fn default() -> Option<CFRetained<CFAllocator>> {
        extern "C-unwind" {
            fn CFAllocatorGetDefault() -> Option<NonNull<CFAllocator>>;
        }
        let ret = unsafe { CFAllocatorGetDefault() };
        ret.map(|ret| unsafe { CFRetained::retain(ret) })
    }
    #[doc = " # Safety"]
    #[doc = ""]
    #[doc = " - `allocator` might not allow `None`."]
    #[doc = " - `context` must be a valid pointer."]
    #[doc(alias = "CFAllocatorCreate")]
    #[inline]
    pub unsafe fn new(
        allocator: Option<&CFAllocator>,
        context: *mut CFAllocatorContext,
    ) -> Option<CFRetained<CFAllocator>> {
        extern "C-unwind" {
            fn CFAllocatorCreate(
                allocator: Option<&CFAllocator>,
                context: *mut CFAllocatorContext,
            ) -> Option<NonNull<CFAllocator>>;
        }
        let ret = unsafe { CFAllocatorCreate(allocator, context) };
        ret.map(|ret| unsafe { CFRetained::from_raw(ret) })
    }
    #[doc = " # Safety"]
    #[doc = ""]
    #[doc = " - `allocator` might not allow `None`."]
    #[doc = " - `zone` must be a valid pointer."]
    #[doc(alias = "CFAllocatorCreateWithZone")]
    #[cfg(any())]
    #[inline]
    pub unsafe fn with_zone(
        allocator: Option<&CFAllocator>,
        zone: *mut libc::malloc_zone_t,
    ) -> Option<CFRetained<CFAllocator>> {
        extern "C-unwind" {
            fn CFAllocatorCreateWithZone(
                allocator: Option<&CFAllocator>,
                zone: *mut libc::malloc_zone_t,
            ) -> Option<NonNull<CFAllocator>>;
        }
        let ret = unsafe { CFAllocatorCreateWithZone(allocator, zone) };
        ret.map(|ret| unsafe { CFRetained::from_raw(ret) })
    }
    #[doc = " # Safety"]
    #[doc = ""]
    #[doc = " `allocator` might not allow `None`."]
    #[doc(alias = "CFAllocatorAllocateTyped")]
    #[inline]
    pub unsafe fn allocate_typed(
        allocator: Option<&CFAllocator>,
        size: CFIndex,
        descriptor: CFAllocatorTypeID,
        hint: CFOptionFlags,
    ) -> *mut c_void {
        extern "C-unwind" {
            fn CFAllocatorAllocateTyped(
                allocator: Option<&CFAllocator>,
                size: CFIndex,
                descriptor: CFAllocatorTypeID,
                hint: CFOptionFlags,
            ) -> *mut c_void;
        }
        unsafe { CFAllocatorAllocateTyped(allocator, size, descriptor, hint) }
    }
    #[doc = " # Safety"]
    #[doc = ""]
    #[doc = " - `allocator` might not allow `None`."]
    #[doc = " - `ptr` must be a valid pointer."]
    #[doc(alias = "CFAllocatorReallocateTyped")]
    #[inline]
    pub unsafe fn reallocate_typed(
        allocator: Option<&CFAllocator>,
        ptr: *mut c_void,
        newsize: CFIndex,
        descriptor: CFAllocatorTypeID,
        hint: CFOptionFlags,
    ) -> *mut c_void {
        extern "C-unwind" {
            fn CFAllocatorReallocateTyped(
                allocator: Option<&CFAllocator>,
                ptr: *mut c_void,
                newsize: CFIndex,
                descriptor: CFAllocatorTypeID,
                hint: CFOptionFlags,
            ) -> *mut c_void;
        }
        unsafe { CFAllocatorReallocateTyped(allocator, ptr, newsize, descriptor, hint) }
    }
    #[doc(alias = "CFAllocatorAllocateBytes")]
    #[inline]
    pub fn allocate_bytes(
        allocator: Option<&CFAllocator>,
        size: CFIndex,
        hint: CFOptionFlags,
    ) -> *mut c_void {
        extern "C-unwind" {
            fn CFAllocatorAllocateBytes(
                allocator: Option<&CFAllocator>,
                size: CFIndex,
                hint: CFOptionFlags,
            ) -> *mut c_void;
        }
        unsafe { CFAllocatorAllocateBytes(allocator, size, hint) }
    }
    #[doc = " # Safety"]
    #[doc = ""]
    #[doc = " - `allocator` might not allow `None`."]
    #[doc = " - `ptr` must be a valid pointer."]
    #[doc(alias = "CFAllocatorReallocateBytes")]
    #[inline]
    pub unsafe fn reallocate_bytes(
        allocator: Option<&CFAllocator>,
        ptr: *mut c_void,
        newsize: CFIndex,
        hint: CFOptionFlags,
    ) -> *mut c_void {
        extern "C-unwind" {
            fn CFAllocatorReallocateBytes(
                allocator: Option<&CFAllocator>,
                ptr: *mut c_void,
                newsize: CFIndex,
                hint: CFOptionFlags,
            ) -> *mut c_void;
        }
        unsafe { CFAllocatorReallocateBytes(allocator, ptr, newsize, hint) }
    }
    #[doc(alias = "CFAllocatorAllocate")]
    #[inline]
    pub fn allocate(
        allocator: Option<&CFAllocator>,
        size: CFIndex,
        hint: CFOptionFlags,
    ) -> *mut c_void {
        extern "C-unwind" {
            fn CFAllocatorAllocate(
                allocator: Option<&CFAllocator>,
                size: CFIndex,
                hint: CFOptionFlags,
            ) -> *mut c_void;
        }
        unsafe { CFAllocatorAllocate(allocator, size, hint) }
    }
    #[doc = " # Safety"]
    #[doc = ""]
    #[doc = " - `allocator` might not allow `None`."]
    #[doc = " - `ptr` must be a valid pointer."]
    #[doc(alias = "CFAllocatorReallocate")]
    #[inline]
    pub unsafe fn reallocate(
        allocator: Option<&CFAllocator>,
        ptr: *mut c_void,
        newsize: CFIndex,
        hint: CFOptionFlags,
    ) -> *mut c_void {
        extern "C-unwind" {
            fn CFAllocatorReallocate(
                allocator: Option<&CFAllocator>,
                ptr: *mut c_void,
                newsize: CFIndex,
                hint: CFOptionFlags,
            ) -> *mut c_void;
        }
        unsafe { CFAllocatorReallocate(allocator, ptr, newsize, hint) }
    }
    #[doc = " # Safety"]
    #[doc = ""]
    #[doc = " - `allocator` might not allow `None`."]
    #[doc = " - `ptr` must be a valid pointer."]
    #[doc(alias = "CFAllocatorDeallocate")]
    #[inline]
    pub unsafe fn deallocate(allocator: Option<&CFAllocator>, ptr: *mut c_void) {
        extern "C-unwind" {
            fn CFAllocatorDeallocate(allocator: Option<&CFAllocator>, ptr: *mut c_void);
        }
        unsafe { CFAllocatorDeallocate(allocator, ptr) }
    }
    #[doc(alias = "CFAllocatorGetPreferredSizeForSize")]
    #[inline]
    pub fn preferred_size_for_size(
        allocator: Option<&CFAllocator>,
        size: CFIndex,
        hint: CFOptionFlags,
    ) -> CFIndex {
        extern "C-unwind" {
            fn CFAllocatorGetPreferredSizeForSize(
                allocator: Option<&CFAllocator>,
                size: CFIndex,
                hint: CFOptionFlags,
            ) -> CFIndex;
        }
        unsafe { CFAllocatorGetPreferredSizeForSize(allocator, size, hint) }
    }
    #[doc = " # Safety"]
    #[doc = ""]
    #[doc = " - `allocator` might not allow `None`."]
    #[doc = " - `context` must be a valid pointer."]
    #[doc(alias = "CFAllocatorGetContext")]
    #[inline]
    pub unsafe fn context(allocator: Option<&CFAllocator>, context: *mut CFAllocatorContext) {
        extern "C-unwind" {
            fn CFAllocatorGetContext(
                allocator: Option<&CFAllocator>,
                context: *mut CFAllocatorContext,
            );
        }
        unsafe { CFAllocatorGetContext(allocator, context) }
    }
}
#[inline]
pub extern "C-unwind" fn CFGetTypeID(cf: Option<&CFType>) -> CFTypeID {
    extern "C-unwind" {
        fn CFGetTypeID(cf: Option<&CFType>) -> CFTypeID;
    }
    unsafe { CFGetTypeID(cf) }
}
#[inline]
pub extern "C-unwind" fn CFCopyTypeIDDescription(
    type_id: CFTypeID,
) -> Option<CFRetained<CFString>> {
    extern "C-unwind" {
        fn CFCopyTypeIDDescription(type_id: CFTypeID) -> Option<NonNull<CFString>>;
    }
    let ret = unsafe { CFCopyTypeIDDescription(type_id) };
    ret.map(|ret| unsafe { CFRetained::from_raw(ret) })
}
#[inline]
pub extern "C-unwind" fn CFGetRetainCount(cf: Option<&CFType>) -> CFIndex {
    extern "C-unwind" {
        fn CFGetRetainCount(cf: Option<&CFType>) -> CFIndex;
    }
    unsafe { CFGetRetainCount(cf) }
}
#[inline]
pub extern "C-unwind" fn CFEqual(cf1: Option<&CFType>, cf2: Option<&CFType>) -> bool {
    extern "C-unwind" {
        fn CFEqual(cf1: Option<&CFType>, cf2: Option<&CFType>) -> Boolean;
    }
    let ret = unsafe { CFEqual(cf1, cf2) };
    ret != 0
}
#[inline]
pub extern "C-unwind" fn CFHash(cf: Option<&CFType>) -> CFHashCode {
    extern "C-unwind" {
        fn CFHash(cf: Option<&CFType>) -> CFHashCode;
    }
    unsafe { CFHash(cf) }
}
#[inline]
pub extern "C-unwind" fn CFCopyDescription(cf: Option<&CFType>) -> Option<CFRetained<CFString>> {
    extern "C-unwind" {
        fn CFCopyDescription(cf: Option<&CFType>) -> Option<NonNull<CFString>>;
    }
    let ret = unsafe { CFCopyDescription(cf) };
    ret.map(|ret| unsafe { CFRetained::from_raw(ret) })
}
#[inline]
pub extern "C-unwind" fn CFGetAllocator(cf: Option<&CFType>) -> Option<CFRetained<CFAllocator>> {
    extern "C-unwind" {
        fn CFGetAllocator(cf: Option<&CFType>) -> Option<NonNull<CFAllocator>>;
    }
    let ret = unsafe { CFGetAllocator(cf) };
    ret.map(|ret| unsafe { CFRetained::retain(ret) })
}
extern "C-unwind" {}
extern "C-unwind" {}
extern "C-unwind" {}
extern "C-unwind" {}
extern "C-unwind" {}
extern "C-unwind" {}
