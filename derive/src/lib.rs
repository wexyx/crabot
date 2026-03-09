mod tool;
mod schema;
mod common;

extern crate proc_macro;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn tool_function(attr: TokenStream, item: TokenStream) -> TokenStream {
    tool::impl_tool_function(attr, item)
}

#[proc_macro_derive(Scheme, attributes(field, tool))]
pub fn derive_schema(input: TokenStream) -> TokenStream {
    schema::impl_derive_schema(input)
}