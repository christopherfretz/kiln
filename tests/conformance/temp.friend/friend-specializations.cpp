// [temp.friend]/1.3: a friend declarator-id that is a template-id names
// the specialization of a function template found by lookup that its
// type deduces ([temp.deduct.decl]); in a class template it is matched
// for each specialization (the libc++ `operator==<>` pattern), and only
// that specialization is a friend.
//@ expect: run
//@ stdout: 1 0 3 5 11 9
extern "C" int printf(const char*, ...);
template <class T> class Box;
template <class T> bool operator==(const Box<T>& a, const Box<T>& b);
template <class T> T peek(const Box<T>& b);
template <class T, class U> int mix(const Box<T>& a, U u);
namespace N {
  template <class T> int get(const T& x);
}
template <class T> class Box {
  T v;
  friend bool operator== <>(const Box&, const Box&);
  friend T peek<T>(const Box&);
  friend int mix<>(const Box&, long);
public:
  constexpr Box(T x) : v(x) {}
};
template <class T> bool operator==(const Box<T>& a, const Box<T>& b) { return a.v == b.v; }
template <class T> T peek(const Box<T>& b) { return b.v; }
template <class T, class U> int mix(const Box<T>& a, U u) { return int(a.v + u); }
class Plain {
  int p = 9;
  friend int N::get<>(const Plain&);
};
template <class T> int N::get(const T& x) { return x.p; }
int main() {
  Box<int> a(3), b(3), c(4);
  Box<short> s(1);
  printf("%d %d %d %d %d %d\n", a == b ? 1 : 0, a == c ? 1 : 0, peek(a), mix(c, 1L),
         mix(s, 10L), N::get(Plain()));
}
