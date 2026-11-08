// [temp.constr.atomic]/1-3, [stmt.expand]: the parameter mapping (and so
// the satisfaction-cache key) of an atom whose lambda body holds an
// expansion statement includes the parameters named in the
// expansion-initializer and in the body. Checking `X<long, long>` first
// must not answer `X<long, char>` from the cache. GCC 16.1 agrees (clang
// 19 has no expansion statements). WP-821.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
template <class T, class U> concept X1 = [] {
  int n = 0;
  template for (auto x : {T{}, U{}}) n += sizeof(x);
  return n > 9;
}();
static_assert(X1<long, long>);
static_assert(!X1<long, char>);

template <class T, class U> concept X2 = [] {
  int n = 0;
  template for (constexpr auto x : {sizeof(int), sizeof(T)}) n += x + sizeof(U);
  return n > 20;
}();
static_assert(X2<long, long>);
static_assert(!X2<long, char>);

template <class T, class U> concept X3 = [] {
  int n = 0;
  template for (auto x : {T{}, T{}}) n += sizeof(x);
  return n > 2;
}();
static_assert(X3<long, char> && X3<long, int> && !X3<char, long>);
