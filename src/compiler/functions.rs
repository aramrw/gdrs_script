use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    ast::{Function, Type},
    compiler::{compile_generics, statements::compile_stmt, types::compile_type_ext},
    sema::TypeInfo,
};

pub fn compile_function(
    func: &Function,
    target_obj: Option<&String>,
    is_trait_impl: bool,
    type_info: &TypeInfo,
) -> TokenStream {
    let name_to_use = &func.name;
    let name = quote::format_ident!("{}", name_to_use);
    let async_kw = if func.is_async {
        quote!(async)
    } else {
        quote!()
    };
    let attrs = func.attributes.iter().map(|a| {
        let attr = a.parse::<TokenStream>().expect("Failed to parse attribute");
        quote! { #[#attr] }
    });
    let main_attr = if func.name == "main" && func.is_async && func.attributes.is_empty() {
        quote!(#[tokio::main])
    } else {
        quote!()
    };

    let gens = compile_generics(&func.generics);
    let params = func
        .params
        .iter()
        .map(|p| {
            let p_name = quote::format_ident!("{}", p.name);
            if p.name == "self" {
                match &p.ty {
                    Type::Ref(inner, mutable) if matches!(**inner, Type::SelfType) => {
                        if *mutable {
                            quote!(&mut self)
                        } else {
                            quote!(&self)
                        }
                    }
                    Type::SelfType => {
                        if p.is_mutable {
                            quote!(mut self)
                        } else {
                            quote!(self)
                        }
                    }
                    _ => {
                        // Fallback for when self is not explicitly a reference but should be
                        // according to previous conventions or just as a default.
                        // But here we'll try to be more strict if the user wants Rust-like.
                        if p.is_mutable {
                            quote!(&mut self)
                        } else {
                            quote!(&self)
                        }
                    }
                }
            } else if p.ty == Type::Str {
                quote! { #p_name: &str }
            } else {
                let p_ty = compile_type_ext(&p.ty, target_obj);

                let is_primitive = matches!(
                    p.ty,
                    Type::I32 | Type::I64 | Type::F32 | Type::F64 | Type::Bool
                );

                // ALL Solar function parameters are passed by reference in the generated Rust
                // unless they are already references.
                if is_primitive {
                    if p.is_mutable {
                        quote!(mut #p_name: #p_ty)
                    } else {
                        quote!(#p_name: #p_ty)
                    }
                } else if matches!(p.ty, Type::Ref(_, _)) {
                    quote!(#p_name: #p_ty)
                } else if p.is_mutable {
                    quote!(#p_name: &mut #p_ty)
                } else {
                    quote!(#p_name: &#p_ty)
                }
            }
        })
        .collect::<Vec<_>>();

    let is_macroquad = func
        .attributes
        .iter()
        .any(|a| a.contains("macroquad::main"));
    let ret_type = if func.name == "main" && !is_macroquad {
        quote!(-> Result<(), Box<dyn ::std::error::Error>>)
    } else {
        match &func.return_type {
            Some(ty) => {
                let t = compile_type_ext(ty, target_obj);
                quote!(-> #t)
            }
            None => quote!(),
        }
    };
    let unit_ty = Type::Unit;
    let main_ret_ty = Type::Result(Box::new(Type::Unit), Box::new(Type::Error));

    let body = if func.name == "main" && !is_macroquad {
        let b = compile_stmt(&func.body, true, target_obj, None, type_info);
        quote! { { #b; Ok(()) } }
    } else {
        compile_stmt(
            &func.body,
            true,
            target_obj,
            func.return_type.as_ref().or(Some(&unit_ty)),
            type_info,
        )
    };
    let vis = if is_trait_impl { quote!() } else { quote!(pub) };
    quote! { #main_attr #( #attrs )* #vis #async_kw fn #name #gens (#( #params ),*) #ret_type #body }
}
