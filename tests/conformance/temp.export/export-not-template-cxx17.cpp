// [diff.cpp03.temp], C++03 [temp]/1: before C++20 `export` must be followed by
// a template head. Any other declaration gets clang's "expected template" at the
// token after `export` (and is then declared without it), `export template`
// without `<` is "expected '<' after 'template'" and skipped to `;`; a
// non-template `export` after a template head or in a class is also an error.
// Negative twin of export-template-cxx17.cpp. WP-1744.
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 19:8 error "expected template"
//@ diag: 20:8 error "expected template"
//@ diag: 21:22 error "expected template"
//@ diag: 22:27 error "expected template"
//@ diag: 23:8 error "expected template"
//@ diag: 23:8 warning "exported templates are unsupported"
//@ diag: 24:17 error "expected '<' after 'template'"
//@ diag: 25:7 error "expected template"
//@ diag: 26:31 error "expected member name or ';' after declaration specifiers"
export int x;
export namespace E { int y; }
namespace M { export int w; }
template <class T> export void h();
export export template <class U> U k();
export template int f3<int>(int);
export;
template <class T> struct Q { export template <class U> void m(); };
template <class T> int f3(T) { return 0; }
int use() { h(); return x + E::y + M::w + k<int>(); }
