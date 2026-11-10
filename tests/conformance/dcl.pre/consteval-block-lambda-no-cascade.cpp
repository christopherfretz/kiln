// [dcl.pre]/P3289R1, [temp.inst]/3, [expr.prim.lambda]: a consteval
// block whose evaluation fails (here: an uncaught exception) makes the
// program ill-formed, and is diagnosed once; it does not invalidate the
// enclosing body. A lambda whose body holds it, substituted at
// instantiation, stays a valid closure, so later uses of the lambda (a
// call, a copy, a variable template it initializes, a closure returned
// from a function template) and of the enclosing function report
// nothing more (WP-1257, the consteval-block form of WP-1247; spec/sema-templates.md §5.5).
// A function template, a class member template and a generic lambda are pinned too (GCC
// 16.1 agrees). Requires 22: clang 19 names the closure `(anonymous class)`.
//@ requires: clang>=22
//@ expect: compile-fail
//@ skip(clang): clang 19 has no consteval blocks (P3289R1)
//@ oracle: gcc
//@ strict-diags
//@ diag: 39:16 error "consteval block is not a constant expression"
//@ diag: 39:14 note "while substituting into a lambda expression here"
//@ diag: 42:15 note "in instantiation of function template specialization 'used<int>' requested here"
//@ diag: 39:54 note "unhandled exception of type 'int' with content 1 thrown from here"
//@ diag: 45:36 error "consteval block is not a constant expression"
//@ diag: 45:34 note "while substituting into a lambda expression here"
//@ diag: 46:15 note "in instantiation of variable template specialization 'lam_v' requested here"
//@ diag: 45:74 note "unhandled exception of type 'int' with content 2 thrown from here"
//@ diag: 51:15 error "consteval block is not a constant expression"
//@ diag: 51:13 note "while substituting into a lambda expression here"
//@ diag: 53:23 note "in instantiation of function template specialization 'make<int>' requested here"
//@ diag: 51:53 note "unhandled exception of type 'int' with content 3 thrown from here"
//@ diag: 59:3 error "consteval block is not a constant expression"
//@ diag: 62:15 note "in instantiation of function template specialization 'ft<int>' requested here"
//@ diag: 59:41 note "unhandled exception of type 'int' with content 4 thrown from here"
//@ diag: 66:29 error "consteval block is not a constant expression"
//@ diag: 68:24 note "in instantiation of member function 'C<int>::m' requested here"
//@ diag: 66:67 note "unhandled exception of type 'int' with content 5 thrown from here"
//@ diag: 70:34 error "consteval block is not a constant expression"
//@ diag: 71:17 note "in instantiation of function template specialization '(lambda)::operator()<int>' requested here"
//@ diag: 70:72 note "unhandled exception of type 'int' with content 6 thrown from here"
template <class T>
constexpr int used() {
  int x = [] { consteval { if (sizeof(T) == 4) throw 1; } return 7; }();
  return x;
}
static_assert(used<int>() == 7);

template <class T>
inline constexpr auto lam_v = [] { consteval { if (sizeof(T) == 4) throw 2; } return 8; };
static_assert(lam_v<int>() == 8);
constexpr auto copy = lam_v<int>;
static_assert(copy() == 8);

template <class T> constexpr auto make() {
  return [] { consteval { if (sizeof(T) == 4) throw 3; } return 9; };
}
constexpr auto made = make<int>();
static_assert(made() == 9);

// The same shape without a lambda: a function template, a member
// function of a class template, a generic lambda's call operator.
template <class T> constexpr int ft() {
  consteval { if (sizeof(T) == 4) throw 4; }
  return 10;
}
static_assert(ft<int>() == 10);
static_assert(ft<int>() + 1 == 11);

template <class T> struct C {
  constexpr int m() const { consteval { if (sizeof(T) == 4) throw 5; } return 12; }
};
static_assert(C<int>().m() == 12);

constexpr auto gl = [](auto x) { consteval { if (sizeof(x) == 4) throw 6; } return x; };
static_assert(gl(13) == 13);
static_assert(gl(13) + gl(1) == 14);
