// [dcl.ambig.res]/2: `sizeof(X<A>::type::m)` with `type` a member typedef
// of `X`'s pattern naming a template parameter is a type-id when the
// instantiated `X<A>::type` makes `m` a type: the lookahead parses the
// qualifier rather than classify `m` in the pattern, as for
// `sizeof(std::remove_reference<decltype(*p)>::type::value_type)` in
// rocksdb's memtable/vectorrep.cc (spec/landed/sema/WP-724.md). WP-724.
//@ expect: run
//@ strict-diags
//@ stdout: ok 8 4 1 16
extern "C" int printf(const char *, ...);

template <class T> struct rr { typedef T type; };
template <class T> struct rr<T &> { typedef T type; };
template <class T> struct al { using type = T; };
struct V {
  typedef long value_type;
  struct N { int a, b, c, d; };
};
V *b;

int main() {
  int a = sizeof(rr<decltype(*b)>::type::value_type);
  int c = alignof(al<V>::type::N);
  int d = (int)(rr<V>::type::value_type)1;
  int e = sizeof(rr<V>::type::N);
  printf("ok %d %d %d %d\n", a, c, d, e);
  return 0;
}
