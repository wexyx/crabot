use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
use std::fmt::Debug;
use syn::{
    FnArg, Ident,
    ItemFn, ReturnType,
    Type, PathArguments, GenericArgument
};


#[derive(PartialEq, Debug)]
pub enum OutputType {
    Option,
    Result,
    Raw,
    Unknown,
}

#[derive(Debug)]
pub struct OutputInfo {
    pub ty: syn::Type,     // 最终真实类型（去掉 Result / Option）
    pub is_option: bool,   // 是否是 Option
}

pub fn generate_output_type(item_fn: &ItemFn) -> OutputInfo {
    let return_ty = match &item_fn.sig.output {
        ReturnType::Type(_, ty) => ty.as_ref(),
        ReturnType::Default => {
            // 没有返回值 -> ()
            return OutputInfo {
                ty: syn::parse_quote! { () },
                is_option: false,
            };
        }
    };

    // Step 1: 如果是 Result<T, E>，先剥掉 Result
    let inner_ty = if let Some(ok_ty) = extract_result_ok_type(return_ty) {
        ok_ty
    } else {
        return_ty.clone()
    };

    // Step 2: 判断是否 Option<T>
    if let Some(opt_inner) = extract_option_inner(&inner_ty) {
        OutputInfo {
            ty: opt_inner,
            is_option: true,
        }
    } else {
        OutputInfo {
            ty: inner_ty,
            is_option: false,
        }
    }
}

pub fn rename_origin_func(
    prefix: &str,
    struct_method: bool,
    item_fn: &ItemFn,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let fn_block = &item_fn.block; // 获取函数体
    let fn_sig = &item_fn.sig; // 获取函数签名

    let original_name = fn_sig.ident.clone();
    let new_name = Ident::from_string(format!("{}{}", prefix, original_name).as_str()).ok();
    let async_prefix = if fn_sig.asyncness.is_some() {
        quote! { async }
    } else {
        quote! {}
    };

    let output = fn_sig.output.clone();

    let inputs = fn_sig.inputs.iter();
    let mut input_args = Vec::new();
    for input in fn_sig.inputs.clone() {
        match input {
            syn::FnArg::Receiver(r) => {
            }
            syn::FnArg::Typed(t) => {
                input_args.push(t.pat.clone());
            }
        }
    }

    let new_method_code = quote! {
        #async_prefix fn #new_name(#(#inputs),*) #output
            #fn_block
    };

    let has_receiver = fn_sig
        .inputs
        .iter()
        .any(|input| matches!(input, FnArg::Receiver(_)));

    let call_code = if has_receiver {
        quote! {
            self.#new_name(#(#input_args),*).await
        }
    } else if struct_method {
        quote! {
            Self::#new_name(#(#input_args),*).await
        }
    } else {
        quote! {
            #new_name(#(#input_args),*).await
        }
    };

    return (new_method_code, call_code);
}

/// 简单驼峰转下划线函数
pub(super) fn to_snake_case(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if i != 0 {
                result.push('_');
            }
            for lc in ch.to_lowercase() {
                result.push(lc);
            }
        } else {
            result.push(ch);
        }
    }

    result
}

fn extract_result_ok_type(ty: &Type) -> Option<Type> {
    if let Type::Path(tp) = ty {
        let segment = tp.path.segments.last()?;

        if segment.ident != "Result" {
            return None;
        }

        if let PathArguments::AngleBracketed(args) = &segment.arguments {
            let mut iter = args.args.iter();

            if let Some(GenericArgument::Type(ok_ty)) = iter.next() {
                return Some(ok_ty.clone());
            }
        }
    }

    None
}

fn extract_option_inner(ty: &Type) -> Option<Type> {
    if let Type::Path(tp) = ty {
        let segment = tp.path.segments.last()?;

        if segment.ident != "Option" {
            return None;
        }

        if let PathArguments::AngleBracketed(args) = &segment.arguments {
            if let Some(GenericArgument::Type(inner)) = args.args.first() {
                return Some(inner.clone());
            }
        }
    }

    None
}