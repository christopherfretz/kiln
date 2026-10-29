//@ expect: ast
//@ oracle: clang
// [temp.inst]/3, [temp.mem.enum]: a member enumeration's definition is
// instantiated with its enclosing class.
template <int N> struct Flags {
  enum Kind { A = N, B = N + 1 };
  Kind k;
};
int v = Flags<4>::B;
