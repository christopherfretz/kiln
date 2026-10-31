// [temp.names]/5, [temp.arg.explicit]: `template` before a member that is
// not a template, at definition and at instantiation; a member
// template-id names no non-template member function.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:51 error "'h' following the 'template' keyword does not refer to a template"
//@ diag: 25:17 note "declared as a non-template here"
//@ diag: 34:13 error "no matching member function for call to 'f'"
//@ diag: 24:45 note "candidate function template not viable: requires single argument 'u', but 2 arguments were provided"
//@ diag: 22:36 note "candidate function template not viable: requires 0 arguments, but 2 were provided"
//@ diag: 28:42 error "no matching member function for call to 'f'"
//@ diag: 29:10 note "in instantiation of function template specialization 'c<int>' requested here"
//@ diag: 24:45 note "candidate function template not viable: requires single argument 'u', but 2 arguments were provided"
//@ diag: 22:36 note "candidate function template not viable: requires 0 arguments, but 2 were provided"
//@ diag: 30:51 error "'h' following the 'template' keyword does not refer to a template"
//@ diag: 31:10 note "in instantiation of function template specialization 'd<X>' requested here"
//@ diag: 25:17 note "declared as a non-template here"
//@ diag: 24:77 error "invalid operands to binary expression ('unsigned long' and 'std::nullptr_t')"
//@ diag: 32:42 note "in instantiation of function template specialization 'X::f<int, std::nullptr_t>' requested here"
//@ diag: 33:10 note "in instantiation of function template specialization 'e<int>' requested here"
struct X {
  template <class T> constexpr int f() const { return sizeof(T); }
  constexpr int f(int) const { return -1; }
  template <class T, class U> constexpr int f(U u) const { return sizeof(T) + u; }
  constexpr int h() const { return 0; }
};
template <class T> int b(X x) { return x.template h<T>(); }
template <class T> int c(X x) { return x.f<T>(1, 2); }
int cc = c<int>(X{});
template <class U> int d(U u) { return u.template h<int>(); }
int dd = d(X{});
template <class T> int e(X x) { return x.f<T>(nullptr); }
int ee = e<int>(X{});
int z = X{}.f<int>(1, 2);
