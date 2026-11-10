// [module.interface]/1, /6: exporting a class exports its members; their
// out-of-class definitions (static data members, member functions,
// member function templates, nested classes and enumerations, of a class
// and of a class template) are written without `export` and are usable
// from an importer. An exported qualified redeclaration of an exported
// namespace member is valid (WP-1159). A class template's static data
// member is used in export-member-template-var-run.cpp (clang 19 defect).
//@ expect: run
//@ std: c++26
//@ module: member_defs=Inputs/member-defs.cppm
import member_defs;

int main() {
  X<2> x;
  if (x.f() != 3 || x.g(3.5) != 6)
    return 1;
  if (X<2>::M{}.v != 20)
    return 2;
  S s;
  if (S::x != 7 || s.f() != 14 || s.g(4) != 5)
    return 3;
  if (S::N{}.a != 3 || int(S::E::a) != 4)
    return 4;
  return ns::h() == 5 ? 0 : 5;
}
