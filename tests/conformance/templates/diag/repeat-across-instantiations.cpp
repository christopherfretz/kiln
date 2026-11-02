//@ expect: compile-fail
//@ strict-diags
//@ diag: 36:24 error "constexpr variable 'v' must be initialized by a constant expression"
//@ diag: 38:17 note "in instantiation of static data member 'U<int>::v' requested here"
//@ diag: 36:28 note "non-constexpr function 'runtime' cannot be used in a constant expression"
//@ diag: 34:5 note "declared here"
//@ diag: 36:24 error "constexpr variable 'v' must be initialized by a constant expression"
//@ diag: 39:18 note "in instantiation of static data member 'U<long>::v' requested here"
//@ diag: 36:28 note "non-constexpr function 'runtime' cannot be used in a constant expression"
//@ diag: 34:5 note "declared here"
//@ diag: 43:9 error "array is too large (18446744073709551614 elements)"
//@ diag: 45:9 note "in instantiation of template class 'N<char>' requested here"
//@ diag: 43:9 error "array is too large (18446744073709551615 elements)"
//@ diag: 46:10 note "in instantiation of template class 'N<short>' requested here"
//@ diag: 51:19 error "static assertion failed due to requirement 'sizeof(int) > 100': small"
//@ diag: 53:9 note "in instantiation of member class 'Outer<int>::Inner' requested here"
//@ diag: 55:12 note "in instantiation of template class 'Outer<int>' requested here"
//@ diag: 51:29 note "expression evaluates to '4 > 100'"
//@ diag: 51:19 error "static assertion failed due to requirement 'sizeof(char) > 100': small"
//@ diag: 53:9 note "in instantiation of member class 'Outer<char>::Inner' requested here"
//@ diag: 56:13 note "in instantiation of template class 'Outer<char>' requested here"
//@ diag: 51:29 note "expression evaluates to '1 > 100'"
//@ diag: 61:17 error "static assertion failed due to requirement 'sizeof(int) == 0': dependent"
//@ diag: 63:8 note "in instantiation of template class 'A<int>' requested here"
//@ diag: 61:27 note "expression evaluates to '4 == 0'"
//@ diag: 69:9 error "'m' declared as an array with a negative size"
//@ requires: clang<22
// clang 19 spelling; repeat-across-instantiations-clang22.cpp has clang 22's (WP-377).

// An error inside a template is reported once per instantiation that hits
// it, with that instantiation's context notes, even where the two
// messages are word for word the same: the deduplication key includes
// the instantiation stack (spec/diag.md 7.4).
int runtime();
template <class T> struct U {
  static constexpr int v = runtime() + sizeof(T);
};
int a = U<int>::v;
int b = U<long>::v;

// Messages that differ by the template argument.
template <class T> struct N {
  int m[sizeof(T) - 3];
};
N<char> c;
N<short> d;

// A member class: the stack has two frames.
template <class T> struct Outer {
  struct Inner {
    static_assert(sizeof(T) > 100, "small");
  };
  Inner i;
};
Outer<int> e;
Outer<char> f;

// The same specialization named twice is instantiated (and diagnosed)
// once.
template <class T> struct A {
  static_assert(sizeof(T) == 0, "dependent");
};
A<int> g;
A<int> h;

// A non-dependent error in the definition is reported there, once, not
// again by each instantiation.
template <class T> struct P {
  int m[-1];
  T t;
};
P<int> i;
P<long> j;
