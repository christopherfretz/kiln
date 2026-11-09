// [basic.start.main]/3: a function named `main` with C language linkage in
// a namespace is `main` for clang 22 (`FunctionDecl::isMain`), so its rules
// apply, and its linkage is warned about at the name (WP-1040).
//@ std: c++26
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:28 error "'main' must return 'int'"
//@ diag: 11:33 warning "'main' should not be 'extern"
namespace c {
extern "C" { namespace d { void main(); } }
}
