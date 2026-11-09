// [class.union.general]/2: a union can have non-virtual member functions,
// including constructors and a destructor (WP-1015).
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
union U {
  int a;
  float f;
  constexpr U() : a(1) {}
  ~U() {}
  int get() const { return a; }
  static int s() { return 2; }
  template<class T> T as() const { return T(a); }
};
int x = U().get() + U::s() + U().as<int>();
