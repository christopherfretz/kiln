//@ expect: compile-fail
//@ strict-diags
// A non-`bool` atomic constraint that is not type-dependent is diagnosed
// where it is written; the concept (or clause) is invalid and naming it
// adds no further diagnostic (WP-109).
//@ diag: 11:33 error "atomic constraint must be of type 'bool' (found 'int')"
//@ diag: 12:33 error "atomic constraint must be of type 'bool' (found 'unsigned long')"
//@ diag: 13:41 error "atomic constraint must be of type 'bool' (found 'int')"
//@ diag: 14:40 error "atomic constraint must be of type 'bool' (found 'unsigned long')"
// Matches clang 19.
template <class T> concept C1 = 1;
template <class T> concept C2 = sizeof(T);
template <class T> concept C3 = true && 2;
template <class T> requires (sizeof(T) + 0) void f(T) {}
static_assert(C2<int>);
template <class T> concept C4 = (bool)sizeof(T);
static_assert(C4<int>);
template <class T> concept C5 = sizeof(T) > 1 && __is_same(T, T);
static_assert(!C5<char>);
