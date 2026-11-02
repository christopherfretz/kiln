// [class.copy.assign]/12.2, [dcl.link]/6, [over.match.best]/2: the
// implicitly-defined copy assignment of a trivially copyable array member
// copies it with `__builtin_memcpy` (clang `buildMemcpyForAssignmentOp`),
// and that use finds the declaration an earlier written call created: one
// function, so a later written call is not ambiguous. A library
// `memcpy` declaration is a different function. libstdc++ format:4173 /
// fstream.tcc:387 (19 headers, docs/status/libstdcxx.md cluster X,
// WP-332). Companion: libstdcxx-builtin-memcpy-implicit-first.cpp.
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: abcd 4 4 xy
extern "C" int printf(const char *, ...);
extern "C" void *memcpy(void *, const void *, decltype(sizeof 0));
void f(char *d, const char *s) { __builtin_memcpy(d, s, 2); }
struct A { int a[4]; };
void g(A &x, const A &y) { x = y; }
void h(char *d, const char *s) { __builtin_memcpy(d, s, 2); memcpy(d + 2, s + 2, 2); }
template <class T> struct B { T b[2]; };
template <class T> void k(T *d, const T *s) { B<T> x{}, y{{s[0], s[1]}}; x = y; __builtin_memcpy(d, x.b, sizeof x.b); }
constexpr A cp(A a) { A b{}; b = a; return b; }
static_assert(cp(A{{1, 2, 3, 4}}).a[3] == 4);
int main() {
  char buf[5] = {}, src[5] = "abcd";
  f(buf, src);
  h(buf, src);
  A x{}, y{{1, 2, 3, 4}};
  g(x, y);
  char two[3] = {}, xy[2] = {'x', 'y'};
  k(two, xy);
  printf("%s %d %d %s\n", buf, x.a[3], cp(y).a[3], two);
}
