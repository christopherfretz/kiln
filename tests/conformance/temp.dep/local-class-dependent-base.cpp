// [temp.dep.general]/3, [temp.pre]/8, [temp.dep.type]/1: a base class
// declared in a dependent context (a local class of a function template, of
// a generic lambda or of a member function of a class template, a member
// class of a class template) is a dependent base unless it is the current
// instantiation, so unqualified lookup in the derived class does not search
// it. A non-dependent name it declares is undeclared at definition (a type
// name too); a call with dependent arguments is diagnosed at instantiation
// (clang's "explicit qualification required"). `this->z`, `A::z` and
// `B::z` find the member. WP-1084.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:5 error "unknown type name 'I'"
//@ diag: 26:22 error "use of undeclared identifier 'z'"
//@ diag: 27:17 error "use of undeclared identifier 'h'"
//@ diag: 35:35 error "use of undeclared identifier 'z'"
//@ diag: 38:37 error "use of undeclared identifier 'z'"
//@ diag: 39:48 error "use of undeclared identifier 'z'"
//@ diag: 45:35 error "use of undeclared identifier 'z'"
//@ diag: 50:38 error "explicit qualification required to use member 'z' from dependent base class"
//@ diag: 50:10 note "in instantiation of member function 'dependent_call(int)::B::g' requested here"
//@ diag: 53:40 note "in instantiation of function template specialization 'dependent_call<int>' requested here"
//@ diag: 49:18 note "member is declared here"
template <class T> int local_base() {
  struct A { int z = 1; void h() {} typedef int I; };
  struct B : A {
    int g() { return z; }
    void g2() { h(); }
    I g3();
    int ok() { return this->z + A::z + B::z; }
  };
  return B().ok();
}
template <class T> struct X {
  struct M { int z; };
  struct D : M { int g() { return z; } };
  int m() {
    struct A { int z = 1; };
    struct B : A { int g() { return z; } };
    struct C { struct E : A { int g() { return z; } }; };
    return 0;
  }
};
template <class T> auto generic_lambda = [](auto) {
  struct A { int z = 1; };
  struct B : A { int g() { return z; } };
  return 0;
};
template <class T> int dependent_call(T t) {
  struct A { int z(int v) { return v; } };
  struct B : A { int g(T u) { return z(u); } };
  return B().g(t);
}
int use() { return local_base<int>() + dependent_call(1); }
