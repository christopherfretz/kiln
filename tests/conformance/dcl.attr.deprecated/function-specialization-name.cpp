// [dcl.attr.deprecated]/4, [temp.spec.general]: a deprecated function
// template specialization is named with its converted arguments, every
// name scoped, defaults and deduced arguments included, typed for a
// placeholder parameter (clang getNameForDiagnostic; WP-303).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 21:3 warning "'dflt<n::A, n::Ctx<n::A>>' is deprecated"
//@ diag: 16:43 note "'dflt<n::A, n::Ctx<n::A>>' has been explicitly marked deprecated here"
//@ diag: 25:3 warning "'nt<n::E::a, (n::E)3>' is deprecated"
//@ diag: 17:29 note "'nt<n::E::a, (n::E)3>' has been explicitly marked deprecated here"
//@ diag: 20:3 warning "'visit<n::A, n::Ctx<char *>>' is deprecated"
//@ diag: 24:3 note "in instantiation of function template specialization 'test<char>' requested here"
//@ diag: 15:31 note "'visit<n::A, n::Ctx<char *>>' has been explicitly marked deprecated here"
namespace n { struct A {}; template <class T> struct Ctx {}; enum class E { a }; }
template <class F, class C> [[deprecated]] void visit(F, C) {}
template <class T, class U = n::Ctx<T>> [[deprecated]] void dflt(T) {}
template <n::E e, auto v> [[deprecated]] void nt() {}
template <class CharT> void test() {
  using Context = n::Ctx<CharT *>;
  visit(n::A{}, Context{});
  dflt(n::A{});
}
void g() {
  test<char>();
  nt<n::E::a, (n::E)3>();
}
