// [temp.expl.spec]/1, /5: `template<> struct O<long>::I { ... };` explicitly specializes a
// member class of an implicitly instantiated specialization; its member functions (inline,
// templates, defaulted) are ordinary members, not function template specializations, and
// its out-of-line members are defined without `template<>`; a member class the
// specialization leaves alone is still instantiated from the pattern
//@ expect: run
//@ strict-diags
extern "C" int printf(const char *, ...);
template <class T>
struct O {
  struct I;
  struct J {
    T x;
  };
};
template <>
struct O<long>::I {
  int v;
  int get() const;
  template <class U>
  U as() const { return U(v) + 1; }
  static int s;
  bool operator==(const I &) const = default;
  I() : v(1) {}
};
int O<long>::I::get() const { return v; }
int O<long>::I::s = 5;
template <>
struct O<char>::J {
  long y;
  void set(long z) { y = z; }
};
int main() {
  O<long>::I i;
  O<char>::J j{};
  j.set(2);
  O<int>::J k{3};
  printf("%d %d %d %d %ld %d\n", i.get(), i.as<int>(), O<long>::I::s, i == i, j.y, k.x);
  return 0;
}
