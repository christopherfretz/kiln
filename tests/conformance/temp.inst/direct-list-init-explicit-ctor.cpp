// [dcl.init.general]/16, [over.match.list]: an instantiated variable keeps
// its pattern's form of initialization. `T v{a, b}` is
// direct-list-initialization, which may choose an explicit constructor;
// `T v(a)` stays direct-initialization (libc++ `__code_point_view<_CharT>
// __view{__begin, __end}` in `__parse_fill_align`). Verified against
// clang 19.
//@ expect: run
template <class C> class V;
template <> class V<char> {
public:
  constexpr explicit V(const char* a, const char* b) : a(a), b(b) {}
  const char* a;
  const char* b;
};
struct W { explicit W(int x, int y) : v(x + y) {} explicit W(int x) : v(x) {} int v; };
template <class C> struct P {
  template <class I> constexpr long f(I& b, I e) { V<C> v{b, e}; return v.b - v.a; }
};
template <class T> int k(T a) { int v{a}; int w(a); W x{a, a}; W y(a); return v + w + x.v + y.v; }
static_assert([] { const char* s = "abc"; return P<char>{}.f(s, s + 2); }() == 2);
int main() { return k(1) == 5 ? 0 : 1; }
