//@ expect: run
//@ stdout: 3 4 7 9 2
// [temp.friend]/1: class templates that befriend themselves and each other
// (every specialization of each is a friend of every specialization of
// the other). Regression test (WP-121): kiln once declared these
// befriendings as the enclosing template itself.
template <class T> class A;
template <class T> class B {
  template <class U> friend class A;
  template <class U> friend class B;
  T b;
public:
  constexpr explicit B(T v) : b(v) {}
  template <class U> constexpr T plus(const B<U>& o) const { return b + T(o.b); }
};
template <class T> class A {
  template <class U> friend class B;
  template <class U> friend class A;
  T a;
public:
  constexpr explicit A(T v) : a(v) {}
  template <class U> constexpr T peek(const B<U>& o) const { return a + T(o.b); }
  template <class U> constexpr T same(const A<U>& o) const { return a + T(o.a); }
};
template <class T> class C {
  template <class U> friend class C;
  template <class V> friend class C;
  T c;
public:
  constexpr explicit C(T v) : c(v) {}
  template <class U> constexpr T diff(const C<U>& o) const { return c - T(o.c); }
};
extern "C" int printf(const char*, ...);
int main() {
  constexpr B<int> bi(1);
  constexpr B<long> bl(2);
  constexpr A<long> al(5);
  constexpr A<int> ai(4);
  constexpr C<char> cc(5);
  constexpr C<int> ci(3);
  static_assert(al.peek(bi) == 6 && ai.same(al) == 9 && cc.diff(ci) == 2);
  printf("%d %d %d %d %d\n", bi.plus(bl), ai.peek(B<short>(0)), (int)al.peek(bl), ai.same(al), (int)cc.diff(ci));
}
