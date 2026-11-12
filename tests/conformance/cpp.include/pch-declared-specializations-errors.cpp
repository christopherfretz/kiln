// [temp.expl.spec]/7, [temp.spec.partial], [basic.def.odr]: through a
// precompiled header, a late-defined explicit specialization is still
// defined, an implicitly instantiated variable template specialization
// is still instantiated, and a late-defined partial specialization is
// still the one chosen (WP-1352).
//@ expect: compile-fail
//@ diag: 11:24 error "redefinition of 'ch<char>'"
//@ diag: 12:31 error "explicit specialization of 'd3::sz' after instantiation"
//@ diag: 13:27 error "no member named 'w' in 'd2::pr<int, int>'"
#include "Inputs/declared_specializations.h"
template <> struct d1::ch<char> {};
template <> constexpr int d3::sz<int> = 4;
int x = d2::pr<int, int>::w;
