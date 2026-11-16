// [class.friend]/4: a friend constructor, destructor or conversion function
// must match a member of the named class; each candidate is noted (WP-1495).
// Found running GCC's C++ testsuite (WP-1466).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 13:13 error "friend declaration of 'A' does not match any declaration in 'A'"
//@ diag: 11:12 note "candidate constructor has type mismatch at 1st parameter (expected 'double' but has 'int')"
//@ diag: 11:8 note "candidate constructor (the implicit copy constructor) has type mismatch at 1st parameter (expected 'double' but has 'const A &')"
//@ diag: 14:13 error "friend declaration of 'operator long' does not match any declaration in 'A'"
struct A { A(int); ~A(); operator int(); };
struct B {
  friend A::A(double);
  friend A::operator long();
};
