// [class.ctor]/7 and [dcl.constexpr]/4 in C++17: an implicitly-defined
// (or defaulted) default constructor is constexpr only if a user-written
// one with an empty body and no mem-initializers would satisfy the
// requirements of a constexpr constructor, which include that every
// non-variant non-static data member and base class subobject is
// initialized (a union: exactly one variant member; an anonymous union
// member: one of its members). A trivial default constructor that leaves
// a scalar uninitialized is therefore not constexpr before C++20, and
// default-initializing an object with it calls a non-constexpr function
// ([expr.const]/2.2): never a constant expression. clang 22 and GCC 16.1
// reject each. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -std=c++17
//@ strict-diags
//@ diag: 48:15 error "constexpr function never produces a constant expression [-Winvalid-constexpr]"
//@ diag: 48:38 note "non-constexpr constructor 'Scalar' cannot be used in a constant expression"
//@ diag: 47:8 note "declared here"
//@ diag: 49:15 error "constexpr variable 'v' must be initialized by a constant expression"
//@ diag: 48:38 note "non-constexpr constructor 'Scalar' cannot be used in a constant expression"
//@ diag: 49:19 note "in call to 'scalar(1)'"
//@ diag: 47:8 note "declared here"
//@ diag: 52:15 error "constexpr function never produces a constant expression [-Winvalid-constexpr]"
//@ diag: 52:36 note "non-constexpr constructor 'Array' cannot be used in a constant expression"
//@ diag: 51:8 note "declared here"
//@ diag: 55:15 error "constexpr function never produces a constant expression [-Winvalid-constexpr]"
//@ diag: 55:36 note "non-constexpr constructor 'Union' cannot be used in a constant expression"
//@ diag: 54:7 note "declared here"
//@ diag: 58:15 error "constexpr function never produces a constant expression [-Winvalid-constexpr]"
//@ diag: 58:34 note "non-constexpr constructor 'Anon' cannot be used in a constant expression"
//@ diag: 57:8 note "declared here"
//@ diag: 62:15 error "constexpr function never produces a constant expression [-Winvalid-constexpr]"
//@ diag: 62:37 note "non-constexpr constructor 'Derived' cannot be used in a constant expression"
//@ diag: 61:8 note "declared here"
//@ diag: 65:15 error "constexpr function never produces a constant expression [-Winvalid-constexpr]"
//@ diag: 65:40 note "non-constexpr constructor 'Defaulted' cannot be used in a constant expression"
//@ diag: 64:27 note "declared here"
//@ diag: 68:15 error "constexpr function never produces a constant expression [-Winvalid-constexpr]"
//@ diag: 68:41 note "non-constexpr constructor 'Templ' cannot be used in a constant expression"
//@ diag: 67:27 note "declared here"
//@ diag: 70:15 error "constexpr function never produces a constant expression [-Winvalid-constexpr]"
//@ diag: 70:37 note "non-constexpr constructor 'Scalar' cannot be used in a constant expression"
//@ diag: 47:8 note "declared here"
//@ diag: 73:13 error "constexpr constructor never produces a constant expression [-Winvalid-constexpr]"
//@ diag: 73:13 note "non-constexpr constructor 'Scalar' cannot be used in a constant expression"
//@ diag: 47:8 note "declared here"

struct Scalar { int a; };
constexpr int scalar(int i) { Scalar t; t.a = i; return t.a; }
constexpr int v = scalar(1);

struct Array { int a[2]; };
constexpr int array(int i) { Array t; t.a[0] = i; return t.a[0]; }

union Union { int a; char c; };
constexpr int unite(int i) { Union t; t.a = i; return t.a; }

struct Anon { union { int a; char c; }; };
constexpr int anon(int i) { Anon t; t.a = i; return t.a; }

struct Base { int b; };
struct Derived : Base {};
constexpr int base(int i) { Derived t; t.b = i; return t.b; }

struct Defaulted { int a; Defaulted() = default; };
constexpr int deflt(int i) { Defaulted t; t.a = i; return t.a; }

template <class T> struct Templ { T a; };
constexpr int templ(int i) { Templ<int> t; t.a = i; return t.a; }

constexpr int elems(int i) { Scalar t[2]; t[0].a = i; return t[0].a; }

struct Holder {
  constexpr Holder() : n(0) {}
  Scalar s;
  int n;
};
