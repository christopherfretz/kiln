// [temp.explicit]/10, spec/modules.md §7 "Explicit instantiations": an
// importer's explicit instantiation definition of a static data member or
// a whole class template specialization of a named module defines it
// (weak_odr), after the module's explicit instantiation declaration
// (double) or with none (int). clang 22 and 19 reject both ("declaration
// of 'v' in the global module follows declaration in module xiown",
// [basic.link]/11) while accepting the same explicit instantiation of a
// member function or variable template; kiln treats every kind alike.
//@ expect: run
//@ std: c++20
//@ module: xiown=Inputs/explicit-inst-owner.cppm
//@ skip(clang): clang 22 and 19 reject an importer's explicit instantiation of a module's static data member
import xiown;

template double V<double>::v;
template struct C<double>;
template int V<int>::v;
template struct C<int>;

int main() {
  // 7 + 2 + 3 = 12.
  return V<double>::v + C<double>().f(1) + C<double>::s == 12 && V<int>::v + C<int>().f(1) + C<int>::s == 12
             ? 0
             : 1;
}
