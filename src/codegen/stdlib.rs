use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn generate_solar_std() -> TokenStream {
    let stream = quote! {
    #![allow(unused)]
    #[global_allocator]
    static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

    use std::io::{Read, Write};
    use std::borrow::Cow;

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum SolarCow<'a> {
        Borrowed(&'a str),
        Owned(String),
    }

    impl<'a> SolarCow<'a> {
        pub fn as_str(&self) -> &str {
            match self {
                SolarCow::Borrowed(s) => s,
                SolarCow::Owned(s) => s.as_str(),
            }
        }
    }

    impl<'a> std::ops::Deref for SolarCow<'a> {
        type Target = str;
        fn deref(&self) -> &str { self.as_str() }
    }

    impl<'a> AsRef<str> for SolarCow<'a> {
        fn as_ref(&self) -> &str { self.as_str() }
    }

    impl<'a> std::fmt::Display for SolarCow<'a> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.as_str())
        }
    }

    impl<'a> From<&'a str> for SolarCow<'a> {
        fn from(s: &'a str) -> Self { SolarCow::Borrowed(s) }
    }

    impl<'a> From<String> for SolarCow<'a> {
        fn from(s: String) -> Self { SolarCow::Owned(s) }
    }

    pub trait SolarStr {
        fn to_owned_string(&self) -> String;
        fn solar_to_string(&self) -> String;
        fn solar_to_str(&self) -> &str;
        fn solar_len(&self) -> i32;
        fn solar_contains(&self, s: impl AsRef<str>) -> bool;
        fn solar_split(&self, s: impl AsRef<str>) -> Vec<String>;
    }

    impl SolarStr for str {
        fn to_owned_string(&self) -> String { self.to_owned() }
        fn solar_to_string(&self) -> String { self.to_owned() }
        fn solar_to_str(&self) -> &str { self }
        fn solar_len(&self) -> i32 { self.len() as i32 }
        fn solar_contains(&self, s: impl AsRef<str>) -> bool { self.contains(s.as_ref()) }
        fn solar_split(&self, s: impl AsRef<str>) -> Vec<String> { self.split(s.as_ref()).map(|x| x.to_owned()).collect() }
    }

    pub trait SolarString {
        fn solar_append(&mut self, s: impl AsRef<str>);
        fn solar_len(&self) -> i32;
        fn solar_contains(&self, s: impl AsRef<str>) -> bool;
        fn solar_as_str(&self) -> &str;
        fn solar_to_str(&self) -> &str;
        fn solar_to_string(&self) -> String;
        fn solar_lines(&self) -> Vec<String>;
        fn solar_split(&self, s: impl AsRef<str>) -> Vec<String>;
    }

    impl SolarString for String {
        fn solar_append(&mut self, s: impl AsRef<str>) { self.push_str(s.as_ref()); }
        fn solar_len(&self) -> i32 { self.len() as i32 }
        fn solar_contains(&self, s: impl AsRef<str>) -> bool { self.contains(s.as_ref()) }
        fn solar_as_str(&self) -> &str { self.as_str() }
        fn solar_to_str(&self) -> &str { self.as_str() }
        fn solar_to_string(&self) -> String { self.clone() }
        fn solar_lines(&self) -> Vec<String> { self.lines().map(|x| x.to_owned()).collect() }
        fn solar_split(&self, s: impl AsRef<str>) -> Vec<String> { self.split(s.as_ref()).map(|x| x.to_owned()).collect() }
    }

    pub trait SolarVec<T> {
        fn solar_len(&self) -> i32;
        fn solar_get(&self, i: &i32) -> T;
    }

    impl<T: Clone> SolarVec<T> for Vec<T> {
        fn solar_len(&self) -> i32 { self.len() as i32 }
        fn solar_get(&self, i: &i32) -> T { self[*i as usize].clone() }
    }

    pub trait SolarIndex<Idx> {
        type Output;
        fn solar_index(&self, i: Idx) -> &Self::Output;
    }

    impl<T> SolarIndex<i32> for Vec<T> {
        type Output = T;
        fn solar_index(&self, i: i32) -> &T { &self[i as usize] }
    }

    impl<T> SolarIndex<i32> for *mut T {
        type Output = T;
        fn solar_index(&self, i: i32) -> &T { unsafe { &*self.add(i as usize) } }
    }

    impl<T> SolarIndex<i32> for *const T {
        type Output = T;
        fn solar_index(&self, i: i32) -> &T { unsafe { &*self.add(i as usize) } }
    }

    pub trait SolarAdd<Rhs = Self> {
        type Output;
        fn solar_add(&self, rhs: &Rhs) -> Self::Output;
    }

    // impl<T> SolarAdd for T
    // where
    //     T: std::ops::Add<Output = T> + Copy
    // {
    //     fn solar_add(&self, other: &Self) -> Self {
    //         *self + *other
    //     }
    // }

    impl SolarAdd for i32 {
        type Output = i32;
        fn solar_add(&self, rhs: &i32) -> i32 { *self + *rhs }
    }

    impl SolarAdd for i64 {
        type Output = i64;
        fn solar_add(&self, rhs: &i64) -> i64 { *self + *rhs }
    }

    impl SolarAdd for f32 {
        type Output = f32;
        fn solar_add(&self, rhs: &f32) -> f32 { *self + *rhs }
    }

    impl SolarAdd for f64 {
        type Output = f64;
        fn solar_add(&self, rhs: &f64) -> f64 { *self + *rhs }
    }

    impl SolarAdd<&str> for &str {
        type Output = String;
        fn solar_add(&self, rhs: &&str) -> String { format!("{}{}", self, rhs) }
    }

    impl SolarAdd<String> for &str {
        type Output = String;
        fn solar_add(&self, rhs: &String) -> String { format!("{}{}", self, rhs) }
    }

    impl<'a> SolarAdd<&'a String> for &str {
        type Output = String;
        fn solar_add(&self, rhs: &&'a String) -> String { format!("{}{}", self, *rhs) }
    }

    impl SolarAdd<&str> for String {
        type Output = String;
        fn solar_add(&self, rhs: &&str) -> String { format!("{}{}", self, rhs) }
    }

    impl SolarAdd<String> for String {
        type Output = String;
        fn solar_add(&self, rhs: &String) -> String { format!("{}{}", self, rhs) }
    }

    impl<'a> SolarAdd<&'a String> for String {
        type Output = String;
        fn solar_add(&self, rhs: &&'a String) -> String { format!("{}{}", self, *rhs) }
    }

    impl SolarAdd<i32> for &str {
        type Output = String;
        fn solar_add(&self, rhs: &i32) -> String { format!("{}{}", self, rhs) }
    }

    impl SolarAdd<i64> for &str {
        type Output = String;
        fn solar_add(&self, rhs: &i64) -> String { format!("{}{}", self, rhs) }
    }

    impl SolarAdd<f32> for &str {
        type Output = String;
        fn solar_add(&self, rhs: &f32) -> String { format!("{}{}", self, rhs) }
    }

    impl SolarAdd<f64> for &str {
        type Output = String;
        fn solar_add(&self, rhs: &f64) -> String { format!("{}{}", self, rhs) }
    }

    impl SolarAdd<i32> for String {
        type Output = String;
        fn solar_add(&self, rhs: &i32) -> String { format!("{}{}", self, rhs) }
    }

    impl SolarAdd<i64> for String {
        type Output = String;
        fn solar_add(&self, rhs: &i64) -> String { format!("{}{}", self, rhs) }
    }

    impl SolarAdd<f32> for String {
        type Output = String;
        fn solar_add(&self, rhs: &f32) -> String { format!("{}{}", self, rhs) }
    }

    impl SolarAdd<f64> for String {
        type Output = String;
        fn solar_add(&self, rhs: &f64) -> String { format!("{}{}", self, rhs) }
    }

    pub trait SolarSub<Rhs = Self> {
        type Output;
        fn solar_sub(&self, rhs: &Rhs) -> Self::Output;
    }

    impl SolarSub for i32 {
        type Output = i32;
        fn solar_sub(&self, rhs: &i32) -> i32 { *self - *rhs }
    }

    impl SolarSub for i64 {
        type Output = i64;
        fn solar_sub(&self, rhs: &i64) -> i64 { *self - *rhs }
    }

    impl SolarSub for f32 {
        type Output = f32;
        fn solar_sub(&self, rhs: &f32) -> f32 { *self - *rhs }
    }

    impl SolarSub for f64 {
        type Output = f64;
        fn solar_sub(&self, rhs: &f64) -> f64 { *self - *rhs }
    }

    pub trait SolarMul<Rhs = Self> {
        type Output;
        fn solar_mul(&self, rhs: &Rhs) -> Self::Output;
    }

    impl SolarMul for i32 {
        type Output = i32;
        fn solar_mul(&self, rhs: &i32) -> i32 { *self * *rhs }
    }

    impl SolarMul for i64 {
        type Output = i64;
        fn solar_mul(&self, rhs: &i64) -> i64 { *self * *rhs }
    }

    impl SolarMul for f32 {
        type Output = f32;
        fn solar_mul(&self, rhs: &f32) -> f32 { *self * *rhs }
    }

    impl SolarMul for f64 {
        type Output = f64;
        fn solar_mul(&self, rhs: &f64) -> f64 { *self * *rhs }
    }

    pub trait SolarDiv<Rhs = Self> {
        type Output;
        fn solar_div(&self, rhs: &Rhs) -> Self::Output;
    }

    impl SolarDiv for i32 {
        type Output = i32;
        fn solar_div(&self, rhs: &i32) -> i32 { *self / *rhs }
    }

    impl SolarDiv for i64 {
        type Output = i64;
        fn solar_div(&self, rhs: &i64) -> i64 { *self / *rhs }
    }

    impl SolarDiv for f32 {
        type Output = f32;
        fn solar_div(&self, rhs: &f32) -> f32 { *self / *rhs }
    }

    impl SolarDiv for f64 {
        type Output = f64;
        fn solar_div(&self, rhs: &f64) -> f64 { *self / *rhs }
    }

    pub trait SolarLT<Rhs = Self> {
        fn solar_lt(&self, rhs: &Rhs) -> bool;
    }

    impl SolarLT for i32 {
        fn solar_lt(&self, rhs: &i32) -> bool { *self < *rhs }
    }

    impl SolarLT for i64 {
        fn solar_lt(&self, rhs: &i64) -> bool { *self < *rhs }
    }

    impl SolarLT for f32 {
        fn solar_lt(&self, rhs: &f32) -> bool { *self < *rhs }
    }

    impl SolarLT for f64 {
        fn solar_lt(&self, rhs: &f64) -> bool { *self < *rhs }
    }

    pub trait SolarGT<Rhs = Self> {
        fn solar_gt(&self, rhs: &Rhs) -> bool;
    }

    impl SolarGT for i32 {
        fn solar_gt(&self, rhs: &i32) -> bool { *self > *rhs }
    }

    impl SolarGT for i64 {
        fn solar_gt(&self, rhs: &i64) -> bool { *self > *rhs }
    }

    impl SolarGT for f32 {
        fn solar_gt(&self, rhs: &f32) -> bool { *self > *rhs }
    }

    impl SolarGT for f64 {
        fn solar_gt(&self, rhs: &f64) -> bool { *self > *rhs }
    }

    pub trait SolarLE<Rhs = Self> {
        fn solar_le(&self, rhs: &Rhs) -> bool;
    }

    impl SolarLE for i32 {
        fn solar_le(&self, rhs: &i32) -> bool { *self <= *rhs }
    }

    impl SolarLE for i64 {
        fn solar_le(&self, rhs: &i64) -> bool { *self <= *rhs }
    }

    impl SolarLE for f32 {
        fn solar_le(&self, rhs: &f32) -> bool { *self <= *rhs }
    }

    impl SolarLE for f64 {
        fn solar_le(&self, rhs: &f64) -> bool { *self <= *rhs }
    }

    pub trait SolarGE<Rhs = Self> {
        fn solar_ge(&self, rhs: &Rhs) -> bool;
    }

    impl SolarGE for i32 {
        fn solar_ge(&self, rhs: &i32) -> bool { *self >= *rhs }
    }

    impl SolarGE for i64 {
        fn solar_ge(&self, rhs: &i64) -> bool { *self >= *rhs }
    }

    impl SolarGE for f32 {
        fn solar_ge(&self, rhs: &f32) -> bool { *self >= *rhs }
    }

    impl SolarGE for f64 {
        fn solar_ge(&self, rhs: &f64) -> bool { *self >= *rhs }
    }

    pub trait SolarEq<Rhs = Self> {
        fn solar_eq(&self, rhs: &Rhs) -> bool;
    }

    impl SolarEq for i32 {
        fn solar_eq(&self, rhs: &i32) -> bool { *self == *rhs }
    }

    impl SolarEq for i64 {
        fn solar_eq(&self, rhs: &i64) -> bool { *self == *rhs }
    }

    impl SolarEq for f32 {
        fn solar_eq(&self, rhs: &f32) -> bool { *self == *rhs }
    }

    impl SolarEq for f64 {
        fn solar_eq(&self, rhs: &f64) -> bool { *self == *rhs }
    }

    impl SolarEq for bool {
        fn solar_eq(&self, rhs: &bool) -> bool { *self == *rhs }
    }

    impl SolarEq for String {
        fn solar_eq(&self, rhs: &String) -> bool { self == rhs }
    }

    impl SolarEq<&str> for &str {
        fn solar_eq(&self, rhs: &&str) -> bool { self == rhs }
    }

    pub trait SolarI32 {
        fn solar_to_string(&self) -> String;
    }

    impl SolarI32 for i32 {
        fn solar_to_string(&self) -> String { self.to_string() }
    }

    pub trait SolarI64 {
        fn solar_to_string(&self) -> String;
    }

    impl SolarI64 for i64 {
        fn solar_to_string(&self) -> String { self.to_string() }
    }

    pub trait SolarF32 {
        fn solar_to_string(&self) -> String;
    }

    impl SolarF32 for f32 {
        fn solar_to_string(&self) -> String { self.to_string() }
    }

    pub trait SolarF64 {
        fn solar_to_string(&self) -> String;
    }

    impl SolarF64 for f64 {
        fn solar_to_string(&self) -> String { self.to_string() }
    }

    pub trait SolarAsArg<'a> {
        type Out;
        fn as_arg(&'a self) -> Self::Out;
    }

    impl<'a, T: 'a> SolarAsArg<'a> for T {
        type Out = &'a T;
        fn as_arg(&'a self) -> &'a T { self }
    }

    pub trait SolarAsVal<T> {
        fn as_val(&self) -> T;
    }

    impl<'a> crate::SolarAsVal<&'a str> for String {
        fn as_val(&self) -> &'a str { unsafe { std::mem::transmute(self.as_str()) } }
    }

    impl<'a, 'b> crate::SolarAsVal<&'a str> for &'b String {
        fn as_val(&self) -> &'a str { unsafe { std::mem::transmute(self.as_str()) } }
    }

    impl<'a> crate::SolarAsVal<&'a str> for &'a str {
        fn as_val(&self) -> &'a str { self }
    }

    impl<'a, 'b> crate::SolarAsVal<&'a str> for &'b &'a str {
        fn as_val(&self) -> &'a str { *self }
    }

    impl crate::SolarAsVal<String> for &str {
        fn as_val(&self) -> String { self.to_string() }
    }

    impl crate::SolarAsVal<String> for &&str {
        fn as_val(&self) -> String { self.to_string() }
    }

    impl crate::SolarAsVal<String> for String {
        fn as_val(&self) -> String { self.clone() }
    }

    impl<'a> crate::SolarAsVal<String> for &'a String {
        fn as_val(&self) -> String { (*self).clone() }
    }

    // Only implement for non-specialized types to avoid conflicts
    // This is a hack, but without negative trait bounds or better specialization it's hard.
    // We'll just implement for primitive-ish types that need it.
    impl crate::SolarAsVal<i32> for i32 { fn as_val(&self) -> i32 { *self } }
    impl crate::SolarAsVal<i32> for &i32 { fn as_val(&self) -> i32 { **self } }
    impl crate::SolarAsVal<f32> for f32 { fn as_val(&self) -> f32 { *self } }
    impl crate::SolarAsVal<f32> for &f32 { fn as_val(&self) -> f32 { **self } }
    impl crate::SolarAsVal<f64> for f64 { fn as_val(&self) -> f64 { *self } }
    impl crate::SolarAsVal<f64> for &f64 { fn as_val(&self) -> f64 { **self } }
    impl crate::SolarAsVal<bool> for bool { fn as_val(&self) -> bool { *self } }
    impl crate::SolarAsVal<bool> for &bool { fn as_val(&self) -> bool { **self } }

    impl crate::SolarAsVal<()> for () { fn as_val(&self) -> () { () } }
    impl crate::SolarAsVal<()> for &() { fn as_val(&self) -> () { **self } }

    #[cfg(feature = "macroquad")]
    impl crate::SolarAsVal<::macroquad::math::Vec2> for ::macroquad::math::Vec2 { fn as_val(&self) -> ::macroquad::math::Vec2 { *self } }
    #[cfg(feature = "macroquad")]
    impl crate::SolarAsVal<::macroquad::math::Vec2> for &::macroquad::math::Vec2 { fn as_val(&self) -> ::macroquad::math::Vec2 { **self } }
    #[cfg(feature = "macroquad")]
    impl crate::SolarAsVal<::macroquad::color::Color> for ::macroquad::color::Color { fn as_val(&self) -> ::macroquad::color::Color { *self } }
    #[cfg(feature = "macroquad")]
    impl crate::SolarAsVal<::macroquad::color::Color> for &::macroquad::color::Color { fn as_val(&self) -> ::macroquad::color::Color { **self } }


    // We can't have a blanket impl T: Clone because it conflicts with specialized impls.
    // For custom Solar objects, we can either generate the impl or use a macro.
    // For now, let's add a few more common ones.
    impl<T: Clone> SolarAsVal<Vec<T>> for Vec<T> { fn as_val(&self) -> Vec<T> { self.clone() } }
    impl<T: Clone> SolarAsVal<Vec<T>> for &Vec<T> { fn as_val(&self) -> Vec<T> { (*self).clone() } }


    impl<T: Clone> SolarAsVal<T> for ::std::rc::Rc<::std::cell::RefCell<T>> {
        fn as_val(&self) -> T { self.borrow().clone() }
    }

    impl<T: Clone> SolarAsVal<T> for ::std::sync::Arc<::parking_lot::RwLock<T>> {
        fn as_val(&self) -> T { self.read().clone() }
    }

    pub trait SolarAsSize {
        fn as_size(&self) -> usize;
    }

    impl SolarAsSize for i32 {
        fn as_size(&self) -> usize { *self as usize }
    }

    impl<'a> SolarAsSize for &'a i32 {
        fn as_size(&self) -> usize { **self as usize }
    }

    pub mod mem {
        pub fn alloc<T>(size: &i32) -> *mut T {
            let mut v = Vec::with_capacity(*size as usize);
            let p = v.as_mut_ptr();
            std::mem::forget(v);
            p
        }
        pub fn free<T>(p: &*mut T, size: &i32) {
            unsafe {
                let _ = Vec::from_raw_parts(*p, 0, *size as usize);
            }
        }
    }

    pub mod sr_fs {
        use std::io::{Read, Write};
        use std::fs::OpenOptions;

        pub fn create(path: impl AsRef<str>) -> std::fs::File {
            std::fs::File::create(path.as_ref()).expect("Failed to create file")
        }

        pub fn open(path: impl AsRef<str>) -> std::fs::File {
            std::fs::File::open(path.as_ref()).expect("Failed to open file")
        }

        pub fn append(path: impl AsRef<str>) -> std::fs::File {
            OpenOptions::new().append(true).create(true).open(path.as_ref()).expect("Failed to open file for append")
        }

        pub fn read(f: &mut std::fs::File) -> String {
            let mut s = String::new();
            f.read_to_string(&mut s).expect("Failed to read file");
            s
        }

        pub fn read_to_string(path: impl AsRef<str>) -> String {
            std::fs::read_to_string(path.as_ref()).expect("Failed to read file to string")
        }

        pub fn write(f: &mut std::fs::File, content: impl AsRef<str>) {
            f.write_all(content.as_ref().as_bytes()).expect("Failed to write file");
        }

        pub fn write_string(f: &mut std::fs::File, content: String) {
            f.write_all(content.as_bytes()).expect("Failed to write file");
        }
    }

    pub mod sr_io {
        use std::io::{self, Write};

        pub fn readline() -> String {
            let mut s = String::new();
            io::stdin().read_line(&mut s).expect("Failed to read line");
            s.trim().to_string()
        }

        pub fn write(content: impl AsRef<str>) {
            io::stdout().write_all(content.as_ref().as_bytes()).expect("Failed to write to stdout");
            io::stdout().flush().expect("Failed to flush stdout");
        }

        pub fn println(content: impl AsRef<str>) {
            println!("{}", content.as_ref());
        }

        pub fn exit(code: i32) {
            std::process::exit(code);
        }

        pub fn args() -> Vec < String > {
            std::env::args().collect()
        }
    }

    pub mod sr_string {
        pub fn new() -> String { String::new() }
    }

        pub mod sr_math {
        pub fn pi() -> f32 { std::f32::consts::PI }
        pub fn e() -> f32 { std::f32::consts::E }
        pub fn tau() -> f32 { std::f32::consts::TAU }
        pub fn pi64() -> f64 { std::f64::consts::PI }
        pub fn e64() -> f64 { std::f64::consts::E }
        pub fn tau64() -> f64 { std::f64::consts::TAU }
    }
        };
        stream
}
