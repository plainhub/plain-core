use super::*;

#[test]
fn application_id_columns_are_text() {
    let connection = Connection::open_in_memory().unwrap();
    init(&connection).unwrap();
    let mut tables = connection
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'file_names_fts%'")
        .unwrap();
    let tables = tables
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<plain_libs::rusqlite::Result<Vec<_>>>()
        .unwrap();
    for table in tables {
        let mut columns = connection
            .prepare(
                "SELECT name,type FROM pragma_table_info(?1) WHERE name='id' OR name GLOB '*_id'",
            )
            .unwrap();
        for column in columns
            .query_map([&table], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
        {
            let (name, kind) = column.unwrap();
            assert_eq!(kind, "TEXT", "{table}.{name} must use a string ID");
        }
    }
}
