// [temp.constr.normal]/1.4, [temp.constr.atomic]/1-3: a concept-id is
// normalized into the concept's constraint-expression with a parameter
// mapping per atom; an argument that fails to substitute (`Tc&` for `void`)
// makes only the atoms that use its parameter ill-formed. A42b<void, void&>
// is satisfied by its first operand, `Void<void>`; NV<void, void&> is not
// (its first operand is false, its second ill-formed), and the failure is
// reported at the argument. Older clangs fail the whole concept-id.
// Found running GCC's C++ testsuite. WP-1509.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 29:9 error "no matching function for call to 'f'"
//@ diag: 28:24 note "candidate template ignored: constraints not satisfied [with T = void]"
//@ diag: 28:37 note "because 'void' does not satisfy 'R2'"
//@ diag: 25 note "because substituted constraint expression is ill-formed: cannot form a reference to 'void'"
//@ diag: 24:44 note "and '!Void<void>' evaluated to false"
template <class T> struct A { static const int x = 42; };
template <class Ta> concept A42 = A<Ta>::x == 42;
template <class Tv> concept Void = __is_same_as(Tv, void);
template <class Tb, class Ub> concept A42b = Void<Tb> || A42<Ub>;
template <class Tc> concept R42c = A42b<Tc, Tc&>;
static_assert(R42c<void>);
static_assert(R42c<int>);
template <class Tb, class Ub> concept NV = !Void<Tb> || A42<Ub>;
template <class Tc> concept R2 = NV<Tc, Tc&>;
static_assert(!R2<void>);
static_assert(R2<int>);
template <class T> int f() requires R2<T> { return 0; }
int g = f<void>();
