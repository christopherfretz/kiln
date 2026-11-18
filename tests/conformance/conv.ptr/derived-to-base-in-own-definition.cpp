// CWG2310, [conv.ptr]/3: the derived-to-base pointer conversion needs a
// complete derived class, and a class is incomplete inside its own
// definition. clang 22 converts there anyway (a static member's
// initializer, a template instantiated from it, a trait) and so does
// kiln (docs/status/deviations.md); GCC 16.1 rejects the constant-evaluated
// conversions only. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -std=c++17
//@ xfail(gcc): GCC 16.1 rejects a constant-evaluated conversion from a class being defined
struct W { int i; };
constexpr bool take(const W *) { return true; }
struct Z1 : W { static Z1 z; static constexpr const W *p = &z; };          // non-template direct
struct Z2 : W { static Z2 z; static constexpr bool k = take(&z); };        // argument
template <class A, class B> struct chk { static A a; static constexpr B *p = &a; };
struct Z3 : W { chk<Z3, W> c; };                                            // via template (CWG2310 test)
template <class A, class B> struct conv { static A *a; static constexpr bool v = __is_convertible(A *, B *); };
struct Z4 : W { static_assert(conv<Z4, W>::v, ""); };                        // trait
struct Z5 : W { static_assert(__is_convertible(Z5 *, W *), ""); };
struct Z6 : W { static Z6 z; static constexpr W *q = static_cast<W *>(&z); };  // static_cast
struct Z7 : W { static Z7 z; static constexpr bool b = &z == (W *)0; };
