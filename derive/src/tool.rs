use darling::FromMeta;
use darling::ast::NestedMeta;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, PatType, Type, parse_macro_input};

use crate::common::generate_output_type;

#[derive(Default, Debug, FromMeta, Clone)]
struct Config {
    #[darling(default)]
    name: String,

    #[darling(default)]
    desc: String,
}

pub fn impl_tool_function(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_args = match NestedMeta::parse_meta_list(attr.into()) {
        Ok(v) => v,
        Err(e) => {
            return TokenStream::from(darling::Error::from(e).write_errors());
        }
    };

    let config = match Config::from_list(&attr_args) {
        Ok(v) => v,
        Err(e) => {
            return TokenStream::from(e.write_errors());
        }
    };

    let func = parse_macro_input!(item as ItemFn);
    let fn_sig = &func.sig; // 获取函数签名
    if fn_sig.asyncness.is_none() {
        panic!("only async func support")
    }

    let output_info = generate_output_type(&func);
    let fn_name = &func.sig.ident;
    let tool_struct = format_ident!("__tool_{}__", fn_name);
    let mut tool_name = fn_name.to_string();
    if config.name.len() > 0 {
        tool_name = config.name;
    }

    let mut tool_desc = tool_name.clone();
    if config.desc.len() > 0 {
        tool_desc = config.desc;
    }

    // 获取第一个参数类型
    if func.sig.inputs.len() != 1 {
        panic!("require input args")
    }

    let input_ty: Type = match func.sig.inputs.first() {
        Some(FnArg::Typed(PatType { ty, .. })) => *(*ty).clone(),
        _ => panic!("require input args"),
    };

    // 取返回类型作为 Output
    let output_ty = output_info.ty;

    let ctor_ident = format_ident!("{}__bean_constructor__", &tool_struct,);

    let expanded = quote! {
        #func

        #[allow(non_camel_case_types)]
        pub struct #tool_struct {
            data: String,
        }

        #[allow(non_camel_case_types)]
        pub struct #ctor_ident;

        impl common::constructor::Constructor<String, Box<dyn common::tool::Tool>> for #ctor_ident {
            fn create(&self, input: String) -> Box<dyn common::tool::Tool>  {
                let data = #tool_struct {
                    data: input
                };

                Box::new(data)
            }
        }

        ::inventory::submit! {
            common::inventory::FactoryRegistration {
                register_fn: |reg| {
                    let factory = reg.init_factory::<String, Box<dyn common::tool::Tool>>();
                    if let Some(factory) = factory {
                        factory.register(
                            #tool_name,
                            std::sync::Arc::new(#ctor_ident)
                        );
                    }
                }
            }
        }

        #[async_trait::async_trait]
        impl common::tool::Tool for #tool_struct {
            async fn schema(&self) -> common::tool::ToolSchema {
                common::tool::ToolSchema {
                    name: #tool_name.to_string(),
                    desc: #tool_desc.to_string(),
                    input: #input_ty::into_schema(),
                    output: #output_ty::into_schema(),
                    metadata: common::tool::CapabilityMetadata::default(),
                }
            }

            async fn run(&self) -> Result<String, Error> {
                let input: #input_ty = serde_json::from_str(self.data.as_str())?;
                let output = #fn_name(input).await?;
                let data = serde_json::to_string_pretty(&output).unwrap();
                Ok(data)
            }
        }
    };

    expanded.into()
}
