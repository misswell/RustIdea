//! Native editor model shared by every native view.
//!
//! A [`Document`] owns the authoritative text, version, history and dirty
//! state. Views own only cursor, selection and scroll state.

pub mod document;

pub use document::{
    Change, Document, DocumentError, DocumentId, EditSource, Selection, SelectionSet, Transaction,
};
