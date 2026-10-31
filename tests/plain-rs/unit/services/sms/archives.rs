use super::*;
use serde_json::json;

fn message() -> Value {
    json!({"id":"mms_8","body":"line\n\"quoted\"","address":"a\"b","date":"1970-01-01T00:00:01Z","threadId":"9","isMms":true})
}

#[test]
fn csv_preserves_field_values_and_quotes() {
    assert_eq!(
        csv(&[message()]).unwrap(),
        concat!(
            "id,body,address,date,thread_id,is_mms\n",
            r#""mms_8","line\n""quoted""","a""b","1000","9","true""#
        )
    );
}

#[test]
fn json_and_csv_archives_do_not_overwrite_a_batch_with_the_same_timestamp() {
    let directory = tempfile::tempdir().unwrap();
    let messages = vec![message()];
    write(directory.path(), &messages, 1_750_000_000_000).unwrap();
    write(directory.path(), &messages, 1_750_000_000_000).unwrap();
    let entries = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 4);
    for entry in entries {
        let bytes = fs::read(&entry).unwrap();
        if entry.extension().unwrap() == "json" {
            assert_eq!(
                serde_json::from_slice::<Vec<Value>>(&bytes).unwrap(),
                messages
            );
        } else {
            assert_eq!(String::from_utf8(bytes).unwrap(), csv(&messages).unwrap());
        }
    }
}

#[test]
fn retention_removes_only_owned_archives_strictly_older_than_thirty_days() {
    let directory = tempfile::tempdir().unwrap();
    let now = 1_750_000_000_000;
    let cutoff = u64::try_from(now - RETENTION_MS).unwrap();
    for (name, age) in [
        ("deleted_old.json", 1),
        ("deleted_old.csv", 1),
        ("deleted_boundary.json", 0),
        ("unrelated.json", 1),
        ("deleted_notes.txt", 1),
    ] {
        let file = fs::File::create(directory.path().join(name)).unwrap();
        file.set_times(
            fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_millis(cutoff - age)),
        )
        .unwrap();
    }
    write(directory.path(), &[message()], now).unwrap();
    assert!(!directory.path().join("deleted_old.json").exists());
    assert!(!directory.path().join("deleted_old.csv").exists());
    for name in [
        "deleted_boundary.json",
        "unrelated.json",
        "deleted_notes.txt",
    ] {
        assert!(directory.path().join(name).exists());
    }
}
