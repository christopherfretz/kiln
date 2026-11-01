// [except.handle]/10: referring to a non-static member through `this`
// in a handler of a constructor's or destructor's function-try-block is
// undefined behavior; clang warns (-Wexceptions) for each member access
// whose base is `this` under implicit casts only.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 17:30 warning "cannot refer to a non-static member from the handler of a constructor function try block"
//@ diag: 17:50 warning "cannot refer to a non-static member from the handler of a constructor function try block"
//@ diag: 17:57 warning "cannot refer to a non-static member from the handler of a constructor function try block"
//@ diag: 17:77 warning "cannot refer to a non-static member from the handler of a constructor function try block"
//@ diag: 18:37 warning "cannot refer to a non-static member from the handler of a destructor function try block"
//@ diag: 21:39 warning "cannot refer to a non-static member from the handler of a constructor function try block"
//@ diag: 25:35 warning "cannot refer to a non-static member from the handler of a constructor function try block"
struct B { int m; static int s; void f(); B(); B(int); ~B(); int g(); };
// Not reached: no-arg ctor, dtor, a normal function, a lambda body,
// parenthesized/dereferenced `this`, other objects.
B::B() try { } catch (...) { m = 1; s = 2; this->m = 3; f(); int k = sizeof(m); (void)k; }
B::~B() try { } catch (...) { (void)m; }
int B::g() try { return 0; } catch (...) { return m; }
struct C { int x; C(int a) try : x(a) {} catch (...) { [this]{ (void)x; }(); } };
B::B(int) try { } catch (...) { this->s = 1; (this)->m = 2; (*this).m = 3; B o; (void)o.m; }
// A template's handler is diagnosed once, at its definition.
template<class T>
struct X {
  int m; X() try {} catch (...) { m = 1; } };
X<int> xi;
int main() {}
