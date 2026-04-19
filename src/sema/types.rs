
// src/sema/types.rs

use std::collections::{HashMap, HashSet};
use crate::ast::*;
use crate::error::CompilerError;
use crate::lexer::Span;
use miette::SourceSpan;

// --- Helper struct ---
#[derive(Debug, Clone)]
pub struct TraitInfo {
    pub generics: Vec<(String, Vec<String>)>,
    pub bounds: Vec<String>,
    pub functions: HashMap<String, (Vec<(Type, ArgKind)>, Option<Type>)>,
}

// --- SemanticAnalyzer fields related to type information ---
#[derive(Debug)]
pub struct TypeInfo {
    pub functions: HashMap<String, (Vec<(Type, ArgKind)>, Option<Type>)>,
    pub objects: HashMap<String, (HashMap<String, Type>, Vec<(String, Vec<String>)>)>,
    pub enums: HashMap<String, (HashMap<String, Vec<Type>>, Vec<(String, Vec<String>)>)>,
    pub traits: HashMap<String, TraitInfo>,
    pub generic_params: HashMap<String, Vec<String>>,
    pub aliases: HashMap<String, String>,
    pub phantom_types: HashSet<String>,
    pub has_wildcard_phantom: bool,
}

impl TypeInfo {
    pub fn new() -> Self {
        let mut type_info = Self {
            functions: HashMap::new(),
            objects: HashMap::new(),
            enums: HashMap::new(),
            traits: HashMap::new(),
            generic_params: HashMap::new(),
            aliases: HashMap::new(),
            phantom_types: HashSet::new(),
            has_wildcard_phantom: false,
        };

        // Pre-populate common stdlib functions
        type_info.functions.insert(
            "mem::free".to_string(),
            (
                vec![
                    (Type::Ref(Box::new(Type::RawPtr(Box::new(Type::Any), true)), false), ArgKind::Ref),
                    (Type::Ref(Box::new(Type::I32), false), ArgKind::Ref),
                ],
                None,
            ),
        );
        type_info.functions.insert(
            "mem::alloc<>".to_string(),
            (
                vec![(Type::I32, ArgKind::Value)],
                Some(Type::RawPtr(Box::new(Type::Any), true)),
            ),
        );
        type_info.functions.insert(
            "fs::read_to_string".to_string(),
            (vec![(Type::Str, ArgKind::Value)], Some(Type::String)),
        );
        type_info
            .functions
            .insert("io::readline".to_string(), (vec![], Some(Type::String)));
        type_info.functions
            .insert("io::println".to_string(), (vec![(Type::Str, ArgKind::Value)], None));

        type_info.functions.insert(
            "str::len".to_string(),
            (vec![(Type::Ref(Box::new(Type::Str), false), ArgKind::Ref)], Some(Type::I32)),
        );
        type_info.functions.insert(
            "str::contains".to_string(),
            (
                vec![(Type::Ref(Box::new(Type::Str), false), ArgKind::Ref), (Type::Str, ArgKind::Value)],
                Some(Type::Bool),
            ),
        );
        type_info.functions.insert(
            "str::split".to_string(),
            (
                vec![(Type::Ref(Box::new(Type::Str), false), ArgKind::Ref), (Type::Str, ArgKind::Value)],
                Some(Type::Custom(
                    "std::vec::Vector<>".into(),
                    vec![Type::String],
                )),
            ),
        );

        type_info.functions
            .insert("string::new".to_string(), (vec![], Some(Type::String)));
        type_info.functions.insert(
            "string::len".to_string(),
            (
                vec![(Type::Ref(Box::new(Type::String), false), ArgKind::Ref)],
                Some(Type::I32),
            ),
        );
        type_info.functions.insert(
            "string::contains".to_string(),
            (
                vec![(Type::Ref(Box::new(Type::String), false), ArgKind::Ref), (Type::Str, ArgKind::Value)],
                Some(Type::Bool),
            ),
        );
        type_info.functions.insert(
            "string::split".to_string(),
            (
                vec![(Type::Ref(Box::new(Type::String), false), ArgKind::Ref), (Type::Str, ArgKind::Value)],
                Some(Type::Custom(
                    "std::vec::Vector<>".into(),
                    vec![Type::String],
                )),
            ),
        );
        type_info.functions.insert(
            "string::to_str".to_string(),
            (
                vec![(Type::Ref(Box::new(Type::String), false), ArgKind::Ref)],
                Some(Type::Str),
            ),
        );
        type_info.functions.insert(
            "i32::to_string".to_string(),
            (vec![(Type::Ref(Box::new(Type::I32), false), ArgKind::Ref)], Some(Type::String)),
        );
        type_info.functions.insert(
            "i64::to_string".to_string(),
            (vec![(Type::Ref(Box::new(Type::I64), false), ArgKind::Ref)], Some(Type::String)),
        );
        type_info.functions.insert(
            "f32::to_string".to_string(),
            (vec![(Type::Ref(Box::new(Type::F32), false), ArgKind::Ref)], Some(Type::String)),
        );
        type_info.functions.insert(
            "f64::to_string".to_string(),
            (vec![(Type::Ref(Box::new(Type::F64), false), ArgKind::Ref)], Some(Type::String)),
        );
        type_info.functions.insert(
            "f32::powi".to_string(),
            (
                vec![(Type::Ref(Box::new(Type::F32), false), ArgKind::Ref), (Type::I32, ArgKind::Value)],
                Some(Type::F32),
            ),
        );
        type_info.functions.insert(
            "f32::sqrt".to_string(),
            (vec![(Type::Ref(Box::new(Type::F32), false), ArgKind::Ref)], Some(Type::F32)),
        );

        type_info.functions.insert(
            "string::append".to_string(),
            (
                vec![(Type::Ref(Box::new(Type::String), true), ArgKind::MutRef), (Type::Str, ArgKind::Value)],
                None,
            ),
        );

        type_info.functions.insert(
            "Array::len".to_string(),
            (vec![], Some(Type::I32)),
        );
        type_info.functions.insert(
            "std::vec::Vector<>::push".to_string(),
            (vec![(Type::Generic("T".into()), ArgKind::Value)], None),
        );
        type_info.functions.insert(
            "std::vec::Vector<>::pop".to_string(),
            (vec![], Some(Type::Custom("std::option::Option<>".into(), vec![Type::Generic("T".into())]))),
        );
        type_info.functions.insert(
            "std::vec::Vector<>::get".to_string(),
            (vec![(Type::Ref(Box::new(Type::I32), false), ArgKind::Ref)], Some(Type::Generic("T".into()))),
        );
        type_info.functions.insert(
            "std::vec::Vector<>::len".to_string(),
            (vec![], Some(Type::I32)),
        );
        type_info.functions.insert(
            "std::vec::Vector<>::new".to_string(),
            (vec![(Type::Generic("T".into()), ArgKind::Value)], Some(Type::Custom("std::vec::Vector<>".into(), vec![Type::Generic("T".into())]))),
        );
        type_info
    }

