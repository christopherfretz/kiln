// [dcl.enum]/3, [temp.inst]/3: a block-scope opaque-enum-declaration in a
// function template, then its enum-specifier, declare one enumeration in
// each instantiation: the declarations are instantiated in statement
// order, so the enumerators are declared once (kiln redefined them when
// the opaque declaration was instantiated). Scoped and unscoped, with a
// dependent fixed type, a variable of the type between the two, in a
// member of a class template; the forms clang 22 rejects (an unscoped
// enumerator named after its definition, a redeclaration after the
// definition, a generic lambda) are in local-opaque-enum-in-template-
// lambda.cpp. WP-1613.
//@ std: c++20
//@ expect: run
//@ strict-diags
template <class T> int g() {
  enum class K : T;
  enum class K : T { A = 3, B };
  return (int)K::A + (int)K::B + (int)sizeof(K);
}
template <class T> int h() {
  enum class J : T;
  J j{};
  enum class J : T { D = sizeof(j) };
  return (int)J::D;
}
template <class T> struct C {
  int m() {
    enum Q : T;
    enum Q : T { Z = 5 };
    return Q::Z;
  }
};
int main() {
  if (g<int>() != 11) return 1;
  if (g<char>() != 8) return 2;
  if (h<short>() != 2) return 3;
  if (C<char>().m() != 5) return 5;
  return 0;
}
