// CWG 2631, [expr.const]/16, [class.mem.general]: an immediate invocation
// in a default member initializer that another default member
// initializer uses is evaluated at each use of the outer one, in that
// use's context, not where the outer one is written: `S3`'s `S2 s{}`
// evaluates `pick(__builtin_FUNCTION())` only where `S3` is initialized
// or by a constructor (here only in `long_name` and `Wide()`, whose names
// are long enough). clang 19 accepts it;
// clang 22 evaluates it at `S3`'s definition (an empty name) and rejects
// it, which the working draft does not allow, so kiln follows clang 19.
// Reduced from real code.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang<22
consteval int pick(const char *s) { return s[3]; }
struct S2 { int m = pick(__builtin_FUNCTION()); };
struct S3 { S2 s{}; };
struct Wide { S2 s{}; Wide() {} };
void long_name() { S3 a{}; Wide b; }
