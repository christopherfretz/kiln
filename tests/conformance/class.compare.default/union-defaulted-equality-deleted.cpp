// [class.compare.default]/2: a union with a variant member has a deleted
// defaulted `operator==` (WP-1497).
//@ expect: compile-fail
//@ std: c++20
//@ diag: 8:29 error "object of type 'F' cannot be compared because its 'operator==' is implicitly deleted"
//@ diag: 7:23 note "explicitly defaulted function was implicitly deleted here"
union F { int i; bool operator==(const F &) const = default; };
bool g(F a, F b) { return a == b; }
