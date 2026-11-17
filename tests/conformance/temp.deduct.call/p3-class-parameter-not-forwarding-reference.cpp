// [temp.deduct.call]/3: in an implicit deduction guide, `T&&` naming a
// parameter of the class template is not a forwarding reference, so an
// lvalue does not deduce `T` from it; a constructor template's own `U&&`
// and a user-written guide's `T&&` still are (P0512R0).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -std=c++17
//@ strict-diags
//@ diag: 42:5 error "no viable constructor or deduction guide for deduction of template arguments of 'Box'"
//@ diag: 24:3 note "candidate function [with T = int] not viable: expects an rvalue for 1st argument"
//@ diag: 24:3 note "implicit deduction guide declared as 'template <class T> Box(T &&) -> Box<T>'"
//@ diag: 23:27 note "candidate template ignored: could not match 'Box<T>' against 'int'"
//@ diag: 23:27 note "implicit deduction guide declared as 'template <class T> Box(Box<T>) -> Box<T>'"
//@ diag: 46:6 error "no viable constructor or deduction guide for deduction of template arguments of 'Pair'"
//@ diag: 28:22 note "candidate function [with T = int, U = int] not viable: expects an rvalue for 1st argument"
//@ diag: 28:22 note "implicit deduction guide declared as 'template <class T, class U> Pair(T &&, U &&, long *) -> Pair<T>'"
//@ diag: 27:27 note "candidate function template not viable: requires 1 argument, but 3 were provided"
//@ diag: 27:27 note "implicit deduction guide declared as 'template <class T> Pair(Pair<T>) -> Pair<T>'"

template <class X, class Y> struct same;
template <class X> struct same<X, X> {};

template <class T> struct Box {
  Box(T &&);
};

template <class T> struct Pair {
  template <class U> Pair(T &&, U &&, long *);
};

template <class T> struct Guided {
  Guided(T &&, char *);
};
template <class T> Guided(T &&, char *) -> Guided<T>;

int n;
long *lp;
char *cp;

Box b1 = 3;
same<decltype(b1), Box<int>> s1;
Box b2 = n;

Pair p1{1, n, lp};
same<decltype(p1), Pair<int>> s2;
Pair p2{n, 1, lp};

Guided g1{n, cp};
same<decltype(g1), Guided<int &>> s3;
