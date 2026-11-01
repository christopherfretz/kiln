// [expr.prim.req.simple], [temp.constr.atomic]: the note for an expression
// requirement whose substitution failed shows the expression as written
// (clang's printPretty with its default policy): the template parameter by
// name, and the braced form of an explicit type conversion with one pair of
// braces.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:15 error "static assertion failed"
//@ diag: 21:15 note "because 'int' does not satisfy 'A'"
//@ diag: 20:42 note "because 'T::x + 1' would be invalid: type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 23:15 error "static assertion failed"
//@ diag: 23:15 note "because 'int' does not satisfy 'B'"
//@ diag: 22:42 note "because 'T(1, 2)' would be invalid: excess elements in scalar initializer"
//@ diag: 25:15 error "static assertion failed"
//@ diag: 25:15 note "because 'int' does not satisfy 'C'"
//@ diag: 24:42 note "because 'T{1, 2}' would be invalid: excess elements in scalar initializer"
//@ diag: 28:15 error "static assertion failed"
//@ diag: 28:15 note "because 'S' does not satisfy 'D'"
//@ diag: 27:57 note "because 'T{.a = 1, .a = 2}' would be invalid: initializer overrides prior initialization of this subobject"
template<class T> concept A = requires { T::x + 1; };
static_assert(A<int>);
template<class T> concept B = requires { T(1, 2); };
static_assert(B<int>);
template<class T> concept C = requires { T{1, 2}; };
static_assert(C<int>);
struct S { int a; };
template<class T> concept D = requires { T{.a = 1, .a = 2}; };
static_assert(D<S>);
