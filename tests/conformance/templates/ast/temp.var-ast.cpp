//@ expect: ast
//@ oracle: clang
// [temp.var]/1, [temp.inst]/7: a variable template and its implicit
// instantiations.
template <class T> constexpr int size_v = sizeof(T);
int a = size_v<char>;
int b = size_v<long>;
