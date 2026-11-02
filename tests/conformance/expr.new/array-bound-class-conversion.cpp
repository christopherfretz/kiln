// [expr.new]/8 (C++14): the first array bound is implicitly converted to
// std::size_t, a class-type bound by its non-explicit conversion
// functions (clang PerformImplicitConversion, AssignmentAction::Converting).
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 30:11 error "conversion from 'Two' to"
//@ diag: 19:14 note "candidate function"
//@ diag: 19:36 note "candidate function"
//@ diag: 31:11 error "converting 'Ex' to incompatible type"
//@ diag: 32:11 error "converting 'None' to incompatible type"
//@ diag: 33:11 error "converting 'Inc' to incompatible type"
//@ diag: 34:11 error "converting 'Ptr' to incompatible type"
//@ diag: 35:11 error "attempt to use a deleted function"
//@ diag: 24:14 note "'operator int' has been explicitly marked deleted here"
//@ diag: 36:11 error "conversion from 'Tpl2' to"
//@ diag: 25:34 note "candidate function [with T = "
//@ diag: 25:73 note "candidate function [with T = "
struct Two { operator int() const; operator long() const; };
struct Ex { explicit operator int() const; };
struct None { };
struct Inc;
struct Ptr { operator int*() const; };
struct Del { operator int() const = delete; };
struct Tpl2 { template <class T> operator T() const; template <class T> operator T&() const; };
struct Ok { operator int() const; };
struct TplOk { template <class T> operator T() const; };

void f(Two two, Ex ex, None n, Inc& inc, Ptr p, Del d, Tpl2 t2, Ok ok, TplOk tok) {
  new int[two];
  new int[ex];
  new int[n];
  new int[inc];
  new int[p];
  new int[d];
  new int[t2];
  new int[ok];
  new int[tok][2];
}
