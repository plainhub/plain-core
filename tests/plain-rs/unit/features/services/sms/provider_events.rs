use super::*;

#[test]
fn sms_application_hints_keep_the_existing_whitelist_and_mms_fallback() {
    for app in [
        "com.android.mms",
        "com.google.android.apps.messaging",
        "foundation.e.message",
        "com.moez.QKSMS",
        "com.samsung.android.messaging",
        "com.android.mms.service",
        "com.xiaomi.messaging",
        "com.hicloud.android.clone",
        "com.huawei.message",
        "com.sonyericsson.conversations",
        "com.htc.messaging",
        "com.lge.messaging",
        "com.microsoft.android.smsorganizer",
        "com.textra",
        "com.p1.chompsms",
        "org.fossify.messages",
        "com.simplemobiletools.messages",
        "xyz.klinker.messenger",
        "com.handcent.nextsms",
        "com.jb.gosms",
        "org.thoughtcrime.securesms",
        "vendor.mms.app",
    ] {
        let (events, mut receiver) = broadcast::channel(8);
        notification_changed(&events, app);
        let event = receiver.try_recv().unwrap();
        assert_eq!(event.event_type, "SMS_PROVIDER_CHANGED");
        assert_eq!(event.payload, "{\"uris\":[]}");
        assert!(receiver.try_recv().is_err());
    }
    for app in [
        "",
        "org.example.chat",
        "vendor.MMS.app",
        "com.google.android.apps.messaging.extra",
    ] {
        let (events, mut receiver) = broadcast::channel(8);
        notification_changed(&events, app);
        assert!(receiver.try_recv().is_err(), "{app}");
    }
}
