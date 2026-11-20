// [dcl.link]/3: support for a language linkage other than "C" and "C++"
// is implementation-defined, and clang (like kiln) supports no other:
// "unknown linkage language" at the string, which may be a concatenation
// ([lex.string]/7); the declarations then belong to the enclosing scope
// with C++ linkage, so no further error follows (WP-1745).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:8 error "unknown linkage language"
//@ diag: 13:8 error "unknown linkage language"
//@ diag: 14:8 error "unknown linkage language"
//@ diag: 15:8 error "unknown linkage language"
extern "Java" void f();
extern "Fortran" { int x; }
extern "c" int y;
extern "C" "C++" int z;
int main() { f(); return x + y + z; }
