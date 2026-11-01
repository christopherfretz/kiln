// [dcl.init.list]/5-6, [class.temporary]/6: the backing array of a
// namespace-scope std::initializer_list is a _ZGR global (constant when
// constant-initialized); constructor templates taking
// std::initializer_list<T> deduce T from a braced list (WP-117).
namespace std {
template <class E> class initializer_list {
  const E *b; decltype(sizeof 0) n;
  constexpr initializer_list(const E *b, decltype(sizeof 0) n) : b(b), n(n) {}
public:
  constexpr initializer_list() : b(nullptr), n(0) {}
  constexpr const E *begin() const { return b; }
  constexpr const E *end() const { return b + n; }
  constexpr decltype(sizeof 0) size() const { return n; }
};
}
extern "C" int printf(const char *, ...);
std::initializer_list<int> gl = {1, 2, 3};
int f() { return 4; }
std::initializer_list<int> gd = {f(), 5};
struct V {
  int s = 0;
  template <class T> V(std::initializer_list<T> l) { for (T x : l) s += int(x); }
  template <class T> V(T, T) : s(-1) {}
};
V v1 = {1, 2};
V v2{1.5, 2.5};
V v3(1, 2);
int main() {
  int t = 0;
  for (int x : gl) t += x;
  for (int x : gd) t += x;
  printf("%d %zu %d %d %d\n", t, gl.size(), v1.s, v2.s, v3.s);
}
