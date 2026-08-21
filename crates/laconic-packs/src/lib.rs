//! The language packs. All per-language knowledge lives here.

pub mod common;
pub mod go;
pub mod java;
pub mod python;
pub mod rust;
pub mod typescript;

use laconic_engine::pack::Pack;

/// Every pack laconic ships. The engine takes this as input rather than owning it, so the engine
/// holds no list of languages and adding one touches no engine file.
pub fn all() -> Vec<Box<dyn Pack>> {
    vec![
        Box::new(go::GoPack),
        Box::new(python::PythonPack),
        Box::new(rust::RustPack),
        Box::new(java::JavaPack),
        Box::new(typescript::TypeScriptPack),
    ]
}
