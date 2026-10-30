//! Embedded Lua ("ELUA") templates.
//!
//! Port of the C `elua` library: a template such as
//! `<p><%= name %></p><% for i=1,3 do %>x<% end %>` is compiled to a Lua
//! chunk that calls `print` for every text run and `<%= %>` expression.
//! `<%# %>` blocks are comments. The original `cstr` growable string buffer is
//! replaced by [`String`].

mod elua;

pub use elua::{elua_loadbuffer, elua_loadfile, elua_parse, EluaConf, EluaContext};
