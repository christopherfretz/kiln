//@ expect: ast
//@ oracle: clang
// [temp.variadic]/5: pack expansions in braced initializers ([dcl.init.list])
template <class... Ts> int first(Ts... ts) {
  int a[] = {ts...};
  int b[] = {0, ts..., 0};
  return a[0] + b[0];
}