    pub fn resolve_type(&mut self, ty: &Type, prefix: &str) -> Type {
        match ty {
            Type::BoxPtr(inner) => Type::BoxPtr(Box::new(self.resolve_type(inner, prefix))),
            Type::RawPtr(inner, mutable) => {
                Type::RawPtr(Box::new(self.resolve_type(inner, prefix)), *mutable)
            }
            Type::Ref(inner, mutable) => Type::Ref(Box::new(self.resolve_type(inner, prefix)), *mutable),
            Type::Managed(inner) => Type::Managed(Box::new(self.resolve_type(inner, prefix))),
            Type::ThreadSafe(inner) => Type::ThreadSafe(Box::new(self.resolve_type(inner, prefix))),
            Type::WeakManaged(inner) => Type::WeakManaged(Box::new(self.resolve_type(inner, prefix))),
            Type::WeakThreadSafe(inner) => Type::WeakThreadSafe(Box::new(self.resolve_type(inner, prefix))),
            Type::Array(inner, size) => Type::Array(Box::new(self.resolve_type(inner, prefix)), *size),
            Type::SelfType => {
                // This logic is currently in mod.rs for `analyze_function`, but `resolve_type` is called *during* that analysis.
                // It should probably live with the symbol table, which will be in `analysis.rs`.
                // For now, let's assume it will be handled by `analysis.rs` context.
                // Returning Any as a placeholder.
                Type::Any 
            }
            Type::Custom(name, generics) => {
                if self.generic_params.contains_key(name) {
                    Type::Generic(name.clone())
                } else if name == "String" {
                    Type::String
                } else if name == "File" {
                    Type::File
                } else if name == "any" {
                    Type::Any
                } else {
                    let resolved_generics: Vec<_> =
                        generics.iter().map(|g| self.resolve_type(g, prefix)).collect();

                    let mut name_to_lookup = name.clone();
                    // NOTE: Logic for 'Self' and 'self' needs context from symbol table (current_obj),
                    // which belongs in analysis.rs. This part needs to be carefully managed.
                    if name == "Self" || name == "self" {
                        // Placeholder: This requires symbol table context (current_obj)
                        // This will be resolved in analysis.rs
                        if !resolved_generics.is_empty() {
                             name_to_lookup = format!("{}<>", name_to_lookup); // Add <> if generics are resolved
                        }
                    } else if name.contains("::") {
                        let parts: Vec<&str> = name.split("::").collect();
                        if let Some(alias) = self.aliases.get(parts[0]) {
                            name_to_lookup = format!("{}::{}", alias, parts[1..].join("::"));
                        }
                    } else if let Some(alias) = self.aliases.get(name) {
                        name_to_lookup = alias.clone();
                    } else {
                        if !prefix.is_empty() {
                            let prefixed = format!("{}::{}", prefix, name); 
                            if self.objects.contains_key(&prefixed)
                                || self.objects.contains_key(&format!("{}<>", prefixed))
                                || self.enums.contains_key(&prefixed)
                                || self.enums.contains_key(&format!("{}<>", prefixed))
                                || self.traits.contains_key(&prefixed)
                                || self.traits.contains_key(&format!("{}<>", prefixed))
                            {
                                name_to_lookup = prefixed;
                            }
                        }
                    }

                    if !resolved_generics.is_empty() && !name_to_lookup.ends_with("<>") {
                        let generic_name = format!("{}<>", name_to_lookup);
                        Type::Custom(generic_name, resolved_generics)
                    } else {
                        Type::Custom(name_to_lookup, resolved_generics)
                    }
                }
            }
            Type::Result(ok, err) => Type::Result(
                Box::new(self.resolve_type(ok, prefix)),
                Box::new(self.resolve_type(err, prefix)),
            ),
            _ => ty.clone(),
        }
    }
    
