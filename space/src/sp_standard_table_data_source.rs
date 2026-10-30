//! Ports of `SPStandardTableDataSource` and `SPMockTableDataSource`.

use std::cell::RefCell;

use crate::sp_table_cell::{SPTableColumnHeader, SPTableRow};
use crate::sp_table_data_source::SPTableDataSource;

/// Rows and column headers held in memory.
#[derive(Default)]
pub struct SPStandardTableDataSource {
    pub rows: RefCell<Vec<SPTableRow>>,
    pub column_headers: RefCell<Vec<SPTableColumnHeader>>,
}

impl SPTableDataSource for SPStandardTableDataSource {
    fn get_column_count(&self) -> i32 {
        self.column_headers.borrow().len() as i32
    }
    fn get_row_count(&self) -> i32 {
        self.rows.borrow().len() as i32
    }
    fn get_uri_for_cell(&self, _column: i32, _row: i32) -> String {
        "bungalow:internal:debug".into()
    }
    fn get_text_for_cell(&self, column: i32, row: i32) -> String {
        format!("Row {row}  at column {column}")
    }
    fn get_text_for_column(&self, column: i32) -> String {
        format!("Column {column}")
    }
    fn get_row_uri(&self, _index: i32) -> String {
        "bungalow:internal:debug".into()
    }
    fn is_row_active(&self, index: i32) -> bool {
        index == 1
    }
}

/// Three by three cells of placeholder text.
#[derive(Default)]
pub struct SPMockTableDataSource {
    selected_indexes: RefCell<Vec<i32>>,
}

impl SPTableDataSource for SPMockTableDataSource {
    fn get_column_count(&self) -> i32 {
        3
    }
    fn get_row_count(&self) -> i32 {
        3
    }
    fn get_uri_for_cell(&self, _column: i32, _row: i32) -> String {
        "bungalow:internal:debug".into()
    }
    fn get_text_for_cell(&self, column: i32, row: i32) -> String {
        format!("Row {row}  at column {column}")
    }
    fn get_text_for_column(&self, column: i32) -> String {
        format!("Column {column}")
    }
    fn get_row_uri(&self, _index: i32) -> String {
        "bungalow:internal:debug".into()
    }
    fn is_row_active(&self, index: i32) -> bool {
        index == 1
    }
    fn get_selected_indexes(&self) -> Vec<i32> {
        self.selected_indexes.borrow().clone()
    }
    fn add_indexes_to_selection(&self, index: i32) {
        self.selected_indexes.borrow_mut().push(index);
    }
}
