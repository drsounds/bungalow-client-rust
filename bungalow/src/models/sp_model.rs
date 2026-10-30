//! Port of `SPModel`: the base of all ORM model classes. Each model maps to
//! one table and serialises itself through an [`SPDBConnection`].

use super::sp_db_connection::SPDBConnection;

pub trait SPModel {
    /// Persists this model (INSERT or UPDATE). Returns true on success.
    fn save(&mut self, conn: &SPDBConnection) -> bool;

    /// Deletes this model from the database. Returns true on success.
    fn remove(&mut self, conn: &SPDBConnection) -> bool;

    /// True when the model has a valid primary key.
    fn is_persisted(&self) -> bool;
}
