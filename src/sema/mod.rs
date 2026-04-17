use crate::ast::*;
use crate::error::CompilerError;
use crate::lexer::Span;
use miette::SourceSpan;
use std::collections::{HashMap, HashSet};

struct TraitInfo {
    pub generics: Vec<(String, Vec<String>)>,
    pub bounds: Vec<String>,
    pub functions: HashMap<String, (Vec<Type>, Option<Type>)>,
}

pub struct SemanticAnalyzer {
    functions: HashMap<String, (Vec<Type>, Option<Type>)>,
    objects: HashMap<String, (HashMap<String, Type>, Vec<(String, Vec<String>)>)>,
    enums: HashMap<String, (HashMap<String, Vec<Type>>, Vec<(String, Vec<String>)>)>,
    traits: HashMap<String, TraitInfo>,
    generic_params: HashMap<String, Vec<String>>,
    symbols: HashMap<String, (Type, bool)>,
    phantom_types: HashSet<String>,
    aliases: HashMap<String, String>,
    has_wildcard_phantom: bool,
    current_obj: Option<String>,
    current_prefix: String,
    scope_depth: usize,
    var_declarations: HashMap<String, (usize, bool)>, // (depth, is_explicit_alloc)
    await_points: Vec<usize>, // depth of currently active awaits
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        let mut sa = Self {
            functions: HashMap::new(),
            objects: HashMap::new(),
            enums: HashMap::new(),
            traits: HashMap::new(),
            generic_params: HashMap::new(),
            symbols: HashMap::new(),
            phantom_types: HashSet::new(),
            aliases: HashMap::new(),
            has_wildcard_phantom: false,
            current_obj: None,
            current_prefix: String::new(),
            scope_depth: 0,
            var_declarations: HashMap::new(),
            await_points: Vec::new(),
        };
        
        sa.functions.insert("mem::free".to_string(), (vec![Type::Ref(Box::new(Type::RawPtr(Box::new(Type::Any), true)), false), Type::Ref(Box::new(Type::I32), false)], None));
        sa.functions.insert("mem::alloc<>".to_string(), (vec![Type::I32], Some(Type::RawPtr(Box::new(Type::Any), true))));
        sa.functions.insert("fs::read_to_string".to_string(), (vec![Type::Str], Some(Type::String)));
        sa.functions.insert("io::readline".to_string(), (vec![], Some(Type::String)));
        sa.functions.insert("io::println".to_string(), (vec![Type::Str], None));
        
        sa.functions.insert("str::len".to_string(), (vec![Type::Ref(Box::new(Type::Str), false)], Some(Type::I32)));
        sa.functions.insert("str::contains".to_string(), (vec![Type::Ref(Box::new(Type::Str), false), Type::Str], Some(Type::Bool)));
        sa.functions.insert("str::split".to_string(), (vec![Type::Ref(Box::new(Type::Str), false), Type::Str], Some(Type::Custom("std::vec::Vector<>".into(), vec![Type::String]))));

        sa.functions.insert("string::new".to_string(), (vec![], Some(Type::String)));
        sa.functions.insert("string::len".to_string(), (vec![Type::Ref(Box::new(Type::String), false)], Some(Type::I32)));
        sa.functions.insert("string::contains".to_string(), (vec![Type::Ref(Box::new(Type::String), false), Type::Str], Some(Type::Bool)));
        sa.functions.insert("string::split".to_string(), (vec![Type::Ref(Box::new(Type::String), false), Type::Str], Some(Type::Custom("std::vec::Vector<>".into(), vec![Type::String]))));
        sa.functions.insert("string::append".to_string(), (vec![Type::Ref(Box::new(Type::String), true), Type::Str], None));

