//@ expect: ast
//@ oracle: clang
// [temp.arg.nontype]/1, [dcl.array]: a non-type parameter used as an array
// bound substitutes into a constant array type.
template <unsigned N> struct Buf {
  char bytes[N];
  int words[N / 2 + 1];
};
int sz = sizeof(Buf<8>);
