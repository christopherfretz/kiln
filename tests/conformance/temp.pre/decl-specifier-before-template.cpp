// [temp.pre]/1: no decl-specifier precedes `template` in a
// template-declaration, so `inline template <class T> void f();` is
// ill-formed. clang 22 reads `inline`/`static` before `template` as the
// GNU `static template` / `inline template` explicit-instantiation
// extension and ignores the keyword with a plain warning
// (-Wstatic-inline-explicit-instantiation, not promoted by
// -pedantic-errors, an error only under -Werror); kiln is clang-compatible
// for this extension and matches it. GCC 16.1 rejects it (a documented
// difference). Another decl-specifier there is an error in all three.
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -pedantic-errors
//@ strict-diags
//@ diag: 19:1 warning "ignoring 'inline' keyword on explicit template instantiation"
//@ diag: 20:1 warning "ignoring 'static' keyword on explicit template instantiation"
//@ diag: 21:1 warning "ignoring 'inline' keyword on explicit template instantiation"
//@ diag: 22:15 warning "ignoring 'inline' keyword on explicit template instantiation"
template <class T> void g(T) {}
inline template <class T> void f1();
static template <class T> struct X {};
inline template <> void g<int>(int) {}
namespace N { inline template <class T> int v = 0; }
extern template void g<char>(char);
template <class T> void ok();
