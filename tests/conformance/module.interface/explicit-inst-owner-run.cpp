// [temp.explicit]/10,12, [module.reach], spec/modules.md §7 "Explicit
// instantiations": the unit holding a specialization's explicit
// instantiation definition owns its definition, whoever created the
// specialization. For a function template, a member function, a variable
// template and a class with a virtual function (module, importer):
// extern template / definition (double), nothing / definition (int),
// nothing / implicit use (short) and definition / implicit use (long,
// the module's object defines it) each link exactly once. The static
// data member rows are in explicit-inst-owner-members-run.cpp and
// explicit-inst-sdm-importer-run.cpp (clang rejects some of them).
//@ expect: run
//@ std: c++20
//@ module: xiown=Inputs/explicit-inst-owner.cppm
import xiown;

template double twice<double>(double);
template double S<double>::get(double);
template double vt<double>;
template struct D<double>;

template int twice<int>(int);
template int S<int>::get(int);
template int vt<int>;
template struct D<int>;

template <class T> T all() {
  D<T> d;
  D<T> *p = &d;
  return twice(T(1)) + S<T>().get(T(1)) + vt<T> + p->f(T(1));
}

int main() {
  // 2 + 2 + 5 + 3 = 12.
  return all<double>() == 12 && all<int>() == 12 && all<short>() == 12 && all<long>() == 12 ? 0 : 1;
}
