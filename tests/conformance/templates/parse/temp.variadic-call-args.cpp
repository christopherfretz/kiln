//@ expect: ast
//@ oracle: clang
// [temp.variadic]/5: pack expansions in function call arguments
void sink(...);
template <class... Ts> void fwd(Ts... ts) { sink(ts...); }
template <class... Ts> void twice(Ts... ts) { sink(ts..., ts...); }
template <class... Ts> void plus1(Ts... ts) { sink((ts + 1)...); }
