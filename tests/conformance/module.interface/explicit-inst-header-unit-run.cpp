// [temp.explicit]/10,12, [module.import]/5, spec/modules.md §7 "Explicit
// instantiations": a header unit has no object file, so its explicit
// instantiation definitions (long) are defined weak_odr in each importer,
// as are the importer's own after the header's extern template (double)
// or with none (int); an implicit use (short) is linkonce_odr.
//@ expect: run
//@ std: c++20
//@ args: -Wno-experimental-header-units
//@ header-unit: "Inputs/explicit-inst-hu.h"
import "Inputs/explicit-inst-hu.h";

template double twice<double>(double);
template double S<double>::get(double);
template double V<double>::v;
template double vt<double>;
template struct C<double>;
template struct D<double>;

template int twice<int>(int);
template int S<int>::get(int);
template int V<int>::v;
template int vt<int>;
template struct C<int>;
template struct D<int>;

template <class T> T all() {
  D<T> d;
  D<T> *p = &d;
  // 2 + 2 + 7 + 5 + 2 + 3 + 3 = 24.
  return twice(T(1)) + S<T>().get(T(1)) + V<T>::v + vt<T> + C<T>().f(T(1)) + C<T>::s + p->f(T(1));
}

int main() { return all<double>() == 24 && all<int>() == 24 && all<short>() == 24 && all<long>() == 24 ? 0 : 1; }
