use crate::ast::Decl;

// pub fn collect_decls(&mut self, decls: &[Decl], prefix: &str) -> Result<(), CompilerError> {
//     for decl in decls {
//         match decl {
//             Decl::Object(obj) | Decl::ExternObject(obj) => {
//                 let mut full_name = if prefix.is_empty() {
//                     obj.name.clone()
//                 } else {
//                     if obj.name.contains("::") {
//                         obj.name.clone()
//                     } else {
//                         format!("{}::{}", prefix, obj.name)
//                     }
//                 };
//                 if !obj.generics.is_empty() {
//                     full_name = format!("{}<>", full_name);
//                 }
//
//                 let mut fields = HashMap::new();
//                 for f in &obj.fields {
//                     fields.insert(f.name.clone(), f.ty.clone());
//                 }
//                 self.objects
//                     .insert(full_name, (fields, obj.generics.clone()));
//             }
//             Decl::Enum(enm) | Decl::ExternEnum(enm) => {
//                 let mut full_name = if prefix.is_empty() {
//                     enm.name.clone()
//                 } else {
//                     if enm.name.contains("::") {
//                         enm.name.clone()
//                     } else {
//                         format!("{}::{}", prefix, enm.name)
//                     }
//                 };
//                 if !enm.generics.is_empty() {
//                     full_name = format!("{}<>", full_name);
//                 }
//                 let mut variants = HashMap::new();
//                 for v in &enm.variants {
//                     variants.insert(v.name.clone(), v.types.clone());
//                 }
//                 self.enums
//                     .insert(full_name, (variants, enm.generics.clone()));
//             }
//             Decl::Trait(tr) | Decl::ExternTrait(tr) => {
//                 let mut full_name = if prefix.is_empty() {
//                     tr.name.clone()
//                 } else {
//                     if tr.name.contains("::") {
//                         tr.name.clone()
//                     } else {
//                         format!("{}::{}", prefix, tr.name)
//                     }
//                 };
//                 if !tr.generics.is_empty() {
//                     full_name = format!("{}<>", full_name);
//                 }
//                 let mut trait_funcs = HashMap::new();
//                 for f in &tr.functions {
//                     let ret = f.return_type.as_ref().map(|t| self.resolve_type(t));
//                     let sig_params: Vec<_> =
//                         f.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
//                     let sig = (sig_params, ret);
//                     trait_funcs.insert(f.name.clone(), sig.clone());
//                     // Store with full namespaced trait name
//                     self.functions
//                         .insert(format!("{}::{}", full_name, f.name), sig);
//                 }
//                 self.traits.insert(
//                     full_name,
//                     TraitInfo {
//                         generics: tr.generics.clone(),
//                         bounds: tr.bounds.clone(),
//                         functions: trait_funcs,
//                     },
//                 );
//             }
//             Decl::Function(func) | Decl::ExternFunction(func) => {
//                 let full_name = if prefix.is_empty() {
//                     func.name.clone()
//                 } else {
//                     if func.name.contains("::") {
//                         func.name.clone()
//                     } else {
//                         format!("{}::{}", prefix, func.name)
//                     }
//                 };
//                 let mut final_name = full_name;
//                 if !func.generics.is_empty() {
//                     final_name = format!("{}<>", final_name);
//                 }
//
//                 // Set up generic params for signature resolution
//                 let old_gens = self.generic_params.clone();
//                 for (name, bounds) in &func.generics {
//                     self.generic_params.insert(name.clone(), bounds.clone());
//                 }
//
//                 let ret = func.return_type.as_ref().map(|t| self.resolve_type(t));
//                 let params: Vec<_> = func
//                     .params
//                     .iter()
//                     .map(|p| self.resolve_type(&p.ty))
//                     .collect();
//                 self.functions.insert(final_name, (params, ret));
//
//                 self.generic_params = old_gens;
//             }
//             Decl::Module(name, inner) => {
//                 let new_prefix = if prefix.is_empty() {
//                     name.clone()
//                 } else {
//                     if name.contains("::") {
//                         name.clone()
//                     } else {
//                         format!("{}::{}", prefix, name)
//                     }
//                 };
//                 if new_prefix.contains("::") {
//                     let parts: Vec<&str> = new_prefix.split("::").collect();
//                     let last = parts.last().unwrap();
//                     self.aliases.insert(last.to_string(), new_prefix.clone());
//                 }
//                 self.collect_decls(inner, &new_prefix)?;
//             }
//             Decl::Use(u) => {
//                 if u.is_crate {
//                     self.phantom_types.insert(u.path[0].clone());
//                     if !u.path.is_empty() {
//                         let full_path = u.path.join("::");
//                         let last = u.path.last().unwrap();
//                         self.aliases.insert(last.clone(), full_path);
//                     }
//                 } else if !u.path.is_empty() {
//                     let full_path = u.path.join("::");
//                     let last = u.path.last().unwrap();
//                     self.aliases.insert(last.clone(), full_path);
//                 }
//                 if u.is_wildcard {
//                     self.has_wildcard_phantom = true;
//                 }
//             }
//             _ => {}
//         }
//     }
//     Ok(())
// }
