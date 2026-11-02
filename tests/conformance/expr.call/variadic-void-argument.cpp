// [expr.call]/12: after the default argument promotions, an argument for
// the ellipsis of type void is ill-formed; the callee is named as clang
// names it (function, method, constructor). In a template the check is
// made by each instantiation, also of a call the instantiation shares.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:8 error "cannot pass expression of type 'void' to variadic function"
//@ diag: 28:10 error "cannot pass expression of type 'void' to variadic method"
//@ diag: 29:11 error "cannot pass expression of type 'void' to variadic function"
//@ diag: 30:8 error "cannot pass expression of type 'void' to variadic function"
//@ diag: 31:8 error "cannot pass expression of type 'void' to variadic function"
//@ diag: 32:17 error "cannot pass expression of type 'void' to variadic method"
//@ diag: 35:13 error "cannot pass expression of type 'void' to variadic constructor"
//@ diag: 36:10 error "cannot pass initializer list to variadic method"
//@ diag: 39:40 error "cannot pass expression of type 'void' to variadic function"
//@ diag: 42:15 note "in instantiation of function template specialization 'shared<int>' requested here"
//@ diag: 40:44 error "cannot pass expression of type 'void' to variadic function"
//@ diag: 43:15 note "in instantiation of function template specialization 'rebuilt<int>' requested here"
void f();
void g(int, ...);
struct M { void m(int, ...); static void s(int, ...); };
struct C { C(int, ...); };
void (*p)(int, ...) = g;

void calls() {
  M x;
  g(1, f());
  x.m(1, f());
  M::s(1, f());
  p(1, f());
  g(1, (void)0);
  (x.*&M::m)(1, f());
  g(1, 2.0f, 'c', f);
  (void)noexcept(g(1, 2));
  C c(1, 2, f());
  x.m(1, {1});
}

template<class T> void shared() { g(1, f()); }
template<class T> void rebuilt(T t) { g(t, f()); }
template<class T> void never() { g(1, f()); }
template void shared<int>();
template void rebuilt<int>(int);
