// [dcl.enum]/3, [temp.inst]/3: the forms of local-opaque-enum-in-
// template.cpp that clang 22 rejects ("enumerator does not exist in
// instantiation", "implicit instantiation of undefined member"; clang 19
// and GCC 16.1 accept them, and the text has one enumeration per
// instantiation): an opaque redeclaration after the definition, and an
// opaque declaration then definition in a generic lambda. WP-1613.
//@ std: c++20
//@ skip(clang): clang 22 rejects an enumerator of a block-scope opaque-then-defined enumeration in an instantiation
//@ oracle: gcc
//@ expect: run
template <class T> int g() {
  enum J : T;
  enum J : T { D = 1, E };
  enum J : T;
  enum class K : T { A = 3 };
  enum class K : T;
  return D + E + (int)K::A + (int)sizeof(J);
}
template <class T> int lam() {
  auto f = [](auto x) {
    enum class L : decltype(x);
    enum class L : decltype(x) { V = 7 };
    return (int)L::V + (int)sizeof(L);
  };
  return f(T{});
}
int main() {
  if (g<int>() != 10) return 1;
  if (g<char>() != 7) return 2;
  if (lam<long long>() != 15) return 3;
  return 0;
}
