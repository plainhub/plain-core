use plain_rs::events::WsEvent;
use tokio::sync::broadcast;

pub fn notification_changed(events: &broadcast::Sender<WsEvent>, app_id: &str) {
    if app_id.contains("mms")
        || matches!(
            app_id,
            "com.android.mms"
                | "com.google.android.apps.messaging"
                | "foundation.e.message"
                | "com.moez.QKSMS"
                | "com.samsung.android.messaging"
                | "com.android.mms.service"
                | "com.xiaomi.messaging"
                | "com.hicloud.android.clone"
                | "com.huawei.message"
                | "com.sonyericsson.conversations"
                | "com.htc.messaging"
                | "com.lge.messaging"
                | "com.microsoft.android.smsorganizer"
                | "com.textra"
                | "com.p1.chompsms"
                | "org.fossify.messages"
                | "com.simplemobiletools.messages"
                | "xyz.klinker.messenger"
                | "com.handcent.nextsms"
                | "com.jb.gosms"
                | "org.thoughtcrime.securesms"
        )
    {
        let _ = events.send(WsEvent::broadcast(
            "SMS_PROVIDER_CHANGED",
            "{\"uris\":[]}".into(),
        ));
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/sms/provider_events.rs"]
mod tests;
