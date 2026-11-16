// [dcl.fct.spec]/3: `explicit` appears only on a constructor declared in
// its class, not in a friend declaration; [class.dtor]/1: a destructor is
// not a template, a befriended one either. Both were accepted once
// friend constructors and destructors were. Found running GCC's C++
// testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:3 error "'explicit' is invalid in friend declarations"
//@ diag: 15:28 error "destructor cannot be declared as a template"

struct A { A(); ~A(); };

struct B {
  explicit friend A::A();
  template <int> friend A::~A();
};
