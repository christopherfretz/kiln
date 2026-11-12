// [temp.explicit]/10,12, [module.global.frag], spec/modules.md §7
// "Explicit instantiations": with the templates in the module's global
// module fragment, the unit holding the explicit instantiation definition
// still owns it: extern template there / definition here (double) and
// nothing / definition here (int) are defined here weak_odr, the
// fragment's definition (long) only in the module's object, an implicit
// use (short) linkonce_odr. (A function template is left out: clang 22
// rejects an explicit instantiation that names it through the exported
// using-declaration.)
//@ expect: run
//@ std: c++20
//@ module: xigmf=Inputs/explicit-inst-gmf.cppm
import xigmf;

template double S<double>::get(double);
template double V<double>::v;
template double vt<double>;
template struct C<double>;
template struct D<double>;

template int S<int>::get(int);
template int V<int>::v;
template int vt<int>;
template struct C<int>;
template struct D<int>;

template <class T> T all() {
  D<T> d;
  D<T> *p = &d;
  // 2 + 7 + 5 + 2 + 3 + 3 = 22.
  return S<T>().get(T(1)) + V<T>::v + vt<T> + C<T>().f(T(1)) + C<T>::s + p->f(T(1));
}

int main() { return all<double>() == 22 && all<int>() == 22 && all<short>() == 22 && all<long>() == 22 ? 0 : 1; }
