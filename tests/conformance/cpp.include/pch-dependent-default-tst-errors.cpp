// [temp.arg.general]/9, [class.derived.general]/2: the member type of a
// dependent specialization completed with its defaults is checked as a
// base class at the instantiation, as without a precompiled header
// (WP-1320).
//@ expect: compile-fail
//@ diag: 10:14 error "base specifier must name a class"
#include "Inputs/dependent_default_tst.h"

template <typename T>
struct bad : ddt::sized<T>::type {};
bad<int> b;
