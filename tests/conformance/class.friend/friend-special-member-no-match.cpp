// [class.friend]/4: a friend constructor, destructor or conversion function
// must match a member of the named class. Found running GCC's C++
// testsuite (WP-1466).
//@ expect: compile-fail
//@ std: c++17
//@ xfail(clang): clang 22 adds a note per candidate constructor, which kiln does not give yet
//@ strict-diags
//@ diag: 12:13 error "friend declaration of 'A' does not match any declaration in 'A'"
//@ diag: 13:13 error "friend declaration of 'operator long' does not match any declaration in 'A'"
struct A { A(int); ~A(); operator int(); };
struct B {
  friend A::A(double);
  friend A::operator long();
};
