// [module.interface]/1, /6: an export-declaration declares namespace
// members, so `export` on an out-of-class definition or redeclaration of a
// class member is ill-formed (clang's `err_export_non_namespace_scope_name`,
// reported at the declarator-id, also through `export { }` and
// `export extern "C++"`). A namespace member's qualified redeclaration is
// checked against its previous declaration's linkage instead, and an
// explicit specialization named by a template-id is not diagnosed (WP-1159).
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 38:36 error "cannot export 'x' as it is not at namespace scope"
//@ diag: 39:37 error "cannot export 'f' as it is not at namespace scope"
//@ diag: 40:58 error "cannot export 'M' as it is not at namespace scope"
//@ diag: 41:15 error "cannot export 'x' as it is not at namespace scope"
//@ diag: 42:16 error "cannot export 'f' as it is not at namespace scope"
//@ diag: 43:35 error "cannot export 'g' as it is not at namespace scope"
//@ diag: 44:18 error "cannot export 'N' as it is not at namespace scope"
//@ diag: 45:22 error "cannot export 'E' as it is not at namespace scope"
//@ diag: 46:15 error "cannot export 'operator+' as it is not at namespace scope"
//@ diag: 48:11 error "cannot export 'h' as it is not at namespace scope"
//@ diag: 49:10 error "cannot export 'y' as it is not at namespace scope"
//@ diag: 51:29 error "cannot export 'k' as it is not at namespace scope"
//@ diag: 52:15 error "cannot export 'm' as it is not at namespace scope"
//@ diag: 52:25 error "cannot export 'n' as it is not at namespace scope"
//@ diag: 54:16 error "cannot export redeclaration 'nf' here since the previous declaration has module linkage"
//@ diag: 34:20 note "previous declaration is here"
//@ diag: 63:15 error "definition or redeclaration of 'gf' cannot name the global scope"
export module m;
export template <auto P> struct X { static int x; void f(); template <class T> struct M; };
struct S {
  static int x, y; void f(); template <class T> void g(); struct N; enum class E : int;
  int operator+(int); void h(); void k(); static int m, n; template <class T> void t();
};
namespace N { void nf(); }
export namespace E { void ef(); void eg(); }
export void gf();

export template <auto P> int X<P>::x = 0;
export template <auto P> void X<P>::f() {}
export template <auto P> template <class T> struct X<P>::M {};
export int S::x = 0;
export void S::f() {}
export template <class T> void S::g() {}
export struct S::N { int a; };
export enum class S::E : int { e };
export int S::operator+(int) { return 0; }
export {
  void S::h() {}
  int S::y = 1;
}
export extern "C++" void S::k() {}
export int S::m = 2, S::n = 3;
export template <> void S::t<int>() {}
export void N::nf() {}
export void E::ef() {}

// The class members stay declared: no cascade errors.
S::N sn{1};
int use() { return S::x + S::y + S::m + sizeof(X<1>::M<int>); }

// The global scope cannot be named; an exported namespace's own qualified
// member gets no extra-qualification warning.
export void ::gf() {}
namespace E {
export void E::eg() {}
}
