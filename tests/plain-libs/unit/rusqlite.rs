use super::*;
#[test]
fn cached_statements_borrow_text_blobs_and_rollback_failed_transactions() {
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE values_test(id INTEGER PRIMARY KEY, text_value TEXT, blob_value BLOB)",
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO values_test VALUES (?1,?2,?3)",
            params![1, "sample", &[1_u8, 2, 3]],
        )
        .unwrap();
    let mut statement = connection
        .prepare_cached("SELECT text_value,blob_value FROM values_test")
        .unwrap();
    let mut rows = statement.query([]).unwrap();
    let row = rows.next().unwrap().unwrap();
    assert_eq!(row.get_ref(0).unwrap().as_str().unwrap(), "sample");
    assert_eq!(row.get_ref(1).unwrap().as_blob().unwrap(), &[1, 2, 3]);
    drop(rows);
    drop(statement);
    {
        let transaction = connection.transaction().unwrap();
        transaction
            .execute("INSERT INTO values_test VALUES (2,'rollback',NULL)", [])
            .unwrap();
        assert!(
            transaction
                .execute("INSERT INTO values_test VALUES (1,'duplicate',NULL)", [])
                .is_err()
        );
    }
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM values_test", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
