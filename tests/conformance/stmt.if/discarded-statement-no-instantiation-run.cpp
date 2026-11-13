// [stmt.if]/2: a discarded statement of a non-template function is still
// checked, but (note) an odr-use in it needs no definition: for
// [basic.def.odr] it is not potentially evaluated, so nothing named there
// is odr-used. A function template specialization or a member function of
// a class template specialization is implicitly instantiated only when
// referenced in a context that requires a definition ([temp.inst]/8), a
// variable template specialization or static data member likewise
// ([temp.inst]/7-8), and an implicit special member is defined only when
// odr-used ([class.default.ctor]/4, [class.copy.ctor]/12, [class.dtor]/11),
// a vtable is used only by a constructor's definition. So none of the
// bodies below, each of which static_asserts for every T, is instantiated
// (clang `MarkFunctionReferenced` / `DoMarkVarDeclReferenced` with
// `OdrUseContext::FormallyUsed`). A nested `if constexpr` keeps the
// discarded state, whichever branch it takes; a lambda body inside a
// discarded statement does not (see discarded-statement-still-instantiates).
// The shape is a downstream field-access macro
// (its exception header): an `if constexpr` in an
// ordinary member function whose discarded branch calls a helper template
// that static_asserts for its arguments. kiln instantiated them all.
//@ expect: run
//@ stdout: taken 3 7
extern "C" int printf(const char *, ...);

template <class T> inline constexpr bool never = false;

// Function templates: called, named, address taken, an operator.
template <class T> int f(T) {
  static_assert(never<T>, "f instantiated");
  return 0;
}
template <class T> struct Box {};
template <class T> int operator+(Box<T>, Box<T>) {
  static_assert(never<T>, "operator+ instantiated");
  return 0;
}

// Member functions of a class template specialization.
template <class T> struct A {
  A() {}
  ~A() { static_assert(never<T>, "~A instantiated"); }
  int g() {
    static_assert(never<T>, "A::g instantiated");
    return 0;
  }
  static int s() {
    static_assert(never<T>, "A::s instantiated");
    return 0;
  }
  template <class U> int t(U) {
    static_assert(never<U>, "A::t instantiated");
    return 0;
  }
  static int sdm;
};
template <class T> int A<T>::sdm = T::no_such_member;

// A constructor that is not instantiated uses no vtable, so the virtual
// function is not instantiated either.
template <class T> struct V {
  V() {}
  virtual int h() {
    static_assert(never<T>, "V::h instantiated");
    return 0;
  }
};

// Variable templates (not usable in constant expressions).
template <class T> int vt = T::no_such_member;

int run(bool b) {
  if constexpr (sizeof(int) == 0) {
    A<int> a;
    V<int> v;
    return f(1) + (Box<int>() + Box<int>()) + a.g() + A<int>::s() + a.t(1) +
           A<int>::sdm + v.h() + vt<int> + (&f<long>)(2);
  } else if constexpr (sizeof(int) == 4) {
    // Taken: but the discarded `else` below is still discarded.
    if (b) {
      return 3;
    }
  } else {
    // Nested in a discarded statement, taken or not.
    if constexpr (true) {
      return f('x');
    } else {
      return f(2.0);
    }
  }
  return 1;
}

struct S {
  int field(int x);
};
int S::field(int x) {
  if constexpr (true) {
    return x;
  } else {
    return vt<char> + f(x) + A<char>().g();
  }
}

int main() {
  printf("taken %d %d\n", run(true), S().field(7));
  return 0;
}
