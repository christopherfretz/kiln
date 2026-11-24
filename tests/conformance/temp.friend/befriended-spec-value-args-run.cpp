// [temp.friend]/1, [temp.type]/1, [temp.over.link]/5: a friend naming a
// class template specialization with a non-type argument in a class
// template (`friend It<T, true>;`, `friend class It<T *, true>;`, a member
// template's `friend struct D::In<T, 2>;`) befriends the specialization
// the same template-id names anywhere else: each instantiation's
// It<X, true> reaches the private members, from a primary template's
// member and a partial specialization's (WP-3270). Reduced from real code.
//@ expect: run
//@ stdout: 1 2 3 4 5
extern "C" int printf(const char *, ...);

template <class T, bool B> struct It;
struct D {
  template <class U, int N> struct In;
};
template <class T> class C {
  friend It<T, true>;
  friend class It<T *, true>;
  friend struct D::In<T, 2>;
  int v = 1;
  static int s() { return 2; }
  int w = 4;

public:
  It<T, true> *p = nullptr;
};

template <class T, bool B> struct It {
  static int get(C<T> &c) { return c.v; }
  static int call() { return C<T>::s(); }
};
template <class T> struct It<T *, true> {
  static int get(C<T> &c) { return c.v + c.s(); }
};
template <class U, int N> struct D::In {
  static int get(C<U> &c) { return c.w + N - 2; }
  static int one(C<U> &c) { return c.v; }
};

int main() {
  C<int> a;
  C<long> b;
  int one = It<int, true>::get(a);
  int two = It<long, true>::call();
  int three = It<long *, true>::get(b);
  int four = D::In<int, 2>::get(a);
  int five = D::In<long, 2>::one(b) + 4;
  printf("%d %d %d %d %d\n", one, two, three, four, five);
}
