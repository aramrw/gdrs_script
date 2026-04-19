use std::collections::HashMap;
use proc_macro2::TokenStream;
use quote::quote;

use crate::{ast::{Decl, Type}, compiler::{compile_generics, compile_id, compile_type, functions::compile_function}};

pub fn compile_decls(decls: &[Decl], tokens: &mut TokenStream) {
    for decl in decls {
        match decl {
            Decl::Function(func) => {
                tokens.extend(compile_function(func, None, false));
            }
            Decl::Trait(tr) => {
                let name = quote::format_ident!("{}", tr.name);
                let gens = compile_generics(&tr.generics);
                let bounds = if tr.bounds.is_empty() {
                    quote!()
                } else {
                    let b_tokens = tr.bounds.iter().map(|b| {
                        let id = compile_id(b);
                        quote!(#id)
                    });
                    quote!(: #( #b_tokens )+*)
                };
                let functions = tr.functions.iter().map(|f| {
                    let fname = quote::format_ident!("{}", f.name);
                    let params = f.params.iter().map(|p| {
                        let p_name = quote::format_ident!("{}", p.name);
                        if p.name == "self" {
                            match &p.ty {
                                Type::Ref(_, mutable) => {
                                    if *mutable {
                                        quote!(&mut self)
                                    } else {
                                        quote!(&self)
                                    }
                                }
                                _ => quote!(&self),
                            }
                        } else {
                            let p_ty = compile_type(&p.ty);
                            quote!(#p_name: #p_ty)
                        }
                    });
                    let ret = if let Some(rt) = &f.return_type {
                        let rty = compile_type(rt);
                        quote!(-> #rty)
                    } else {
                        quote!()
                    };
                    quote!(fn #fname(#( #params ),*) #ret;)
                });
                tokens.extend(quote! { pub trait #name #gens #bounds { #( #functions )* } });
            }
            Decl::Impl(imp) => {
                let target = compile_id(&imp.target);
                let gparams = compile_generics(&imp.generics);
                let is_trait_impl = imp.trait_name.is_some();
                let full_target = if imp.generics.is_empty() {
                    imp.target.clone()
                } else {
                    let gids: Vec<_> = imp.generics.iter().map(|(g, _)| g.clone()).collect();
                    format!("{}<{}>", imp.target, gids.join(", "))
                };
                let functions = imp
                    .functions
                    .iter()
                    .map(|f| compile_function(f, Some(&full_target), is_trait_impl));

                let assoc_types = imp.associated_types.iter().map(|(name, ty)| {
                    let id = quote::format_ident!("{}", name);
                    let ct = compile_type(ty);
                    quote!(type #id = #ct;)
                });

                if let Some(trait_name) = &imp.trait_name {
                    let tr_name = compile_id(trait_name);
                    // Use #target directly as compile_id handles generics if present in the name
                    tokens.extend(quote! { impl #gparams #tr_name for #target { 
                        #( #assoc_types )*
                        #( #functions )* 
                    } });
                } else {
                    tokens.extend(quote! { impl #gparams #target { 
                        #( #assoc_types )*
                        #( #functions )* 
                    } });
                }

                // If it has a destroy method, implement Drop
                if imp.functions.iter().any(|f| f.name == "destroy") {
                    tokens.extend(quote! {
                        impl #gparams Drop for #target {
                            fn drop(&mut self) {
                                self.destroy();
                            }
                        }
                    });
                }
            }
            Decl::Object(obj) => {
                let name = quote::format_ident!("{}", obj.name);
                let gens = compile_generics(&obj.generics);
                let fields = obj.fields.iter().map(|f| {
                    let fname = quote::format_ident!("{}", f.name);
                    let fty = if matches!(f.ty, Type::Str) {
                        quote!(::std::string::String)
                    } else {
                        compile_type(&f.ty)
                    };
                    let fattrs = f.attributes.iter().map(|a| {
                        if a.starts_with("e(") {
                            let content = &a[2..a.len() - 1];
                            quote!(#[error(#content)])
                        } else if a == "from" {
                            quote!(#[from])
                        } else {
                            let attr = a.parse::<TokenStream>().expect("Failed to parse attribute");
                            quote!(#[#attr])
                        }
                    });
                    quote! { #( #fattrs )* pub #fname: #fty }
                });
                let is_error = obj.attributes.iter().any(|a| a == "error");
                let attrs = obj.attributes.iter().filter(|a| *a != "error").map(|a| {
                    let attr = a.parse::<TokenStream>().expect("Failed to parse attribute");
                    quote! { #[#attr] }
                });
                let derive_error = if is_error {
                    quote!(#[derive(thiserror::Error, Debug, Clone)])
                } else {
                    if obj.generics.is_empty() {
                        quote!(#[derive(Clone, Debug, Default)])
                    } else {
                        quote!(#[derive(Clone, Debug)])
                    }
                };
                let gens_short = if obj.generics.is_empty() {
                    quote!()
                } else {
                    let gids = obj
                        .generics
                        .iter()
                        .map(|(g, _)| quote::format_ident!("{}", g));
                    quote!(<#( #gids ),*>)
                };
                tokens.extend(quote! {
                    #( #attrs )* #derive_error pub struct #name #gens { #( #fields ),* }
                    impl #gens crate::SolarAsVal<#name #gens_short> for #name #gens_short {
                        fn as_val(&self) -> #name #gens_short { self.clone() }
                    }
                    impl #gens crate::SolarAsVal<#name #gens_short> for &#name #gens_short {
                        fn as_val(&self) -> #name #gens_short { (*self).clone() }
                    }
                });
            }
            Decl::Enum(enm) => {
                let name = quote::format_ident!("{}", enm.name);
                let gens = compile_generics(&enm.generics);
                let is_error = enm.attributes.iter().any(|a| a == "error");
                let variants = enm.variants.iter().map(|v| {
                    let vname = quote::format_ident!("{}", v.name);
                    let has_from = v.attributes.iter().any(|a| a == "from");
                    let vattrs = v.attributes.iter().filter(|a| *a != "from").map(|a| {
                        if a.starts_with("e(") {
                            let content = &a[2..a.len() - 1];
                            quote!(#[error(#content)])
                        } else {
                            let attr = a.parse::<TokenStream>().expect("Failed to parse attribute");
                            quote!(#[#attr])
                        }
                    });

                    let from_attr = if has_from { quote!(#[from]) } else { quote!() };

                    if v.types.is_empty() {
                        quote! { #( #vattrs )* #vname }
                    } else {
                        // Place #[from] on the first field if requested
                        let mut field_tokens = Vec::new();
                        for (i, t) in v.types.iter().enumerate() {
                            let ty = compile_type(t);
                            if i == 0 {
                                field_tokens.push(quote!(#from_attr #ty));
                            } else {
                                field_tokens.push(quote!(#ty));
                            }
                        }
                        quote! { #( #vattrs )* #vname(#( #field_tokens ),*) }
                    }
                });
                let attrs = enm.attributes.iter().filter(|a| *a != "error").map(|a| {
                    let attr = a.parse::<TokenStream>().expect("Failed to parse attribute");
                    quote! { #[#attr] }
                });
                let derive_error = if is_error {
                    quote!(#[derive(thiserror::Error, Debug, Clone)])
                } else {
                    quote!(#[derive(Clone, Debug)])
                };
                let gens_short = if enm.generics.is_empty() {
                    quote!()
                } else {
                    let gids = enm
                        .generics
                        .iter()
                        .map(|(g, _)| quote::format_ident!("{}", g));
                    quote!(<#( #gids ),*>)
                };
                tokens.extend(quote! {
                    #( #attrs )* #derive_error pub enum #name #gens { #( #variants ),* }
                    impl #gens crate::SolarAsVal<#name #gens_short> for #name #gens_short {
                        fn as_val(&self) -> #name #gens_short { self.clone() }
                    }
                    impl #gens crate::SolarAsVal<#name #gens_short> for &#name #gens_short {
                        fn as_val(&self) -> #name #gens_short { (*self).clone() }
                    }
                });
            }
            Decl::ExternFunction(_) => {}
            Decl::ExternObject(obj) => {
                let name = quote::format_ident!("{}", obj.name);
                let rust_path = obj
                    .rust_path
                    .as_ref()
                    .expect("Extern object must have a rust_path");
                let target_path = compile_id(rust_path);
                
                let gens = if obj.generics.is_empty() {
                    quote!()
                } else {
                    let gids = obj
                        .generics
                        .iter()
                        .map(|(g, _)| quote::format_ident!("{}", g));
                    quote!(<#( #gids ),*>)
                };
                
                tokens.extend(quote! {
                    pub type #name #gens = #target_path #gens;
                });
            }
            Decl::ExternEnum(enm) => {
                let name = quote::format_ident!("{}", enm.name);
                let rust_path = enm
                    .rust_path
                    .as_ref()
                    .expect("Extern enum must have a rust_path");
                let target_path = compile_id(rust_path);
                
                let gens = if enm.generics.is_empty() {
                    quote!()
                } else {
                    let gids = enm
                        .generics
                        .iter()
                        .map(|(g, _)| quote::format_ident!("{}", g));
                    quote!(<#( #gids ),*>)
                };
                
                tokens.extend(quote! {
                    pub type #name #gens = #target_path #gens;
                });
            }
            Decl::ExternTrait(_) | Decl::ExternImpl(_) | Decl::RustDependency(_, _) => {}
            Decl::RustBlock(code) => {
                let comment = format!("// Injected Rust Block\n{}", code);
                tokens.extend(quote!(#comment));
            }
            Decl::Module(name, inner) => {
                let mut inner_tokens = TokenStream::new();
                compile_decls(inner, &mut inner_tokens);

                let mut mod_tokens = inner_tokens;
                let mut parts: Vec<&str> = name.split("::").collect();

                // if parts.get(0) == Some(&"std") {
                //     parts.remove(0);
                // }

                for part in parts.into_iter().rev() {
                    let id = quote::format_ident!("{}", part);
                    mod_tokens = quote! { pub mod #id { use ::std as std; use crate::{SolarStr, SolarString, SolarVec, SolarIndex, SolarAdd, SolarInto, SolarI32, SolarI64, SolarF32, SolarF64, SolarAsArg, SolarAsVal, SolarAsSize, sr_math, sr_io, sr_fs}; #mod_tokens } };
                }
                tokens.extend(mod_tokens);
            }
            Decl::Use(u) => {
                let mut path_tokens = Vec::new();
                for (i, part) in u.path.iter().enumerate() {
                    let id = quote::format_ident!("{}", part);
                    if i == 0 && u.is_crate {
                        path_tokens.push(quote!(::#id));
                    } else {
                        path_tokens.push(quote!(#id));
                    }
                }

                let path = quote!(#( #path_tokens )::*);

                if u.is_wildcard {
                    tokens.extend(quote!(pub use #path::*;));
                } else if u.items.is_empty() {
                    tokens.extend(quote!(pub use #path;));
                } else {
                    let items = u.items.iter().map(|i| quote::format_ident!("{}", i));
                    tokens.extend(quote!(pub use #path::{#( #items ),*};));
                }
            }
        }
    }
}


pub fn collect_decl_metadata(
    decls: &[Decl],
    prefix: &str,
    mappings: &mut HashMap<String, String>,
    dependencies: &mut Vec<(String, String)>,
    use_tokio: &mut bool,
) {
    for decl in decls {
        match decl {
            Decl::ExternFunction(f) => {
                if let Some(p) = &f.rust_path {
                    let name = if prefix.is_empty() {
                        f.name.clone()
                    } else {
                        format!("{}::{}", prefix, f.name)
                    };
                    mappings.insert(name, p.clone());
                }
            }
            Decl::ExternObject(o) => {
                if let Some(p) = &o.rust_path {
                    let name = if prefix.is_empty() {
                        o.name.clone()
                    } else {
                        format!("{}::{}", prefix, o.name)
                    };
                    mappings.insert(name, p.clone());
                }
            }
            Decl::ExternEnum(e) => {
                if let Some(p) = &e.rust_path {
                    let name = if prefix.is_empty() {
                        e.name.clone()
                    } else {
                        format!("{}::{}", prefix, e.name)
                    };
                    mappings.insert(name, p.clone());
                }
            }
            Decl::RustDependency(name, version) => {
                dependencies.push((name.clone(), version.clone()));
            }
            Decl::Function(f)
                if f.name == "main"
                    && f.is_async
                    && prefix.is_empty() =>
            {
                if f.attributes.is_empty() {
                    *use_tokio = true;
                }
            }
            Decl::Module(name, inner_decls) => {
                let new_prefix = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}::{}", prefix, name)
                };
                collect_decl_metadata(inner_decls, &new_prefix, mappings, dependencies, use_tokio);
            }
            _ => {}
        }
    }
}
