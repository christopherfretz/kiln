// [over.match.ref]/1.1, [dcl.init.ref]/5.4: a copy-initialization of a
// reference does not consider explicit conversion functions, nor does a
// direct-initialization whose result is not reference-compatible (the
// temporary is copy-initialized) or the explicit constructors of a class
// temporary.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:7 error "no viable conversion from 'Ex' to 'int'"
//@ diag: 17:22 note "explicit conversion function is not a candidate"
//@ diag: 20:12 error "no viable conversion from 'Ex' to 'const int'"
//@ diag: 17:22 note "explicit conversion function is not a candidate"
//@ diag: 21:13 error "no viable conversion from 'Ex' to 'const long'"
//@ diag: 22:11 error "no viable conversion from 'Ex' to 'const E3'"
//@ diag: 18:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'Ex' to 'const E3 &' for 1st argument"
//@ diag: 18:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'Ex' to 'E3 &&' for 1st argument"
//@ diag: 18:22 note "explicit constructor is not a candidate"
struct Ex { explicit operator int() const { return 42; } };
struct E3 { explicit E3(Ex); };
int&& r = Ex{};
const int& c = Ex{};
const long& a(Ex{});
const E3& e3(Ex{});
