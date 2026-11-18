// [dcl.constexpr]/3 in C++17 (lifted by P1331R2, [diff.cpp17.dcl.dcl]):
// a constexpr function or constructor may not define a variable for which
// no initialization is performed. clang 22 accepts each as a C++20
// extension; GCC 16.1 rejects (and also counts a class type with a
// trivial default constructor, which the text does not: its default
// initialization calls the constructor, [dcl.init.general]/7). A lambda
// whose body defines one is not implicitly constexpr
// ([expr.prim.lambda.closure]/5). Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -std=c++17
//@ strict-diags
//@ diag: 27:33 warning "uninitialized variable in a constexpr function is a C++20 extension [-Wc++20-extensions]"
//@ diag: 28:34 warning "uninitialized variable in a constexpr function is a C++20 extension [-Wc++20-extensions]"
//@ diag: 31:9 warning "uninitialized variable in a constexpr function is a C++20 extension [-Wc++20-extensions]"
//@ diag: 37:32 warning "uninitialized variable in a constexpr function is a C++20 extension [-Wc++20-extensions]"
//@ diag: 40:16 warning "definition of a static variable in a constexpr function is a C++23 extension [-Wc++23-extensions]"
//@ diag: 40:16 warning "uninitialized variable in a constexpr function is a C++20 extension [-Wc++20-extensions]"
//@ diag: 44:33 warning "uninitialized variable in a constexpr function is a C++20 extension [-Wc++20-extensions]"
//@ diag: 46:14 warning "uninitialized variable in a constexpr function is a C++20 extension [-Wc++20-extensions]"
//@ diag: 52:37 warning "uninitialized variable in a constexpr constructor is a C++20 extension [-Wc++20-extensions]"
//@ diag: 58:7 warning "uninitialized variable in a constexpr function is a C++20 extension [-Wc++20-extensions]"
//@ diag: 64:7 warning "uninitialized variable in a constexpr function is a C++20 extension [-Wc++20-extensions]"
//@ diag: 73:15 error "constexpr variable 'implicit' must be initialized by a constant expression"
//@ diag: 73:26 note "non-constexpr function 'operator()' cannot be used in a constant expression"
//@ diag: 68:24 note "declared here"

constexpr int copy(int i) { int j; j = i; return j; }
constexpr int first(int i) { int a[2]; a[0] = i; return a[0]; }
constexpr int nested(int i) {
  if (i) {
    int k;
    k = i;
    return k;
  }
  return 0;
}
constexpr int two(int i) { int a, b = i; a = b; return a; }
constexpr int has_static() {
  if (false) {
    static int z;
  }
  return 0;
}
constexpr int loop() { for (int i; ;) { i = 0; return i; } }
constexpr int count(int n) {
  int c = 0, x;
  x = n;
  return c + x;
}
struct Box {
  int v;
  constexpr Box(int i) : v(0) { int q; q = i; v = q; }
};

// In a template only a variable of non-dependent type, at the definition.
template <class T> constexpr T dependent(T i) { T j; j = i; return j; }
template <class T> constexpr int plain(T i) {
  int j;
  j = i;
  return j;
}
int use = dependent(1) + plain(2);
auto explicit_lambda = [](int i) constexpr {
  int j;
  j = i;
  return j;
};
auto implicit_lambda = [](int i) {
  int j;
  j = i;
  return j;
};
constexpr int implicit = implicit_lambda(3);
