// [over.match.ref]/1.1, [over.match.copy]/1.2: in a direct-initialization
// an explicit conversion function is a candidate only when it yields
// (a reference to) the target type up to a qualification conversion: one
// yielding a derived class neither binds a reference to the base nor
// reaches the base's copy constructor. clang 19's wording and locations.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:10 error "no viable conversion from 'H' to 'const R'"
//@ diag: 20:19 note "candidate constructor not viable: no known conversion from 'H' to 'int' for 1st argument"
//@ diag: 20:38 note "candidate constructor not viable: no known conversion from 'H' to 'const R &' for 1st argument"
//@ diag: 26:10 error "no viable conversion from 'H2' to 'const R'"
//@ diag: 20:19 note "candidate constructor not viable: no known conversion from 'H2' to 'int' for 1st argument"
//@ diag: 20:38 note "candidate constructor not viable: no known conversion from 'H2' to 'const R &' for 1st argument"
//@ diag: 27:5 error "no viable conversion from 'H' to 'R'"
//@ diag: 20:19 note "candidate constructor not viable: no known conversion from 'H' to 'int' for 1st argument"
//@ diag: 20:38 note "candidate constructor not viable: no known conversion from 'H' to 'const R &' for 1st argument"
//@ diag: 28:3 error "no matching constructor for initialization of 'R'"
//@ diag: 20:19 note "candidate constructor not viable: no known conversion from 'H' to 'int' for 1st argument"
//@ diag: 20:38 note "candidate constructor not viable: no known conversion from 'H' to 'const R' for 1st argument"
struct R { int a; R(int x) : a(x) {} R(const R& o) : a(o.a + 1) {} };
struct D : R { D() : R(40) {} };
D gd;
struct H { explicit operator D() { return D(); } };
struct H2 { explicit operator D&() { return gd; } };
const R& r(H{});
const R& r2(H2{});
R&& r3(H{});
R r4(H{});
static_assert(!__is_constructible(R, H) && !__is_constructible(const R&, H2));