    // Placeholder for path resolution logic that might need context from analysis.rs
    pub fn resolve_path(&self, path: &[PathPart]) -> String {
        // This is a simplified version. The full logic involves checking aliases,
        // current prefix, and known types. This will need to be integrated with
        // analysis.rs context if it depends on `current_prefix` or `aliases` that
        // might be managed differently.
        let mut full_path = String::new();
        for (i, part) in path.iter().enumerate() {
            if i > 0 {
                full_path.push_str("::");
            }
            full_path.push_str(&part.name);
        }
        full_path
    }

    // Logic to collect type declarations (objects, enums, traits, functions, impls, uses)
    pub fn collect_decls(&mut self, decls: &[Decl], prefix: &str) -> Result<(), CompilerError> {
        for decl in decls {
            match decl {
                Decl::Object(obj) | Decl::ExternObject(obj) => {
                    let mut full_name = if prefix.is_empty() {
                        obj.name.clone()
                    } else {
                        if obj.name.contains("::") {
                            obj.name.clone()
                        } else {
                            format!("{}::{}", prefix, obj.name)
                        }
                    };
                    if !obj.generics.is_empty() {
                        full_name = format!("{}<>", full_name);
                    }

                    let mut fields = HashMap::new();
                    for f in &obj.fields {
                        fields.insert(f.name.clone(), self.resolve_type(&f.ty, prefix));
                    }
                    self.objects
                        .insert(full_name, (fields, obj.generics.clone()));
                }
                Decl::Enum(enm) | Decl::ExternEnum(enm) => {
                    let mut full_name = if prefix.is_empty() {
                        enm.name.clone()
                    } else {
                        if enm.name.contains("::") {
                            enm.name.clone()
                        } else {
                            format!("{}::{}", prefix, enm.name)
                        }
                    };
                    if !enm.generics.is_empty() {
                        full_name = format!("{}<>", full_name);
                    }
                    let mut variants = HashMap::new();
                    for v in &enm.variants {
                        variants.insert(v.name.clone(), v.types.clone());
                    }
                    self.enums
                        .insert(full_name, (variants, enm.generics.clone()));
                }
                Decl::Trait(tr) | Decl::ExternTrait(tr) => {
                    let mut full_name = if prefix.is_empty() {
                        tr.name.clone()
                    } else {
                        if tr.name.contains("::") {
                            tr.name.clone()
                        } else {
                            format!("{}::{}", prefix, tr.name)
                        }
                    };
                    if !tr.generics.is_empty() {
                        full_name = format!("{}<>", full_name);
                    }
                    let mut trait_funcs = HashMap::new();
                    for f in &tr.functions {
                        let ret = f.return_type.as_ref().map(|t| self.resolve_type(t, prefix));
                        let sig_params: Vec<_> = f
                            .params
                            .iter()
                            .map(|p| {
                                let ty = self.resolve_type(&p.ty, prefix);
                                let kind = match &ty {
                                    Type::Ref(_, true) => ArgKind::MutRef,
                                    Type::Ref(_, false) => ArgKind::Ref,
                                    _ if p.is_mutable => ArgKind::MutRef,
                                    _ => ArgKind::Value,
                                };
                                (ty, kind)
                            })
                            .collect();
                        let sig = (sig_params, ret);
                        trait_funcs.insert(f.name.clone(), sig.clone());
                        // Store with full namespaced trait name for direct lookup
                        self.functions
                            .insert(format!("{}::{}", full_name, f.name), sig);
                    }
                    self.traits.insert(
                        full_name,
                        TraitInfo {
                            generics: tr.generics.clone(),
                            bounds: tr.bounds.clone(),
                            functions: trait_funcs,
                        },
                    );
                }
                Decl::Function(func) | Decl::ExternFunction(func) => {
                    let full_name = if prefix.is_empty() {
                        func.name.clone()
                    } else {
                        if func.name.contains("::") {
                            func.name.clone()
                        } else {
                            format!("{}::{}", prefix, func.name)
                        }
                    };
                    let mut final_name = full_name;
                    if !func.generics.is_empty() {
                        final_name = format!("{}<>", final_name);
                    }

                    // Store function signatures. Generic parameters are resolved later in analysis.
                    // For now, we just record them.
                    let mut resolved_params = Vec::new();
                    for p in &func.params {
                        let ty = self.resolve_type(&p.ty, prefix);
                        let kind = match &ty {
                            Type::Ref(_, true) => ArgKind::MutRef,
                            Type::Ref(_, false) => ArgKind::Ref,
                            _ if p.is_mutable => ArgKind::MutRef,
                            _ => ArgKind::Value,
                        };
                        resolved_params.push((ty, kind));
                    }
                    let resolved_ret = func.return_type.as_ref().map(|t| self.resolve_type(t, prefix));

                    self.functions.insert(final_name.clone(), (resolved_params, resolved_ret));
                    //println!("DEBUG collect_decls added function: {}", final_name);
                }
                Decl::Module(name, inner) => {
                    let new_prefix = if prefix.is_empty() {
                        name.clone()
                    } else {
                        if name.contains("::") {
                            name.clone()
                        } else {
                            format!("{}::{}", prefix, name)
                        }
                    };
                    // If it's a nested module, create an alias for the last part
                    if new_prefix.contains("::") {
                        let parts: Vec<&str> = new_prefix.split("::").collect();
                        let last_part = parts.last().unwrap();
                        self.aliases.insert(last_part.to_string(), new_prefix.clone());
                    }
                    self.collect_decls(inner, &new_prefix)?;
                }
                Decl::Use(u) => {
                    if u.is_crate {
                        if let Some(crate_name) = u.path.first() {
                            self.phantom_types.insert(crate_name.clone());
                        }
                    }
                    if !u.path.is_empty() {
                        let full_path = u.path.join("::");
                        let last_part = u.path.last().unwrap();
                        // Use alias for the last part of the path
                        self.aliases.insert(last_part.clone(), full_path);
                    }
                    if u.is_wildcard {
                        self.has_wildcard_phantom = true;
                    }
                }
                _ => {} // Ignore other declaration types for now
            }
        }
        Ok(())
    }

