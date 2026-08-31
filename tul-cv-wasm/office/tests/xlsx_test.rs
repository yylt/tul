//! Host test for xlsx parsing using a committed fixture.

use std::io::Cursor;

use calamine::{open_workbook_from_rs, Reader, Xlsx};

#[test]
fn xlsx_fixture_parses() {
    let bytes = include_bytes!("../tests/test.xlsx");
    let mut workbook: Xlsx<Cursor<Vec<u8>>> =
        open_workbook_from_rs(Cursor::new(bytes.to_vec())).expect("open workbook");
    assert_eq!(workbook.sheet_names(), vec!["Sheet1"]);
    let range = workbook.worksheet_range("Sheet1").expect("sheet range");
    let mut rows = range.rows();
    let header: Vec<String> = rows.next().unwrap().iter().map(|c| c.to_string()).collect();
    assert_eq!(header, vec!["name", "age"]);
    let first: Vec<String> = rows.next().unwrap().iter().map(|c| c.to_string()).collect();
    assert_eq!(first, vec!["alice", "30"]);
}
