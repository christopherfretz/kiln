// Diagnostic ids `ext_*` (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

// WP-1435: before C23, pointers to differently qualified arrays (GCC's
// -Wpedantic wording; clang is silent, spec/diag.md D17).
ext_c_array_pointer_qualifiers = Extension [C23ArrayPointerQualifiers] (0) "invalid use of pointers to arrays with different qualifiers in ISO C before C23";
ext_cxx17_attr = Extension [Cxx17AttributeExtensions] (1) "use of the '{0}' attribute is a C++17 extension";
ext_cxx20_attr = Extension [Cxx20AttributeExtensions] (1) "use of the '{0}' attribute is a C++20 extension";
ext_cxx23_attr = Extension [Cxx23AttributeExtensions] (1) "use of the '{0}' attribute is a C++23 extension";
ext_cxx_type_aware_allocators = ExtWarn [ExtCxxTypeAwareAllocators] (0) "type aware allocators are a Clang extension";
ext_kiln_define_member = Warning [KilnExtension] (0) "'__kiln_define_member' is a kiln extension; use 'std::meta::define_aggregate'";
