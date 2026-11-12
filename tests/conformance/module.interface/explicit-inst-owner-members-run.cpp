// [temp.explicit], spec/modules.md §7 "Explicit instantiations": static
// data members of a named-module class template, implicitly used by the
// importer with no explicit instantiation (short, linkonce_odr here) and
// with the module's explicit instantiation definition (long, the
// module's object defines them).
// clang 19 rejects the importer's implicit instantiation of a module
// static data member ("declaration of 'v' in the global module follows
// declaration in module"); clang 22 accepts it.
//@ expect: run
//@ std: c++20
//@ requires: clang>=22
//@ module: xiown=Inputs/explicit-inst-owner.cppm
import xiown;

template <class T> T members() { return V<T>::v + C<T>().f(T(1)) + C<T>::s; }

int main() {
  // 7 + 2 + 3 = 12.
  return members<short>() == 12 && members<long>() == 12 ? 0 : 1;
}
