//@ expect: ast
//@ oracle: clang
// [temp.alias]/2: an alias template argument used in a constant expression.
template <class T> using Bytes = char[sizeof(T)];
Bytes<int> b;
static_assert(sizeof(Bytes<long>) == sizeof(long));
