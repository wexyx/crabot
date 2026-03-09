use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Type};

use darling::{FromDeriveInput, FromField};

/// ==================
/// 字段解析
/// ==================
#[derive(Debug, FromField)]
#[darling(attributes(field))]
struct SchemaField {
    ident: Option<syn::Ident>,
    ty: Type,

    #[darling(default)]
    desc: Option<String>,
}

/// ==================
/// struct 级别解析
/// ==================
#[derive(Debug, FromDeriveInput)]
#[darling(supports(struct_named))]
struct SchemeStruct {
    ident: syn::Ident,
    data: darling::ast::Data<(), SchemaField>,
}

/// ==================
/// 类型解析
/// ==================
fn parse_type(ty: &Type) -> (String, bool, bool) {
    let ty_str = quote!(#ty).to_string().replace(' ', "");

    // Option<T>
    if ty_str.starts_with("Option<") {
        return ("string".to_string(), false, false);
    }

    // Vec<T>
    if ty_str.starts_with("Vec<") {
        return ("array".to_string(), true, true);
    }

    let t = match ty_str.as_str() {
        "i32" | "i64" | "u32" | "u64" | "f32" | "f64" => "number",
        "String" | "&str" => "string",
        "bool" => "boolean",
        _ => "object",
    };

    (t.to_string(), true, false)
}

/// ==================
/// derive 宏
/// ==================
pub fn impl_derive_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let receiver = SchemeStruct::from_derive_input(&input)
        .expect("Failed to parse struct");

    let struct_name = &receiver.ident;
    let fields = match receiver.data {
        darling::ast::Data::Struct(fields) => fields,
        _ => panic!("Only struct supported"),
    };

    let mut required_fields_tokens = Vec::new();

    let field_defs = fields.iter().map(|f| {
        let name = f.ident.as_ref().unwrap().to_string();
        let desc = f.desc.clone().unwrap_or_default();

        let (ty, default_required, is_array) = parse_type(&f.ty);
        let is_required = default_required;
        if is_required {
            required_fields_tokens.push(quote! { #name });
        }

        if is_array {
            quote! {
                properties.insert(
                    #name.to_string(),
                    serde_json::json!({
                        "type": "array",
                        "items": { "type": "string" },
                        "description": #desc
                    })
                );
            }
        } else {
            quote! {
                properties.insert(
                    #name.to_string(),
                    serde_json::json!({
                        "type": #ty,
                        "description": #desc
                    })
                );
            }
        }
    });

    let expanded = quote! {
        impl common::schema::SchemaParams for #struct_name {
            fn into_schema() -> common::schema::Schema {
                let mut properties = serde_json::Map::new();

                #(#field_defs)*

                let data = serde_json::json!({
                    "type": "object",
                    "properties": properties,
                    "required": [#(#required_fields_tokens),*]
                });

                common::schema::Schema::new(data)
            }
        }
    };

    println!("Scheme gen: {}", expanded);
    TokenStream::from(expanded)
}