// [temp.friend]/1: a friend class template or function template
// befriends every specialization; [namespace.memdef]/3: one not
// declared before is a hidden member of the innermost enclosing
// namespace. The befriended templates are redeclared in each class
// template specialization (the libc++ `__bounded_iter` pattern).
//@ expect: run
//@ stdout: 3 7 5 7 9 2
extern "C" int printf(const char*, ...);

template <class T> class Box;
template <class T> T peek(const Box<T>&);
template <class T> class Box {
  T v;
  template <class U> friend class Box;
  template <class U> friend U peek(const Box<U>&);
  template <class U> friend int sum(const Box<U>& a, const Box<U>& b);

public:
  Box(T x) : v(x) {}
  template <class U> T from(const Box<U>& o) const { return T(o.v); }
};
template <class T> T peek(const Box<T>& b) { return b.v; }
template <class T> int sum(const Box<T>& a, const Box<T>& b) { return a.v + b.v; }

namespace N {
class Key {
  int k = 7;
  template <class> friend struct Reader;
  template <class T> friend int twice(T, const Key&);
};
template <class T> struct Reader {
  int get(const Key& x) { return x.k; }
};
template <class T> int twice(T t, const Key& x) { return x.k + t; }
}  // namespace N

// A non-template class befriending a function template, defined later.
class Secret {
  int s = 2;
  template <class T> friend T reveal(const T&, const Secret&);
};
template <class T> T reveal(const T& t, const Secret& x) { return t * x.s; }

int main() {
  Box<int> a(3), b(4);
  Box<long> c(5);
  N::Key k;
  printf("%d %d %d %d %d %d\n", peek(a), sum(a, b), (int)a.from(c), N::Reader<int>().get(k),
         N::twice(2, k), reveal(1, Secret()));
}
