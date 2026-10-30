//! Ports of `SPTableCell`, `SPTableRow`, `SPTableColumn`,
//! `SPTableColumnHeader` and `SPTableRowElement`: plain table data records.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SPTableCell {
    pub uri: String,
    pub title: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SPTableRow {
    pub is_selected: bool,
    pub uri: String,
    pub title: String,
    pub children: Vec<SPTableRow>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SPTableColumn;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SPTableColumnHeader {
    pub uri: String,
    pub title: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SPTableRowElement;
