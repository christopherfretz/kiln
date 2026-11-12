// [temp.explicit]/10,12, [module.reach], spec/modules.md §7 "Explicit
// instantiations": the unit holding a specialization's explicit
// instantiation definition owns its definition. Rows (module, importer):
// - extern template / definition (double): defined here weak_odr;
// - nothing / definition (int): weak_odr;
// - (the static data member and class of double and int) nothing /
//   implicit use: linkonce_odr;
// - extern template / declaration (float), extern template / implicit use
//   (unsigned), nothing / declaration (short): declared only (the program
//   needs a definition elsewhere, [temp.explicit]/12);
// - nothing / implicit use (char): linkonce_odr;
// - definition / implicit use (long): declared only (the module's object
//   defines it weak_odr).
// The static data member and whole-class rows of a definition here are in
// module.interface/explicit-inst-sdm-importer-run.cpp: clang 22 rejects
// them against a named module. clang 19 rejects even an importer's
// implicit instantiation of a module's static data member, so the case
// compares with clang 22 only.
//@ min-oracle: 22
//@ module: mod_explinst=Inputs/mod_explinst.cppm
import mod_explinst;

template double twice<double>(double);
template double S<double>::get(double);
template double vt<double>;
template struct D<double>;

template int twice<int>(int);
template int S<int>::get(int);
template int vt<int>;
template struct D<int>;

extern template float twice<float>(float);
extern template float S<float>::get(float);
extern template float V<float>::v;
extern template float vt<float>;
extern template struct C<float>;
extern template struct D<float>;

extern template short twice<short>(short);
extern template short S<short>::get(short);
extern template short V<short>::v;
extern template short vt<short>;
extern template struct C<short>;
extern template struct D<short>;

template <class T> T all() {
  D<T> d;
  D<T> *p = &d;
  return twice(T(1)) + S<T>().get(T(1)) + V<T>::v + vt<T> + C<T>().f(T(1)) + C<T>::s + p->f(T(1));
}

double use() {
  return all<double>() + all<int>() + all<float>() + all<unsigned>() + all<short>() + all<char>() +
         all<long>();
}
