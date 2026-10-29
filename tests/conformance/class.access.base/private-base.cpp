// [class.access.base]/1,4,5: the access of an inherited member is the more
// restrictive of the member's and the path's; converting to an inaccessible
// base is ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(kiln): needs WP-065 parser classes
//@ diag: 33:22 error "'s' is a private member of 'B'"
//@ diag: 24:11 note "constrained by implicitly private inheritance here"
//@ diag: 23:30 note "member is declared here"
//@ diag: 36:11 error "cannot cast 'E' to its private base class 'B'"
//@ diag: 28:12 note "declared private here"
//@ diag: 37:11 error "cannot cast 'F' to its protected base class 'B'"
//@ diag: 29:12 note "declared protected here"
//@ diag: 38:12 error "'x' is a private member of 'B'"
//@ diag: 24:11 note "constrained by implicitly private inheritance here"
//@ diag: 23:16 note "member is declared here"
//@ diag: 38:18 error "'x' is a private member of 'B'"
//@ diag: 28:12 note "constrained by private inheritance here"
//@ diag: 23:16 note "member is declared here"
//@ diag: 38:24 error "'x' is a protected member of 'B'"
//@ diag: 29:12 note "constrained by protected inheritance here"
//@ diag: 23:16 note "member is declared here"
struct B { int x; static int s; };
class D : B {
public:
  int viaD() { return x; }
};
struct E : private B {};
struct F : protected B {
  int ok() { return x + s; }
};
struct G : D {
  int bad() { return s; }
};
int use(D d, E e, F f) {
  B *pb = &e;
  B &rb = f;
  return d.x + e.x + f.x + d.viaD() + (pb != 0) + rb.x;
}
