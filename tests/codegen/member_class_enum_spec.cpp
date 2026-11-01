// [temp.expl.spec]/1, /5, /7, /16: explicit specializations of a member
// class, a member enumeration and a member class template of implicit class
// template specializations. Their members are ordinary (linkonce_odr when
// inline, strong when defined out of line) and mangle under the
// specialization (`_ZN1XIiE1N...`); the unspecialized members are still
// instantiated from the pattern; a scoped member enumeration's
// enumerators come from the specialization. (No default member initializer
// in the specialized member template: clang 19 hangs on one.)
template <class T>
struct X {
  struct N {
    T a = 1;
    int f() const { return 10; }
  };
  enum class F : int { a = 1, b };
  template <class U>
  struct M {
    U u{};
    int g() const { return sizeof(U); }
  };
  struct D;
};

template <>
struct X<int>::N {
  long z = 7;
  int f() const;
  int inl() const { return 5; }
  static int count;
};
int X<int>::N::f() const { return 42; }
int X<int>::N::count = 3;

template <>
enum class X<int>::F : int { p = 10, q = 20 };

template <>
template <class U>
struct X<int>::M {
  U *ptr;
  int g() const { return 100; }
};
template <>
template <>
struct X<char>::M<int> {
  int h() const { return 200; }
};

template <>
struct X<short>::D {
  int d() const { return 3; }
};

int use() {
  X<int>::N n;
  X<long>::N m;
  X<int>::M<double> mi;
  X<char>::M<int> mc;
  X<char>::M<long> ml;
  X<short>::D d;
  return n.f() + n.inl() + X<int>::N::count + m.f() + mi.g() + mc.h() + ml.g() + d.d() +
         int(X<int>::F::q) + int(X<long>::F::b);
}
