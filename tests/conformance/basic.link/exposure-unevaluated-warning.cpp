// [basic.link]/14-16: not exposures by the text, with clang 22's warning
// at each use. An enumerator of a TU-local enumeration is not a TU-local
// entity (/16.1), also in an unevaluated operand or a discarded statement.
// A specialization of a function template that is not inline is not
// TU-local (/15.1 ignores its body, /16.5), also when named in an
// unevaluated operand or a discarded statement. A specialization of an
// inline template whose body names only such an entity (an enumerator, a
// local class's member function that is not inline) is not TU-local
// either; the warning is at its direct use only, as clang 22 (a call
// through another template gets none). GCC 16.1 accepts each without a
// warning. The exposures are in exposure-unevaluated-discarded.cpp.
//@ expect: compile-ok
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 32:53 warning "TU local entity 'e1' is exposed [-WTU-local-entity-exposure]"
//@ diag: 33:74 warning "TU local entity 'e1' is exposed [-WTU-local-entity-exposure]"
//@ diag: 34:53 warning "TU local entity 'nt<int>' is exposed [-WTU-local-entity-exposure]"
//@ diag: 35:74 warning "TU local entity 'nt<int>' is exposed [-WTU-local-entity-exposure]"
//@ diag: 36:46 warning "TU local entity 'it<int>' is exposed [-WTU-local-entity-exposure]"
//@ diag: 37:58 warning "TU local entity 'it<int>' is exposed [-WTU-local-entity-exposure]"
//@ diag: 38:47 warning "TU local entity 'lc<int>' is exposed [-WTU-local-entity-exposure]"
export module M;
static int helper() { return 1; }
namespace { enum E { e1 = 1 }; }
template <class T> int nt(T) { return helper(); }
template <class T> inline int it(T) { return e1; }
template <class T> inline int lc(T) { struct L { int g() { return helper(); } }; return L().g(); }
template <class T> inline int cnt(T t) { return nt(t); }
template <class T> inline int cit(T t) { return it(t); }

export inline int size_enumerator() { return sizeof(e1); }
export inline int discarded_enumerator() { if constexpr (false) { return e1; } return 0; }
export inline int size_not_inline() { return sizeof(nt(1)); }
export inline int discarded_not_inline() { if constexpr (false) { return nt(1); } return 0; }
export inline int enumerator_spec() { return it(1); }
export inline int size_enumerator_spec() { return sizeof(it(1)); }
export inline int local_class_spec() { return lc(1); }
export inline int through_not_inline() { return cnt(1); }
export inline int through_inline() { return cit(1); }
export int not_inline() { return it(1) + nt(1) + cit(1) + lc(1); }
