// [temp.inst], [except.handle] (WP-160): a try-block in a template is
// rebuilt on instantiation. Each exception declaration is declared again
// from its substituted type (the incomplete and abstract type checks, array
// decay), and the handler order is checked again: -Wexceptions is reported
// per instantiation, also when the definition already warned. A catch-all
// handler that is not last is an error in the definition only (the
// statement is dropped).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 76:10 error "catch-all handler must come last"
//@ diag: 82:32 warning "exception of type 'int' will be caught by earlier handler"
//@ diag: 82:17 note "for type 'int'"
//@ diag: 49:13 error "cannot catch incomplete type 'Inc'"
//@ diag: 67:3 note "in instantiation of function template specialization 'g<Inc>' requested here"
//@ diag: 31:8 note "forward declaration of 'Inc'"
//@ diag: 49:13 error "cannot catch pointer to incomplete type 'Inc'"
//@ diag: 68:3 note "in instantiation of function template specialization 'g<Inc *>' requested here"
//@ diag: 31:8 note "forward declaration of 'Inc'"
//@ diag: 55:12 warning "exception of type 'Der &' will be caught by earlier handler"
//@ diag: 70:3 note "in instantiation of function template specialization 'h<Base, Der>' requested here"
//@ diag: 54:12 note "for type 'Base &'"
//@ diag: 79:18 error "variable type 'A' is an abstract class"
//@ diag: 110:3 note "in instantiation of function template specialization 'd3<A>' requested here"
//@ diag: 74:25 note "unimplemented pure virtual method 'v' in 'A'"
//@ diag: 82:32 warning "exception of type 'int' will be caught by earlier handler"
//@ diag: 111:3 note "in instantiation of function template specialization 'dup<char>' requested here"
//@ diag: 82:17 note "for type 'int'"
//@ diag: 82:32 warning "exception of type 'int' will be caught by earlier handler"
//@ diag: 112:3 note "in instantiation of function template specialization 'dup<long>' requested here"
//@ diag: 82:17 note "for type 'int'"
struct Inc;
struct Base {};
struct Der : Base {};
template <class T> int f(T v) {
  try {
    if (v)
      throw v;
  } catch (T e) {
    return 1;
  } catch (const T &e) {
    return 2;
  } catch (...) {
    return 3;
  }
  return 0;
}
template <class T> void g() {
  try {
  } catch (T) {
  }
}
template <class B, class D> void h() {
  try {
  } catch (B &) {
  } catch (D &) {
  }
}
template <class T> void k() {
  try {
  } catch (T x[2]) {
    T *p = x;
    (void)p;
  }
}
int u() { return f(1) + f(2.0); }
void w1() {
  g<Inc>();
  g<Inc *>();
  g<void *>();
  h<Base, Der>();
  h<Der, Base>();
  k<int>();
}
struct A { virtual void v() = 0; };
template <class T> void d2() {
  try {} catch (...) {} catch (T) {}
}
template <class T> void d3() {
  try {} catch (T) {}
}
template <class T> void dup() {
  try {} catch (int) {} catch (int) {}
}
template <class T> int ftb(T v) try {
  throw v;
} catch (T &e) {
  return e ? 1 : 0;
} catch (...) {
  throw;
}
template <class T> struct S {
  T m;
  S(T v) try : m(v) {
  } catch (const T &) {
  }
};
template <class T> int lam() {
  auto l = [](auto x) {
    try {
      throw x;
    } catch (decltype(x) y) {
      return (int)y;
    }
    return 0;
  };
  return l(T(1));
}
void w2() {
  d2<int>();
  d3<A>();
  dup<char>();
  dup<long>();
  ftb(1);
  S<int> s(1);
  lam<long>();
}
