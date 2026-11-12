// The templates in the global module fragment, with explicit
// instantiation declarations (double) and definitions (long) there
// (explicit-inst-gmf-run.cpp).
module;
#include "explicit-inst-templates.h"
extern template double S<double>::get(double);
extern template double V<double>::v;
extern template double vt<double>;
extern template struct C<double>;
extern template struct D<double>;
template long S<long>::get(long);
template long V<long>::v;
template long vt<long>;
template struct C<long>;
template struct D<long>;
export module xigmf;
export using ::S;
export using ::V;
export using ::vt;
export using ::C;
export using ::D;
