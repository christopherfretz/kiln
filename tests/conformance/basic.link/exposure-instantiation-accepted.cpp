// [basic.link]/15.1, /16.5: an instantiation of a function template that is
// not inline has its body ignored, so it is not TU-local whatever its body
// names, and an exported inline function calling it is not an exposure.
// GCC 16.1 accepts; kiln follows the text and gives clang 22's warning at a
// use of a function template specialization whose template names a
// TU-local entity (also when explicitly instantiated, or through a pointer
// to it). No warning, as clang 22: a name found by argument-dependent lookup
// in the instantiation, a member of a class template, an inline template's
// instantiation that names nothing TU-local (here argument-dependent lookup
// finds `P::adl`), any use in a function that is not inline. The
// exposures are in exposure-instantiation.cpp.
//@ expect: compile-ok
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 40:35 warning "TU local entity 'not_inline<int>' is exposed [-WTU-local-entity-exposure]"
//@ diag: 41:41 warning "TU local entity 'not_inline<long>' is exposed [-WTU-local-entity-exposure]"
//@ diag: 42:44 warning "TU local entity 'explicit_inst<int>' is exposed [-WTU-local-entity-exposure]"
export module M;
static int hidden;
static int helper() { return 1; }
namespace N {
struct A {};
static int adl(A) { return 1; }
}
namespace P {
struct B {};
int adl(B) { return 2; }
}
template <class T> int not_inline(T) { return helper(); }
template <class T> int dependent(T x) { return adl(x); }
template <class T> int explicit_inst() { return hidden; }
template int explicit_inst<int>();
template <class T> struct C {
  int g() { return helper(); }
};
template <class T> inline int by_adl(T x) { return adl(x); }
template <class T> inline int clean(T x) { return x ? 1 : 0; }

export inline int call() { return not_inline(0); }
export inline int address() { auto p = &not_inline<long>; return p(0); }
export inline int call_explicit() { return explicit_inst<int>(); }
export inline int call_dependent() { return dependent(N::A{}); }
export inline int call_member() { return C<int>{}.g(); }
export inline int not_tu_local() { return by_adl(P::B{}); }
export inline int call_clean() { return clean(1); }
export int from_non_inline() { return by_adl(N::A{}) + not_inline(1); }
