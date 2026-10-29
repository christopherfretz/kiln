//@ expect: ast
//@ oracle: clang
// [temp.dep.type]/9, [dcl.enum]/5: a member enumeration of a class template
// is dependent; its enumerators have values only in a specialization.
template <int N> struct F {
  enum K { A = 1, B, C = N, D };
};
int v = F<3>::D + F<3>::A;