        sa
    }

    pub fn analyze(&mut self, program: &mut Program) -> Result<(), CompilerError> {
        self.collect_decls(&program.declarations, "")?;
        self.collect_impls(&program.declarations, "")?;
        self.analyze_decls(&mut program.declarations, "")?;
        Ok(())
    }

    fn collect_impls(&mut self, decls: &[Decl], prefix: &str) -> Result<(), CompilerError> {
        let old_prefix = self.current_prefix.clone();
        self.current_prefix = prefix.to_string();
        for decl in decls {
            match decl {
                Decl::Impl(imp) => {
                    let mut full_target = if prefix.is_empty() { imp.target.clone() } else { 
                        if imp.target.contains("::") { imp.target.clone() } else { format!("{}::{}", prefix, imp.target) }
                    };

                    // Check if the target object is known to be generic
                    if self.objects.contains_key(&format!("{}<>", full_target)) || self.enums.contains_key(&format!("{}<>", full_target)) {
                        if !full_target.ends_with("<>") {
                            full_target = format!("{}<>", full_target);
                        }
                    }

                    for func in &imp.functions { 
                        // Set up generic params for signature resolution
                        let old_gens = self.generic_params.clone();
                        let old_obj = self.current_obj.clone();
                        let old_symbols = self.symbols.clone();
                        
                        self.current_obj = Some(full_target.clone());
                        for (name, bounds) in &imp.generics {
                            self.generic_params.insert(name.clone(), bounds.clone());
                        }
                        for (name, bounds) in &func.generics {
                            self.generic_params.insert(name.clone(), bounds.clone());
                        }

                        let self_gens = imp.generics.iter().map(|(n, _)| Type::Generic(n.clone())).collect::<Vec<_>>();
                        self.symbols.insert("self".to_string(), (Type::Custom(full_target.clone(), self_gens), false));

                        // Register the method in self.functions
                        let sig_params: Vec<_> = func.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
                        let sig_ret = func.return_type.as_ref().map(|t| self.resolve_type(t));
                        self.functions.insert(format!("{}::{}", full_target, func.name), (sig_params, sig_ret));

                        self.generic_params = old_gens;
                        self.current_obj = old_obj;
                        self.symbols = old_symbols;
                    }
                }
                Decl::Module(name, inner) => {
                    let new_prefix = if prefix.is_empty() { name.clone() } else { format!("{}::{}", prefix, name) };
                    self.collect_impls(inner, &new_prefix)?;
                }
                _ => {}
            }
        }
        self.current_prefix = old_prefix;
        Ok(())
    }

    fn semantic_error<T>(&self, message: String, span: Span) -> Result<T, CompilerError> {
        Err(CompilerError::Semantic {
            message,
            span: SourceSpan::new(span.start.into(), (span.end - span.start).into()),
        })
    }

    fn collect_decls(&mut self, decls: &[Decl], prefix: &str) -> Result<(), CompilerError> {
        for decl in decls {
            match decl {
                Decl::Object(obj) | Decl::ExternObject(obj) => {
                    let mut full_name = if prefix.is_empty() { obj.name.clone() } else { 
                        if obj.name.contains("::") { obj.name.clone() } else { format!("{}::{}", prefix, obj.name) }
                    };
                    if !obj.generics.is_empty() {
                        full_name = format!("{}<>", full_name);
                    }

                    let mut fields = HashMap::new();
                    for f in &obj.fields { fields.insert(f.name.clone(), f.ty.clone()); }
                    self.objects.insert(full_name, (fields, obj.generics.clone()));
                }
                Decl::Enum(enm) | Decl::ExternEnum(enm) => {
                    let mut full_name = if prefix.is_empty() { enm.name.clone() } else { 
                        if enm.name.contains("::") { enm.name.clone() } else { format!("{}::{}", prefix, enm.name) }
                    };
                    if !enm.generics.is_empty() {
                        full_name = format!("{}<>", full_name);
                    }
                    let mut variants = HashMap::new();
                    for v in &enm.variants { 
                        variants.insert(v.name.clone(), v.types.clone()); 
                    }
                    self.enums.insert(full_name, (variants, enm.generics.clone()));
                }
                Decl::Trait(tr) | Decl::ExternTrait(tr) => {
                    let mut full_name = if prefix.is_empty() { tr.name.clone() } else { 
                        if tr.name.contains("::") { tr.name.clone() } else { format!("{}::{}", prefix, tr.name) }
                    };
                    if !tr.generics.is_empty() {
                        full_name = format!("{}<>", full_name);
                    }
                    let mut trait_funcs = HashMap::new();
                    for f in &tr.functions {
                        let ret = f.return_type.as_ref().map(|t| self.resolve_type(t));
                        let sig_params: Vec<_> = f.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
                        let sig = (sig_params, ret);
                        trait_funcs.insert(f.name.clone(), sig.clone());
                        // Store with full namespaced trait name
                        self.functions.insert(format!("{}::{}", full_name, f.name), sig);
                    }
                    self.traits.insert(full_name, TraitInfo {
                        generics: tr.generics.clone(),
                        bounds: tr.bounds.clone(),
                        functions: trait_funcs,
                    });
                }
                Decl::Function(func) | Decl::ExternFunction(func) => {
                    let full_name = if prefix.is_empty() { func.name.clone() } else { 
                        if func.name.contains("::") { func.name.clone() } else { format!("{}::{}", prefix, func.name) }
                    };
                    let mut final_name = full_name;
                    if !func.generics.is_empty() {
                        final_name = format!("{}<>", final_name);
                    }

                    // Set up generic params for signature resolution
                    let old_gens = self.generic_params.clone();
                    for (name, bounds) in &func.generics {
                        self.generic_params.insert(name.clone(), bounds.clone());
                    }

                    let ret = func.return_type.as_ref().map(|t| self.resolve_type(t));
                    let params: Vec<_> = func.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
                    self.functions.insert(final_name, (params, ret));

                    self.generic_params = old_gens;
                }
                Decl::Module(name, inner) => {
                    let new_prefix = if prefix.is_empty() { name.clone() } else { 
                        if name.contains("::") { name.clone() } else { format!("{}::{}", prefix, name) }
                    };
                    if new_prefix.contains("::") {
                        let parts: Vec<&str> = new_prefix.split("::").collect();
                        let last = parts.last().unwrap();
                        self.aliases.insert(last.to_string(), new_prefix.clone());
                    }
                    self.collect_decls(inner, &new_prefix)?;
                }
                Decl::Use(u) => {
                    if u.is_crate {
                        self.phantom_types.insert(u.path[0].clone());
                        if !u.path.is_empty() {
                            let full_path = u.path.join("::");
                            let last = u.path.last().unwrap();
                            self.aliases.insert(last.clone(), full_path);
                        }
                    } else if !u.path.is_empty() {
                        let full_path = u.path.join("::");
                        let last = u.path.last().unwrap();
                        self.aliases.insert(last.clone(), full_path);
                    }
                    if u.is_wildcard {
                        self.has_wildcard_phantom = true;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn resolve_path(&self, path: &[PathPart]) -> String {
        let mut full_path = String::new();
        for (i, part) in path.iter().enumerate() {
            if i > 0 { full_path.push_str("::"); }
            
            if i == 0 {
                // If it's a simple name, check if it's a known variable first to allow shadowing module aliases
                if path.len() == 1 && (self.symbols.contains_key(&part.name) || self.var_declarations.contains_key(&part.name)) {
                    full_path.push_str(&part.name);
                } else if let Some(alias) = self.aliases.get(&part.name) {
                    full_path.push_str(alias);
                } else {
                    let prefixed = if self.current_prefix.is_empty() { part.name.clone() } else { format!("{}::{}", self.current_prefix, part.name) };
                    if self.objects.contains_key(&prefixed) || self.objects.contains_key(&format!("{}<>", prefixed)) ||
                       self.enums.contains_key(&prefixed) || self.enums.contains_key(&format!("{}<>", prefixed)) ||
                       self.traits.contains_key(&prefixed) || self.traits.contains_key(&format!("{}<>", prefixed)) ||
                       self.functions.contains_key(&prefixed) || self.functions.contains_key(&format!("{}<>", prefixed)) {
                        full_path.push_str(&prefixed);
                    } else {
                        full_path.push_str(&part.name);
                    }
                }
            } else {
                full_path.push_str(&part.name);
            }
            
            if !part.generics.is_empty() || self.objects.contains_key(&format!("{}<>", full_path)) || self.enums.contains_key(&format!("{}<>", full_path)) || self.traits.contains_key(&format!("{}<>", full_path)) {
                if !full_path.ends_with("<>") {
                    full_path.push_str("<>");
                }
            }
        }

        full_path
    }

    fn analyze_decls(&mut self, decls: &mut [Decl], prefix: &str) -> Result<(), CompilerError> {
        self.current_prefix = prefix.to_string();
        for decl in decls {
            match decl {
                Decl::Function(func) => {
                    self.analyze_function(func, None, None)?;
                }
                Decl::Impl(imp) => {
                    let mut full_target = if prefix.is_empty() { imp.target.clone() } else { 
                        if imp.target.contains("::") { imp.target.clone() } else { format!("{}::{}", prefix, imp.target) }
                    };

                    // Check if the target object is known to be generic
                    if self.objects.contains_key(&format!("{}<>", full_target)) || self.enums.contains_key(&format!("{}<>", full_target)) {
                        if !full_target.ends_with("<>") {
                            full_target = format!("{}<>", full_target);
                        }
                    }

                    if let Some(trait_name) = &imp.trait_name {
                        let full_trait = if prefix.is_empty() { trait_name.clone() } else {
                            if trait_name.contains("::") { trait_name.clone() } else { format!("{}::{}", prefix, trait_name) }
                        };
                        
                        if let Some(tr_info) = self.traits.get(&full_trait) {
                            for (f_name, (f_params, _)) in &tr_info.functions {
                                let mut found = false;
                                for imp_func in &imp.functions {
                                    if &imp_func.name == f_name {
                                        found = true;
                                        if imp_func.params.len() != f_params.len() {
                                            return self.semantic_error(format!("Method '{}' in impl of trait '{}' for '{}' has wrong number of parameters", f_name, full_trait, full_target), imp_func.body.span);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    for func in &mut imp.functions { 
                        // Set up self for analysis
                        let old_symbols = self.symbols.clone();
                        let self_gens = imp.generics.iter().map(|(n, _)| Type::Generic(n.clone())).collect::<Vec<_>>();
                        self.symbols.insert("self".to_string(), (Type::Custom(full_target.clone(), self_gens), false));

                        self.analyze_function(func, Some(&full_target), Some(&imp.generics))?; 

                        self.symbols = old_symbols;
                    }
                }
                Decl::Module(name, inner) => {
                    let new_prefix = if prefix.is_empty() { name.clone() } else { format!("{}::{}", prefix, name) };
                    self.analyze_decls(inner, &new_prefix)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn resolve_type(&mut self, ty: &Type) -> Type {
        match ty {
            Type::BoxPtr(inner) => Type::BoxPtr(Box::new(self.resolve_type(inner))),
            Type::RawPtr(inner, mutable) => Type::RawPtr(Box::new(self.resolve_type(inner)), *mutable),
            Type::Ref(inner, mutable) => Type::Ref(Box::new(self.resolve_type(inner)), *mutable),
            Type::Managed(inner) => Type::Managed(Box::new(self.resolve_type(inner))),
            Type::ThreadSafe(inner) => Type::ThreadSafe(Box::new(self.resolve_type(inner))),
            Type::WeakManaged(inner) => Type::WeakManaged(Box::new(self.resolve_type(inner))),
            Type::WeakThreadSafe(inner) => Type::WeakThreadSafe(Box::new(self.resolve_type(inner))),
            Type::Array(inner, size) => Type::Array(Box::new(self.resolve_type(inner)), *size),
            Type::SelfType => {
                if let Some((ty, _)) = self.symbols.get("self") {
                    ty.clone()
                } else if let Some(name) = &self.current_obj {
                    Type::Custom(name.clone(), Vec::new())
                }
                else { Type::SelfType }
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
                    let resolved_generics: Vec<_> = generics.iter().map(|g| self.resolve_type(g)).collect();

                    let mut name_to_lookup = name.clone();
                    if name == "Self" || name == "self" {
                        if let Some((ty, _)) = self.symbols.get("self") {
                            if let Type::Custom(n, gens) = ty {
                                let mut res_gens = gens.clone();
                                if !resolved_generics.is_empty() {
                                    res_gens = resolved_generics;
                                }
                                return Type::Custom(n.clone(), res_gens);
                            }
                        }
                        if let Some(obj_name) = &self.current_obj {
                            name_to_lookup = obj_name.clone();
                        }
                    } else if name.contains("::") {

                        let parts: Vec<&str> = name.split("::").collect();
                        if let Some(alias) = self.aliases.get(parts[0]) {
                            name_to_lookup = format!("{}::{}", alias, parts[1..].join("::"));
                        }
                    } else if let Some(alias) = self.aliases.get(name) {
                        name_to_lookup = alias.clone();
                    } else {
                        let prefixed = if self.current_prefix.is_empty() { name.clone() } else { format!("{}::{}", self.current_prefix, name) };
                        if self.objects.contains_key(&prefixed) || self.objects.contains_key(&format!("{}<>", prefixed)) ||
                           self.enums.contains_key(&prefixed) || self.enums.contains_key(&format!("{}<>", prefixed)) ||
                           self.traits.contains_key(&prefixed) || self.traits.contains_key(&format!("{}<>", prefixed)) {
                            name_to_lookup = prefixed;
                        } else {
                            name_to_lookup = name.clone();
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
            Type::Result(ok, err) => {
                Type::Result(Box::new(self.resolve_type(ok)), Box::new(self.resolve_type(err)))
            }
            _ => ty.clone(),
        }
    }

    fn deref_type(&mut self, ty: &Type) -> Type {
        match ty {
            Type::BoxPtr(inner) |
            Type::RawPtr(inner, _) |
            Type::Ref(inner, _) |
            Type::Managed(inner) |
            Type::ThreadSafe(inner) |
            Type::WeakManaged(inner) |
            Type::WeakThreadSafe(inner) => self.deref_type(inner),
            _ => ty.clone(),
        }
    }

    fn is_phantom_type(&self, name: &str) -> bool {
        if self.has_wildcard_phantom { return true; }
        if self.phantom_types.contains(name) { return true; }
        for p in &self.phantom_types {
            if name.starts_with(&format!("{}::", p)) { return true; }
        }
        false
    }

    fn analyze_function(&mut self, func: &mut Function, target: Option<&String>, target_generics: Option<&Vec<(String, Vec<String>)>>) -> Result<(), CompilerError> {
        self.symbols.clear();
        self.var_declarations.clear();
        self.await_points.clear();
        self.generic_params.clear();
        self.scope_depth = 0;
        self.current_obj = target.cloned();

        if let Some(t_gens) = target_generics {
            for (name, bounds) in t_gens {
                self.generic_params.insert(name.clone(), bounds.clone());
            }
        }
        for (name, bounds) in &func.generics { self.generic_params.insert(name.clone(), bounds.clone()); }

        if let Some(t) = target {
            let self_gens = target_generics.map(|gs| gs.iter().map(|(n, _)| Type::Generic(n.clone())).collect()).unwrap_or_default();
            self.symbols.insert("self".to_string(), (Type::Custom(t.clone(), self_gens), false));
        }

        for p in &func.params {
            let ty = self.resolve_type(&p.ty);
            self.symbols.insert(p.name.clone(), (ty, false));
        }
        self.analyze_stmt(&mut func.body)?;
        self.refine_stmt(&mut func.body)?;
        self.current_obj = None;
        Ok(())
    }

    fn promote_variable(&mut self, name: &str, thread_safe: bool) {
        if let Some((ty, _)) = self.symbols.get_mut(name) {
            let new_ty = match ty {
                Type::Managed(inner) if thread_safe => Type::ThreadSafe(inner.clone()),
                Type::WeakManaged(inner) if thread_safe => Type::WeakThreadSafe(inner.clone()),
                _ => return,
            };
            *ty = new_ty;
        }
    }

    fn detect_escape(&mut self, expr: &Expr) {
        if let ExprKind::Variable(path) = &expr.kind {
            let name = self.path_to_string(path);
            if let Some((ty, _)) = self.symbols.get_mut(&name) {
                if !matches!(ty, Type::Managed(_) | Type::ThreadSafe(_) | Type::WeakManaged(_) | Type::WeakThreadSafe(_)) {
                    *ty = Type::Managed(Box::new(ty.clone()));
                }
            }
        }
    }

    fn check_aliasing(&mut self, _args: &[Expr], _aks: &[ArgKind], _span: Span) -> Result<(), CompilerError> {
        Ok(())
    }

    fn types_equal(&self, a: &Type, b: &Type) -> bool {
        match (a, b) {
            (Type::Any, _) | (_, Type::Any) => true,
            (Type::Str, Type::String) | (Type::String, Type::Str) => true,
            (Type::BoxPtr(a), Type::BoxPtr(b)) => self.types_equal(a, b),
            (Type::RawPtr(a, m1), Type::RawPtr(b, m2)) => m1 == m2 && self.types_equal(a, b),
            (Type::Ref(a, m1), Type::Ref(b, m2)) => m1 == m2 && self.types_equal(a, b),
            (Type::Managed(a), Type::Managed(b)) => self.types_equal(a, b),
            (Type::ThreadSafe(a), Type::ThreadSafe(b)) => self.types_equal(a, b),
            (Type::WeakManaged(a), Type::WeakManaged(b)) => self.types_equal(a, b),
            (Type::WeakThreadSafe(a), Type::WeakThreadSafe(b)) => self.types_equal(a, b),
            (Type::Array(a, s1), Type::Array(b, s2)) => s1 == s2 && self.types_equal(a, b),
            (Type::Result(a1, a2), Type::Result(b1, b2)) => self.types_equal(a1, b1) && self.types_equal(a2, b2),
            (Type::Custom(n1, g1), Type::Custom(n2, g2)) => {
                if n1 != n2 || g1.len() != g2.len() { return false; }
                for (t1, t2) in g1.iter().zip(g2) {
                    if !self.types_equal(t1, t2) { return false; }
                }
                true
            }
            _ => a == b,
        }
    }

    fn substitute_generics(&self, ty: &Type, params: &Vec<(String, Vec<String>)>, args: &Vec<Type>) -> Type {
        match ty {
            Type::Generic(name) => {
                for (i, (p_name, _)) in params.iter().enumerate() {
                    if p_name == name {
                        if let Some(arg) = args.get(i) {
                            return arg.clone();
                        }
                    }
                }
                ty.clone()
            }
            Type::BoxPtr(inner) => Type::BoxPtr(Box::new(self.substitute_generics(inner, params, args))),
            Type::RawPtr(inner, m) => Type::RawPtr(Box::new(self.substitute_generics(inner, params, args)), *m),
            Type::Ref(inner, m) => Type::Ref(Box::new(self.substitute_generics(inner, params, args)), *m),
            Type::Managed(inner) => Type::Managed(Box::new(self.substitute_generics(inner, params, args))),
            Type::ThreadSafe(inner) => Type::ThreadSafe(Box::new(self.substitute_generics(inner, params, args))),
            Type::WeakManaged(inner) => Type::WeakManaged(Box::new(self.substitute_generics(inner, params, args))),
            Type::WeakThreadSafe(inner) => Type::WeakThreadSafe(Box::new(self.substitute_generics(inner, params, args))),
            Type::Array(inner, s) => Type::Array(Box::new(self.substitute_generics(inner, params, args)), *s),
            Type::Result(ok, err) => Type::Result(
                Box::new(self.substitute_generics(ok, params, args)),
                Box::new(self.substitute_generics(err, params, args))
            ),
            Type::Custom(name, gens) => {
                let sub_gens = gens.iter().map(|g| self.substitute_generics(g, params, args)).collect();
                Type::Custom(name.clone(), sub_gens)
            }
            _ => ty.clone(),
        }
    }

    fn match_generics(&self, p_ty: &Type, a_ty: &Type, generics: &Vec<(String, Vec<String>)>, inferred: &mut Vec<Type>) {
        match (p_ty, a_ty) {
            (Type::Generic(name), _) => {
                for (i, (g_name, _)) in generics.iter().enumerate() {
                    if g_name == name {
                        if inferred[i] == Type::Any {
                            inferred[i] = a_ty.clone();
                        }
                    }
                }
            }
            (Type::BoxPtr(p), Type::BoxPtr(a)) => self.match_generics(p, a, generics, inferred),
            (Type::RawPtr(p, _), Type::RawPtr(a, _)) => self.match_generics(p, a, generics, inferred),
            (Type::Ref(p, _), Type::Ref(a, _)) => self.match_generics(p, a, generics, inferred),
            (Type::Ref(p, _), a) => self.match_generics(p, a, generics, inferred),
            (p, Type::Ref(a, _)) => self.match_generics(p, a, generics, inferred),
            (Type::Managed(p), Type::Managed(a)) => self.match_generics(p, a, generics, inferred),
            (Type::ThreadSafe(p), Type::ThreadSafe(a)) => self.match_generics(p, a, generics, inferred),
            (Type::Custom(_, p_gens), Type::Custom(_, a_gens)) => {
                for (pg, ag) in p_gens.iter().zip(a_gens) {
                    self.match_generics(pg, ag, generics, inferred);
                }
            }
            _ => {}
        }
    }

    fn analyze_stmt(&mut self, stmt: &mut Stmt) -> Result<(), CompilerError> {
        let span = stmt.span;
        match &mut stmt.kind {
            StmtKind::VarDecl { name, is_mutable: _, ty, value } => {
                self.var_declarations.insert(name.clone(), (self.scope_depth, false));
                let val_ty = self.analyze_expr(value)?;
                let expected_ty = ty.as_ref().map(|t| self.resolve_type(t));
                if let Some(et) = expected_ty {
                    if !self.types_equal(&et, &val_ty) {
                        return self.semantic_error(format!("Type mismatch in declaration: expected {:?}, found {:?}", et, val_ty), span);
                    }
                }
                self.symbols.insert(name.clone(), (val_ty, false));
                Ok(())
            }
            StmtKind::Assign { target, value } => {
                let t_ty = self.analyze_expr(target)?;
                let v_ty = self.analyze_expr(value)?;
                if !self.types_equal(&t_ty, &v_ty) {
                    return self.semantic_error(format!("Type mismatch in assignment: {:?} and {:?}", t_ty, v_ty), span);
                }
                Ok(())
            }
            StmtKind::If { condition, then_branch, else_branch } => {
                let cond_ty = self.analyze_expr(condition)?;
                if !self.types_equal(&cond_ty, &Type::Bool) {
                    return self.semantic_error(format!("If condition must be bool, found {:?}", cond_ty), span);
                }
                self.analyze_stmt(then_branch)?;
                if let Some(eb) = else_branch { self.analyze_stmt(eb)?; }
                Ok(())
            }
            StmtKind::While { condition, body } => {
                let cond_ty = self.analyze_expr(condition)?;
                if !self.types_equal(&cond_ty, &Type::Bool) {
                    return self.semantic_error(format!("While condition must be bool, found {:?}", cond_ty), span);
                }
                self.analyze_stmt(body)?;
                Ok(())
            }
            StmtKind::Loop { body } => { self.analyze_stmt(body)?; Ok(()) }
            StmtKind::Block(stmts) | StmtKind::UnsafeBlock(stmts) => {
                self.scope_depth += 1;
                for s in stmts { self.analyze_stmt(s)?; }
                self.scope_depth -= 1;
                Ok(())
            }
            StmtKind::ExprStmt(expr) => { self.analyze_expr(expr)?; Ok(()) }
            StmtKind::Return(expr) => {
                if let Some(e) = expr { 
                    self.analyze_expr(e)?; 
                    self.detect_escape(e);
                }
                Ok(())
            }
            StmtKind::Match { expr, arms } => {
                let expr_ty = self.analyze_expr(expr)?;
                for arm in arms {
                    let old_symbols = self.symbols.clone();
                    self.scope_depth += 1;
                    self.analyze_pattern(&mut arm.pattern, &expr_ty, span)?;
                    self.analyze_stmt(&mut arm.body)?;
                    self.scope_depth -= 1;
                    self.symbols = old_symbols;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn refine_stmt(&mut self, stmt: &mut Stmt) -> Result<(), CompilerError> {
        match &mut stmt.kind {
            StmtKind::VarDecl { name: _, value, .. } => {
                self.refine_expr(value)?;
            }
            StmtKind::Assign { target, value } => {
                self.refine_expr(target)?;
                self.refine_expr(value)?;
            }
            StmtKind::If { condition, then_branch, else_branch } => {
                self.refine_expr(condition)?;
                self.refine_stmt(then_branch)?;
                if let Some(eb) = else_branch { self.refine_stmt(eb)?; }
            }
            StmtKind::While { condition, body } => {
                self.refine_expr(condition)?;
                self.refine_stmt(body)?;
            }
            StmtKind::Loop { body } => {
                self.refine_stmt(body)?;
            }
            StmtKind::Block(stmts) | StmtKind::UnsafeBlock(stmts) => {
                for s in stmts { self.refine_stmt(s)?; }
            }
            StmtKind::ExprStmt(expr) => { self.refine_expr(expr)?; }
            StmtKind::Return(expr) => {
                if let Some(e) = expr { self.refine_expr(e)?; }
            }
            _ => {}
        }
        Ok(())
    }

    fn refine_expr(&mut self, expr: &mut Expr) -> Result<(), CompilerError> {
        match &mut expr.kind {
            ExprKind::Binary(lhs, _, rhs) => {
                self.refine_expr(lhs)?;
                self.refine_expr(rhs)?;
            }
            ExprKind::Call(_, args, _, _) | ExprKind::MacroCall(_, args) => {
                for arg in args { self.refine_expr(arg)?; }
            }
            ExprKind::MethodCall(lhs, _, args, _, _) => {
                self.refine_expr(lhs)?;
                for arg in args { self.refine_expr(arg)?; }
            }
            ExprKind::StructLiteral { fields, .. } => {
                for (_, fexpr) in fields { self.refine_expr(fexpr)?; }
            }
            ExprKind::MemberAccess(lhs, _) | ExprKind::IndexAccess(lhs, _) | ExprKind::Alloc(lhs, _) | ExprKind::Borrow(lhs, _) | ExprKind::Deref(lhs) | ExprKind::Downgrade(lhs) | ExprKind::Negate(lhs) | ExprKind::Unwrap(lhs) | ExprKind::Await(lhs) | ExprKind::Try(lhs) | ExprKind::Cast(lhs, _) => {
                self.refine_expr(lhs)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn analyze_pattern(&mut self, pattern: &mut Pattern, expr_ty: &Type, span: Span) -> Result<(), CompilerError> {
        match pattern {
            Pattern::Variant(enum_name, variant_name, params) => {
                if enum_name.is_empty() {
                    match expr_ty {
                        Type::Result(ok, err) => {
                            if variant_name == "Ok" {
                                *enum_name = "Result".into();
                                if params.len() == 1 {
                                    self.symbols.insert(params[0].clone(), (*ok.clone(), false));
                                }
                                return Ok(());
                            } else if variant_name == "Err" {
                                *enum_name = "Result".into();
                                if params.len() == 1 {
                                    self.symbols.insert(params[0].clone(), (*err.clone(), false));
                                }
                                return Ok(());
                            }
                        }
                        Type::Custom(n, g) if n == "Result" && g.len() == 2 => {
                            if variant_name == "Ok" {
                                *enum_name = "Result".into();
                                if params.len() == 1 {
                                    self.symbols.insert(params[0].clone(), (g[0].clone(), false));
                                }
                                return Ok(());
                            } else if variant_name == "Err" {
                                *enum_name = "Result".into();
                                if params.len() == 1 {
                                    self.symbols.insert(params[0].clone(), (g[1].clone(), false));
                                }
                                return Ok(());
                            }
                        }
                        Type::Custom(n, _) => { *enum_name = n.clone(); }
                        Type::Any => {
                             for name in params { self.symbols.insert(name.clone(), (Type::Any, false)); }
                             return Ok(());
                        }
                        _ => {}
                    }
                }

                let mut found_params = None;
                if let Some((variants, _)) = self.enums.get(enum_name) {
                    if let Some(param_types) = variants.get(variant_name) {
                        found_params = Some(param_types.clone());
                    }
                }

                if let Some(param_types) = found_params {
                    for (name, ty) in params.iter().zip(param_types) { 
                        let resolved = self.resolve_type(&ty);
                        self.symbols.insert(name.clone(), (resolved, false)); 
                    }
                    return Ok(());
                }
                
                if self.phantom_types.contains(enum_name) || self.has_wildcard_phantom {
                     for name in params { self.symbols.insert(name.clone(), (Type::Any, false)); }
                     return Ok(());
                }

                self.semantic_error(format!("Undeclared variant {}::{}", enum_name, variant_name), span)
            }
            Pattern::Variable(name) => { self.symbols.insert(name.clone(), (expr_ty.clone(), false)); Ok(()) }
            Pattern::Literal(lit) => { self.analyze_expr(lit)?; Ok(()) }
        }
    }

    fn path_to_string(&self, path: &[PathPart]) -> String {
        path.iter().map(|p| {
            if p.generics.is_empty() {
                p.name.clone()
            } else {
                format!("{}<>", p.name)
            }
        }).collect::<Vec<_>>().join("::")
    }

    fn analyze_expr(&mut self, expr: &mut Expr) -> Result<Type, CompilerError> {
        let span = expr.span;
        let ty = match &mut expr.kind {
            ExprKind::Unit => Ok(Type::Unit),
            ExprKind::Int(_) => Ok(Type::I32),
            ExprKind::Int64(_) => Ok(Type::I64),
            ExprKind::Float(_) => Ok(Type::F32),
            ExprKind::Float64(_) => Ok(Type::F64),
            ExprKind::Bool(_) => Ok(Type::Bool),
            ExprKind::String(_) => Ok(Type::Str),
            ExprKind::Variable(path) => {
                let name = self.resolve_path(path);
                let await_promoted = if let Some((depth, _explicit)) = self.var_declarations.get(&name) {
                    self.await_points.iter().any(|&aw| aw >= *depth)
                } else { false };
                
                if await_promoted {
                    self.promote_variable(&name, true);
                }

                if let Some((ty, _)) = self.symbols.get(&name) { Ok(ty.clone()) }
                else if self.functions.contains_key(&name) { Ok(Type::Any) }
                else {
                    let is_phantom = path.iter().any(|p| self.phantom_types.contains(&p.name));
                    if is_phantom || self.has_wildcard_phantom {
                        Ok(Type::Any)
                    } else {
                        self.semantic_error(format!("Undeclared variable '{}'", name), span)
                    }
                }
            }
            ExprKind::Binary(lhs, op, rhs) => {
                let l = self.analyze_expr(lhs)?;
                let r = self.analyze_expr(rhs)?;
                
                if l == Type::Any || r == Type::Any {
                    Ok(Type::Any)
                } else {
                    if *op == BinaryOp::Add {
                        let is_l_string = l == Type::String || l == Type::Str;
                        let is_r_string = r == Type::String || r == Type::Str;
                        
                        if is_l_string || is_r_string {
                            return Ok(Type::String);
                        }
                    }

                    if !self.types_equal(&l, &r) {
                        return self.semantic_error(format!("Type mismatch in binary operation: {:?} and {:?}", l, r), span);
                    }
                    match op {
                        BinaryOp::GreaterThan | BinaryOp::LessThan | BinaryOp::GreaterThanOrEqual | BinaryOp::LessThanOrEqual | BinaryOp::Equal => Ok(Type::Bool),
                        BinaryOp::Add => Ok(l),
                        _ => Ok(l),
                    }
                }
            }
            ExprKind::MacroCall(name, args) => {
                for arg in args {
                    self.analyze_expr(arg)?;
                }
                if name == "typeof" {
                    Ok(Type::Str)
                } else if name == "str" {
                    Ok(Type::String) // str!() returns owned string by default
                } else {
                    Ok(Type::I32) // Macros return i32/Unit effectively for now
                }
            }
            ExprKind::Call(path, args, resolved_name, arg_kinds) => {
                let name = self.resolve_path(path);
                if name == "Ok" {
                    let val_ty = self.analyze_expr(&mut args[0])?;
                    Ok(Type::Result(Box::new(val_ty), Box::new(Type::Generic("E".into()))))
                } else if name == "Err" {
                    let err_ty = self.analyze_expr(&mut args[0])?;
                    Ok(Type::Result(Box::new(Type::Generic("T".into())), Box::new(err_ty)))
                } else if name == "array_init" {
                    let val_ty = self.analyze_expr(&mut args[0])?;
                    Ok(Type::RawPtr(Box::new(val_ty), true))
                } else {
                    // Try looking up with prefix if it's a simple name
                    let name_to_lookup = if !name.contains("::") && !self.current_prefix.is_empty() {
                        format!("{}::{}", self.current_prefix, name)
                    } else {
                        name.clone()
                    };

                    let (found_name, ret_type) = if let Some((param_types, rt)) = self.functions.get(&name_to_lookup) {
                        let mut aks = Vec::new();
                        for pt in param_types { 
                            aks.push(match pt {
                                Type::Ref(_, true) => ArgKind::MutRef,
                                Type::Ref(_, false) => ArgKind::Ref,
                                _ => ArgKind::Value,
                            }); 
                        }
                        *arg_kinds = Some(aks);
                        (name_to_lookup.clone(), rt.clone())
                    } else if let Some((param_types, rt)) = self.functions.get(&name) {
                        let mut aks = Vec::new();
                        for pt in param_types { 
                            aks.push(match pt {
                                Type::Ref(_, true) => ArgKind::MutRef,
                                Type::Ref(_, false) => ArgKind::Ref,
                                _ => ArgKind::Value,
                            }); 
                        }
                        *arg_kinds = Some(aks);
                        (name.clone(), rt.clone())
                    } else {
                        let is_phantom = path.iter().any(|p| self.phantom_types.contains(&p.name));
                        if is_phantom {
                            for arg in args.iter_mut() { self.analyze_expr(arg)?; }
                            *resolved_name = Some(name.clone());
                            let ty = Type::Any;
                            expr.ty = Some(ty.clone());
                            return Ok(ty);
                        } else if self.has_wildcard_phantom {
                            for arg in args.iter_mut() { self.analyze_expr(arg)?; }
                            *resolved_name = Some(name.clone());
                            let ty = Type::Any;
                            expr.ty = Some(ty.clone());
                            return Ok(ty);
                        }
                        return self.semantic_error(format!("Undeclared function or variant '{}'", name), span);
                    };

                    *resolved_name = Some(found_name.clone());

                    let mut arg_types = Vec::new();
                    for arg in args.iter_mut() {
                        arg_types.push(self.analyze_expr(arg)?);
                    }

                    let mut ret = ret_type.unwrap_or(Type::Unit);
                    if found_name.contains("::") {
                        let parts: Vec<&str> = found_name.split("::").collect();
                        // Find the object name by taking all parts except the last one (the function)
                        let mut obj_name = parts[..parts.len()-1].join("::");
                        
                        // Perform generic substitution
                        let mut all_gens = Vec::new();
                        for part in path {
                            for g in &part.generics {
                                all_gens.push(self.resolve_type(g));
                            }
                        }

                        if !obj_name.ends_with("<>") && self.objects.contains_key(&format!("{}<>", obj_name)) {
                            obj_name = format!("{}<>", obj_name);
                        }

                        if let Some((_, obj_gens)) = self.objects.get(&obj_name) {
                            if all_gens.is_empty() && !obj_gens.is_empty() {
                                // Infer from arguments
                                let mut inferred = vec![Type::Any; obj_gens.len()];
                                if let Some((param_types, _)) = self.functions.get(&found_name) {
                                    for (p_ty, a_ty) in param_types.iter().zip(&arg_types) {
                                        self.match_generics(p_ty, a_ty, obj_gens, &mut inferred);
                                    }
                                }
                                all_gens = inferred;
                            }

                            if !all_gens.is_empty() && !obj_gens.is_empty() {
                                let old_obj = self.current_obj.take();
                                self.current_obj = Some(obj_name.clone());
                                ret = self.substitute_generics(&ret, obj_gens, &all_gens);
                                self.current_obj = old_obj;
                            }
                        }
                    } else {
                        ret = self.resolve_type(&ret);
                    }
                    
                    if let Some(aks) = arg_kinds {
                        self.check_aliasing(args, aks, span)?;
                    }
                    Ok(ret)
                }
            }
            ExprKind::MethodCall(lhs, name, args, resolved_obj_name, arg_kinds) => {
                let mut lhs_ty = self.analyze_expr(lhs)?;
                lhs_ty = self.resolve_type(&lhs_ty);
                if lhs_ty == Type::Any {
                    for arg in args.iter_mut() { self.analyze_expr(arg)?; }
                    Ok(Type::Any)
                } else if name == "clone" {
                    Ok(lhs_ty)
                } else {
                    let actual_ty = self.deref_type(&lhs_ty);
                    let (obj_name, _is_array) = match &lhs_ty {
                        Type::RawPtr(_, _) => {
                             // Allow arbitrary methods on raw pointers (like .add) for now
                             // We'll treat them as Any to bypass checks
                             for arg in args.iter_mut() { self.analyze_expr(arg)?; }
                             *resolved_obj_name = Some("rawptr".to_string());
                             return Ok(Type::Any);
                        }
                        _ => {
                            match &actual_ty {
                                Type::Str => ("str".to_string(), false),
                                Type::String => ("string".to_string(), false),
                                Type::I32 => ("i32".to_string(), false),
                                Type::I64 => ("i64".to_string(), false),
                                Type::F32 => ("f32".to_string(), false),
                                Type::F64 => ("f64".to_string(), false),
                                Type::Array(_, _) => ("vector".to_string(), true),
                                Type::Custom(n, _) => (n.clone(), false),
                                Type::Generic(n) => {
                                    if let Some(bounds) = self.generic_params.get(n) {
                                        let mut found = None;
                                        for b in bounds {
                                            if let Some(tr) = self.traits.get(b) {
                                                if tr.functions.contains_key(name) {
                                                    found = Some(b.clone());
                                                    break;
                                                }
                                            }
                                        }
                                        if let Some(tr_name) = found { (tr_name, false) }
                                        else { return self.semantic_error(format!("No method '{}' found in bounds of generic type '{}'", name, n), span); }
                                    } else {
                                        return self.semantic_error(format!("No method '{}' on generic type '{}' with no bounds", name, n), span);
                                    }
                                }
                                _ => return self.semantic_error(format!("Method call '{}' on non-object type {:?}", name, lhs_ty), span),
                            }
                        }
                    };
                    
                    *resolved_obj_name = Some(obj_name.clone());

                    if self.is_phantom_type(&obj_name) {
                        for arg in args.iter_mut() { self.analyze_expr(arg)?; }
                        let ty = Type::Any;
                        expr.ty = Some(ty.clone());
                        return Ok(ty);
                    }

                    let full_name = format!("{}::{}", obj_name, name);
                    
                    let mut arg_types = Vec::new();
                    for arg in args.iter_mut() {
                        arg_types.push(self.analyze_expr(arg)?);
                    }

                    if let Some((param_types, ret_type)) = self.functions.get(&full_name).cloned() {
                        let mut aks = Vec::new();
                        let mut start_idx = 0;
                        if let Some(_first) = param_types.first() {
                            // If it's a method call, we might need to skip 'self' in param_types
                            start_idx = 1;
                        }
                        for pt in &param_types[start_idx..] {
                            aks.push(match pt {
                                Type::Ref(_, true) => ArgKind::MutRef,
                                Type::Ref(_, false) => ArgKind::Ref,
                                _ => ArgKind::Value,
                            });
                        }
                        *arg_kinds = Some(aks.clone());
                        self.check_aliasing(args, &aks, span)?;

                        let mut rt = ret_type.unwrap_or(Type::I32);
                        
                        // Infer generics for the object if it's generic
                        let mut lookup_name = obj_name.clone();
                        if !lookup_name.ends_with("<>") && self.objects.contains_key(&format!("{}<>", lookup_name)) {
                            lookup_name = format!("{}<>", lookup_name);
                        }

                        if let Some((_, obj_gens)) = self.objects.get(&lookup_name) {
                            if !obj_gens.is_empty() {
                                let mut inferred = vec![Type::Any; obj_gens.len()];
                                if let Type::Custom(_, lhs_gens) = &actual_ty {
                                    for (i, g) in lhs_gens.iter().enumerate() {
                                        if i < inferred.len() { inferred[i] = g.clone(); }
                                    }
                                }

                                for (p_ty, a_ty) in param_types.iter().zip(&arg_types) {
                                    self.match_generics(p_ty, a_ty, obj_gens, &mut inferred);
                                }
                                
                                let old_obj = self.current_obj.take();
                                self.current_obj = Some(obj_name.clone());
                                rt = self.substitute_generics(&rt, obj_gens, &inferred);
                                self.current_obj = old_obj;
                            }
                        }

                        if let Type::Generic(_) = rt {
                            if let Type::Array(inner, _) = actual_ty { Ok(*inner) }
                            else { Ok(rt) }
                        } else { Ok(rt) }
                    } else {
                        self.semantic_error(format!("No method '{}' on {}", name, obj_name), span) 
                    }
                }
            }
            ExprKind::StructLiteral { path, fields, resolved_name } => {
                let name = self.resolve_path(path);
                let is_phantom = path.iter().any(|p| self.phantom_types.contains(&p.name));
                if is_phantom || self.has_wildcard_phantom {
                    for (_, fexpr) in fields { self.analyze_expr(fexpr)?; }
                    *resolved_name = Some(name.clone());
                    let ty = Type::Any;
                    expr.ty = Some(ty.clone());
                    return Ok(ty);
                }

                let mut target_name = name.clone();
                let mut target_gens = Vec::new();

                if name == "self" || name == "Self" || name == "self<>" || name == "Self<>" {
                    if let Some((Type::Custom(n, gens), _)) = self.symbols.get("self") {
                        target_name = n.clone();
                        target_gens = gens.clone();
                    } else {
                        return self.semantic_error("Cannot use self outside impl".into(), span);
                    }
                }

                let full_name = if self.objects.contains_key(&target_name) {
                    target_name.clone()
                } else {
                    let prefixed = if self.current_prefix.is_empty() { target_name.clone() } else { format!("{}::{}", self.current_prefix, target_name) };
                    if self.objects.contains_key(&prefixed) { prefixed }
                    else { target_name.clone() }
                };
                *resolved_name = Some(full_name.clone());

                let mut all_gens = target_gens;
                for part in path {
                    for g in &part.generics {
                        all_gens.push(self.resolve_type(g));
                    }
                }

                for (fname, fexpr) in fields {
                    let fty = self.analyze_expr(fexpr)?;
                    // Check if field exists and type matches
                    let mut lookup_name = full_name.clone();
                    if !lookup_name.ends_with("<>") && self.objects.contains_key(&format!("{}<>", lookup_name)) {
                        lookup_name = format!("{}<>", lookup_name);
                    }

                    let mut found_expected = None;
                    if let Some((obj_fields, obj_gens)) = self.objects.get(&lookup_name) {
                        if let Some(expected_fty) = obj_fields.get(fname) {
                            found_expected = Some((expected_fty.clone(), obj_gens.clone()));
                        } else {
                            return self.semantic_error(format!("No member '{}' on type {}", fname, full_name), span);
                        }
                    }

                    if let Some((expected_fty, obj_gens)) = found_expected {
                        let mut resolved_expected = self.resolve_type(&expected_fty);
                        if !all_gens.is_empty() {
                            resolved_expected = self.substitute_generics(&resolved_expected, &obj_gens, &all_gens);
                        }

                        if !self.types_equal(&resolved_expected, &fty) {
                            return self.semantic_error(format!("Type mismatch for field '{}': expected {:?}, found {:?}", fname, resolved_expected, fty), span);
                        }
                    }
                    // Storing a reference in a struct field escapes it
                    self.detect_escape(fexpr);
                }
                Ok(Type::Custom(full_name, all_gens))
            }
            ExprKind::MemberAccess(lhs, name) => {
                let lhs_ty = self.analyze_expr(lhs)?;
                if lhs_ty == Type::Any { 
                    Ok(Type::Any) 
                } else {
                    let resolved_lhs = self.resolve_type(&lhs_ty);
                    let actual_ty = self.deref_type(&resolved_lhs);
                    if let Type::Custom(ref obj_name, ref actual_gens) = actual_ty {
                        if self.is_phantom_type(obj_name) {
                            Ok(Type::Any)
                        } else {
                            let mut lookup_name = obj_name.clone();
                            if !lookup_name.ends_with("<>") && self.objects.contains_key(&format!("{}<>", lookup_name)) {
                                lookup_name = format!("{}<>", lookup_name);
                            }

                            let mut found_field = None;
                            if let Some((fields, obj_gens)) = self.objects.get(&lookup_name) {
                                if let Some(fty) = fields.get(name) { 
                                    found_field = Some((fty.clone(), obj_gens.clone()));
                                }
                                else { return self.semantic_error(format!("No member '{}' on type {:?}", name, actual_ty), span); }
                            } else {
                                return self.semantic_error(format!("No member '{}' on type {:?}", name, actual_ty), span);
                            }

                            if let Some((fty, obj_gens)) = found_field {
                                let mut resolved = self.resolve_type(&fty);
                                if !actual_gens.is_empty() {
                                    resolved = self.substitute_generics(&resolved, &obj_gens, actual_gens);
                                }
                                Ok(resolved)
                            } else {
                                Ok(Type::Any)
                            }
                        }
                    } else {
                        self.semantic_error(format!("No member '{}' on type {:?}", name, actual_ty), span)
                    }
                }
            }
            ExprKind::IndexAccess(lhs, index) => {
                let lhs_ty = self.analyze_expr(lhs)?;
                self.analyze_expr(index)?;
                if lhs_ty == Type::Any { Ok(Type::Any) }
                else {
                    let actual_ty = match lhs_ty {
                        Type::BoxPtr(inner) => *inner,
                        Type::RawPtr(inner, _) => *inner,
                        Type::Ref(inner, _) => *inner,
                        Type::Managed(inner) => *inner,
                        Type::ThreadSafe(inner) => *inner,
                        Type::Array(inner, _) => *inner,
                        _ => return self.semantic_error("Indexing only works on arrays or pointers".into(), span),
                    };
                    Ok(actual_ty)
                }
            }
            ExprKind::Alloc(inner, kind) => {
                let ty = self.analyze_expr(inner)?;
                match kind {
                    AllocKind::Box => Ok(Type::BoxPtr(Box::new(ty))),
                    AllocKind::RawMut => Ok(Type::RawPtr(Box::new(ty), true)),
                    AllocKind::RawConst => Ok(Type::RawPtr(Box::new(ty), false)),
                    AllocKind::Rc => Ok(Type::Managed(Box::new(ty))),
                    AllocKind::Arc => Ok(Type::ThreadSafe(Box::new(ty))),
                }
            }
            ExprKind::Borrow(inner, mutable) => {
                let ty = self.analyze_expr(inner)?;
                Ok(Type::Ref(Box::new(ty), *mutable))
            }
            ExprKind::Deref(inner) => {
                let ty = self.analyze_expr(inner)?;
                match ty {
                    Type::Ref(inner_ty, _) => Ok(*inner_ty),
                    Type::BoxPtr(inner_ty) => Ok(*inner_ty),
                    Type::RawPtr(inner_ty, _) => Ok(*inner_ty),
                    Type::Managed(inner_ty) => Ok(*inner_ty),
                    Type::ThreadSafe(inner_ty) => Ok(*inner_ty),
                    Type::Any => Ok(Type::Any),
                    _ => self.semantic_error(format!("Cannot dereference type {:?}", ty), span),
                }
            }
            ExprKind::Downgrade(inner) => {
                let ty = self.analyze_expr(inner)?;
                match ty {
                    Type::Managed(inner_ty) => Ok(Type::WeakManaged(inner_ty)),
                    Type::ThreadSafe(inner_ty) => Ok(Type::WeakThreadSafe(inner_ty)),
                    _ => self.semantic_error(format!("Cannot downgrade non-managed type {:?}", ty), span),
                }
            }
            ExprKind::Negate(inner) => {
                let ty = self.analyze_expr(inner)?;
                if ty == Type::Any { Ok(Type::Any) }
                else {
                    match ty {
                        Type::I32 | Type::I64 | Type::F32 | Type::F64 => Ok(ty),
                        _ => self.semantic_error(format!("Cannot negate type {:?}", ty), span),
                    }
                }
            }
            ExprKind::Unwrap(inner) => {
                let ty = self.analyze_expr(inner)?;
                match ty {
                    Type::Result(ok, _) => Ok(*ok),
                    Type::WeakManaged(inner_ty) => Ok(Type::Managed(inner_ty)),
                    Type::WeakThreadSafe(inner_ty) => Ok(Type::ThreadSafe(inner_ty)),
                    _ => Ok(ty), 
                }
            }
            ExprKind::Try(inner) => {
                let ty = self.analyze_expr(inner)?;
                match ty {
                    Type::Result(ok, _) => Ok(*ok),
                    Type::Custom(ref n, ref g) if n == "Result" && g.len() == 2 => Ok(g[0].clone()),
                    _ => self.semantic_error(format!("Cannot use '?' on non-result type {:?}", ty), span),
                }
            }
            ExprKind::Await(inner) => {
                let ty = self.analyze_expr(inner)?;
                self.await_points.push(self.scope_depth);
                Ok(ty)
            }
            ExprKind::Cast(inner, ty) => {
                self.analyze_expr(inner)?;
                Ok(self.resolve_type(ty))
            }
        }?;
        expr.ty = Some(ty.clone());
        Ok(ty)
    }
}
