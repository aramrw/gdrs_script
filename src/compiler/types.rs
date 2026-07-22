use proc_macro2::TokenStream;
use quote::quote;

use crate::{ast::Type, compiler::compile_id};

pub fn compile_type_ext(ty: &Type, target_obj: Option<&String>) -> TokenStream {
    match ty {
        Type::Unit => quote!(()),
        Type::I32 => quote!(i32),
        Type::I64 => quote!(i64),
        Type::F32 => quote!(f32),
        Type::F64 => quote!(f64),
        Type::Bool => quote!(bool),
        Type::Str => quote!(::std::string::String),
        Type::File => quote!(::std::fs::File),
        Type::Owned(inner) => compile_type_ext(inner, target_obj),
        Type::BoxPtr(inner) => {
            let t = compile_type_ext(inner, target_obj);
            quote!(Box<#t>)
        }
        Type::Array(inner, size) => {
            let t = compile_type_ext(inner, target_obj);
            quote!([#t; #size])
        }
        Type::RawPtr(inner, mutable) => {
            let t = compile_type_ext(inner, target_obj);
            if *mutable {
                quote!(*mut #t)
            } else {
                quote!(*const #t)
            }
        }
        Type::Ref(inner, mutable) => {
            if !*mutable && matches!(**inner, Type::Str) {
                quote!(&str)
            } else {
                let t = compile_type_ext(inner, target_obj);
                if *mutable {
                    quote!(&mut #t)
                } else {
                    quote!(&#t)
                }
            }
        }
        Type::Managed(inner) => {
            let t = compile_type_ext(inner, target_obj);
            quote!(::std::rc::Rc<::std::cell::RefCell<#t>>)
        }
        Type::ThreadSafe(inner) => {
            let t = compile_type_ext(inner, target_obj);
            quote!(::std::sync::Arc<::parking_lot::RwLock<#t>>)
        }
        Type::WeakManaged(inner) => {
            let t = compile_type_ext(inner, target_obj);
            quote!(::std::rc::Weak<::std::cell::RefCell<#t>>)
        }
        Type::WeakThreadSafe(inner) => {
            let t = compile_type_ext(inner, target_obj);
            quote!(::std::sync::Weak<::parking_lot::RwLock<#t>>)
        }
        Type::Generic(name) => {
            let id = quote::format_ident!("{}", name);
            quote!(#id)
        }
        Type::Custom(name, generics) => {
            if name == "String" || name == "string" || name == "sr_string" {
                quote!(::std::string::String)
            } else if name == "File" {
                quote!(::std::fs::File)
            } else {
                let id = compile_id(name);
                if generics.is_empty() {
                    quote!(#id)
                } else {
                    let gens = generics.iter().map(|g| compile_type_ext(g, target_obj));
                    quote!(#id<#( #gens ),*>)
                }
            }
        }
        Type::SelfType => {
            if let Some(obj) = target_obj {
                // Parse "Vector<T>" back into tokens properly
                if obj.contains('<') {
                    let parts: Vec<_> = obj.split('<').collect();
                    let id = compile_id(parts[0]);
                    let gens_str = parts[1].trim_end_matches('>');
                    let gens_tokens: Vec<_> = gens_str
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .map(|s| {
                            let gid = quote::format_ident!("{}", s);
                            quote!(#gid)
                        })
                        .collect();
                    quote!(#id<#( #gens_tokens ),*>)
                } else {
                    let id = compile_id(obj);
                    quote!(#id)
                }
            } else {
                quote!(Self)
            }
        }
        Type::Result(ok, err) => {
            let o = compile_type_ext(ok, target_obj);
            let e = compile_type_ext(err, target_obj);
            quote!(Result<#o, #e>)
        }
        Type::Tuple(types) => {
            let ts = types.iter().map(|t| compile_type_ext(t, target_obj));
            quote!((#( #ts ),*))
        }
        Type::Any => quote!(_),
        Type::Error => quote!(Box<dyn ::std::error::Error>),
    }
}
