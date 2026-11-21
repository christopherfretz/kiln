// [cpp.pre]/1, [cpp.module], [cpp.import] (C++20): before C++20 `import
// m;` and `module x;` are declarations with an unknown type name, and
// `export` starts an exported template (C++98 [temp]/1), as clang 22
// reports them; nothing is "not supported".
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 12:1 error "unknown type name 'import'"
//@ diag: 13:8 error "expected template"
//@ diag: 13:8 error "unknown type name 'module'"
//@ diag: 14:1 error "unknown type name 'module'"
import m;
export module b;
module x;
