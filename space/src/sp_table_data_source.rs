//! Port of `SPTableDataSource`, the model behind an [`crate::sp_table_element::SPTableElement`].

use crate::sp_table_cell::SPTableCell;

#[allow(unused_variables)]
pub trait SPTableDataSource {
    fn get_column_count(&self) -> i32 {
        0
    }
    fn get_row_count(&self) -> i32 {
        0
    }
    fn get_uri_for_cell(&self, column: i32, row: i32) -> String {
        String::new()
    }
    fn get_text_for_cell(&self, column: i32, row: i32) -> String {
        String::new()
    }
    fn get_text_for_column(&self, column: i32) -> String {
        String::new()
    }
    fn get_row_uri(&self, index: i32) -> String {
        String::new()
    }
    fn is_row_active(&self, index: i32) -> bool {
        false
    }
    fn is_cell_active(&self, index: i32) -> bool {
        false
    }
    fn compare_table_cells(&self, column: i32, t1: &SPTableCell, t2: &SPTableCell) -> bool {
        false
    }
    fn get_track_id_for_row(&self, row: i32) -> i32 {
        0
    }
    /// Progress of a row in 0..1 (drawn as a bar in the last column), or negative for none.
    fn get_progress_for_row(&self, row: i32) -> f32 {
        -1.0
    }
    fn is_row_playable(&self, row: i32) -> bool {
        true
    }
    fn get_selected_indexes(&self) -> Vec<i32> {
        Vec::new()
    }
    fn is_index_selected(&self, index: i32) -> bool {
        false
    }
    fn add_indexes_to_selection(&self, index: i32) {}
    fn remove_indexes_from_selection(&self, index: i32) {}
}