    // Logic to collect information from implementation blocks
    pub fn collect_impls(&mut self, decls: &[Decl], prefix: &str) -> Result<(), CompilerError> {
        for decl in decls {
            match decl {
                Decl::Impl(imp) => {
                    let mut full_target = if prefix.is_empty() {
                        imp.target.clone()
                    } else {
                        if imp.target.contains("::") {
                            imp.target.clone()
                        } else {
                            format!("{}::{}", prefix, imp.target)
                        }
                    };

                    // Handle generic impl targets (e.g., `impl<T> MyObj<T>`)
                    // This assumes `MyObj<>` is already registered in `self.objects` or `self.enums`
                    if !imp.generics.is_empty() && !full_target.ends_with("<>") {
                        full_target = format!("{}<>", full_target);
                    }

                    // Store generic parameters of the impl block and its functions
                    let old_gens = self.generic_params.clone(); // Save current generic params
                    for (name, bounds) in &imp.generics {
                        self.generic_params.insert(name.clone(), bounds.clone());
                    }

                    for func in &imp.functions {
                        // Store method signatures, namespaced by the target type
                        let method_full_name = format!("{}::{}", full_target, func.name);
                        let sig_params: Vec<_> = func
                            .params
                            .iter()
                            .map(|p| {
                                let ty = if p.name == "self" {
                                    // Resolve self type
                                    if full_target.contains('<') {
                                        let parts: Vec<_> = full_target.split('<').collect();
                                        let name = parts[0].to_string();
                                        Type::Custom(name, Vec::new()) // Simplified generics for now
                                    } else {
                                        Type::Custom(full_target.clone(), Vec::new())
                                    }
                                } else {
                                    self.resolve_type(&p.ty, prefix)
                                };
                                let kind = match &ty {
                                    Type::Ref(_, true) => ArgKind::MutRef,
                                    Type::Ref(_, false) => ArgKind::Ref,
                                    _ if p.is_mutable => ArgKind::MutRef,
                                    _ => ArgKind::Value,
                                };
                                (ty, kind)
                            })
                            .collect();
                        let sig_ret = func.return_type.as_ref().map(|t| self.resolve_type(t, prefix));
                        //println!("DEBUG collect_impls added method: {}", method_full_name);
                        self.functions.insert(method_full_name, (sig_params, sig_ret));

                        // Also store generic params for functions within the impl block
                        let old_func_gens = self.generic_params.clone();
                        for (name, bounds) in &func.generics {
                            self.generic_params.insert(name.clone(), bounds.clone());
                        }
                        // Restore generic params after processing func
                        self.generic_params = old_func_gens;
                    }
                    // Restore generic params after processing impl block
                    self.generic_params = old_gens;
                }
                Decl::Module(name, inner) => {
                    let new_prefix = if prefix.is_empty() {
                        name.clone()
                    } else {
                        if name.contains("::") {
                            name.clone()
                        } else {
                            format!("{}::{}", prefix, name)
                        }
                    };
                    self.collect_impls(inner, &new_prefix)?;
                }
                _ => {} // Ignore other declaration types
            }
        }
        Ok(())
    }

    pub fn is_phantom_type(&self, name: &str) -> bool {
        if self.has_wildcard_phantom {
            return true;
        }
        
        // Resolve alias if it exists
        let actual_name = self.aliases.get(name).map(|s| s.as_str()).unwrap_or(name);

        if self.phantom_types.contains(actual_name) {
            return true;
        }
        // Check if the name starts with any of the known phantom types followed by ::
        for phantom_prefix in &self.phantom_types {
            if actual_name.starts_with(&format!("{}::", phantom_prefix)) {
                return true;
            }
        }
        false
    }
}
