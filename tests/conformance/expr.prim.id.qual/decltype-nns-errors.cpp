// [expr.prim.id.qual]/1, [basic.lookup.qual.general]/1: the type a
// `decltype(e)` qualifier denotes must be a class or enumeration; a name
// after it must be found in that scope; a dependent qualifier that is
// substituted with a non-class type cannot be used before `::`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:9 error "'decltype(i)' (aka 'int') is not a class, namespace, or enumeration"
//@ diag: 24:1 error "'decltype(1)' (aka 'int') is not a class, namespace, or enumeration"
//@ diag: 25:9 error "'decltype(*pi)' (aka 'Inc &') is not a class, namespace, or enumeration"
//@ diag: 26:22 error "no member named 'nope' in 'S'"
//@ diag: 27:23 error "no type named 'nope' in 'S'"
//@ diag: 28:9 error "'decltype(s.m)' (aka 'int') is not a class, namespace, or enumeration"
//@ diag: 29:5 error "'decltype(i)' (aka 'int') is not a class, namespace, or enumeration"
//@ diag: 30:12 error "'decltype(i)' (aka 'int') is not a class, namespace, or enumeration"
//@ diag: 30:25 error "expected class name"
//@ diag: 31:52 error "type 'decltype(int{})' (aka 'int') cannot be used prior to '::' because it has no members"
//@ diag: 32:9 note "in instantiation of template class 'X<int>' requested here"
int i;
struct Inc;
Inc *pi;
struct S { int m; };
S s;
int a = decltype(i)::x;
decltype(1)::type t;
int b = decltype(*pi)::x;
int c = decltype(s)::nope;
typename decltype(s)::nope d;
int e = decltype(s.m)::x + 1;
int decltype(i)::*pm;
struct D : decltype(i)::B {};
template <class T> struct X { static const int v = decltype(T{})::value; };
int q = X<int>::v;
