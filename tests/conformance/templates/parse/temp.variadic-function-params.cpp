//@ expect: ast
//@ oracle: clang
// [temp.variadic]/3: function parameter packs and `sizeof...` ([expr.sizeof]/5)
template <class... Ts> unsigned long count(Ts... ts) { return sizeof...(Ts) + sizeof...(ts); }
template <class T, class... Ts> void g(T t, Ts &...rest);
