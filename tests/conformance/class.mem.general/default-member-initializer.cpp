// [class.mem.general]/7: a default member initializer is a complete-class
// context: it names members declared later. Needing it before the
// outermost enclosing class is complete (outside a complete-class
// context) is an error ([class.mem.general]/8).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:35 error "default member initializer for 'n' needed within definition of enclosing class 'Outer' outside of member functions"
//@ diag: 19:9 note "default member initializer declared here"
struct S {
  int a = b + 1;
  int c = sz();
  static constexpr int b = 1;
  static constexpr int sz() { return sizeof(S); }
};
static_assert(S{}.a == 2 && S{}.c == sizeof(S));

struct Outer {
  struct In {
    int n = Outer::k;
  };
  static constexpr int k = 3;
  static constexpr int early = In{}.n;
};

template <class T> struct TS {
  T x = y;
  static constexpr T y = 4;
};
static_assert(TS<int>{}.x == 4);
