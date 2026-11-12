// A header unit with explicit instantiation declarations (double) and
// definitions (long) (explicit-inst-header-unit-run.cpp).
#pragma once
#include "explicit-inst-templates.h"
extern template double twice<double>(double);
extern template double S<double>::get(double);
extern template double V<double>::v;
extern template double vt<double>;
extern template struct C<double>;
extern template struct D<double>;
template long twice<long>(long);
template long S<long>::get(long);
template long V<long>::v;
template long vt<long>;
template struct C<long>;
template struct D<long>;
