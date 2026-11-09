// [temp.inst]/7, [dcl.array]/3, [expr.sizeof]/1: a variable template
// specialization of unknown bound with no definition to instantiate (an
// `extern` declaration, or an explicit instantiation declaration of a
// non-constexpr one) stays incomplete for sizeof; an initializer that
// fails to instantiate leaves it incomplete, noted at the operand's
// parenthesis (clang `completeExprArrayBound`'s point of instantiation);
// a bound other than the initializer's fails a static assertion, an
// explicit specialization's bound included. WP-907.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:25 error "invalid application of 'sizeof' to an incomplete type 'int[]'"
//@ diag: 24:31 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 25:25 note "in instantiation of variable template specialization 'bad<int>' requested here"
//@ diag: 25:25 error "invalid application of 'sizeof' to an incomplete type 'int[]'"
//@ diag: 27:15 error "static assertion failed due to requirement 'sizeof (arr<int>) == 3 * sizeof(int)'"
//@ diag: 27:32 note "expression evaluates to '8 == 12'"
//@ diag: 29:15 error "static assertion failed due to requirement 'sizeof (arr<long>) == 2 * sizeof(long)'"
//@ diag: 29:33 note "expression evaluates to '8 == 16'"
//@ diag: 31:25 error "invalid application of 'sizeof' to an incomplete type 'short[]'"
//@ diag: 32:53 error "invalid application of 'sizeof' to an incomplete type 'short[]'"
//@ diag: 33:19 note "in instantiation of function template specialization 'f<short>' requested here"
template <class T> extern T e[];
unsigned long a = sizeof(e<int>);
template <class T> T bad[] = {T::value};
unsigned long b = sizeof(bad<int>);
template <class T> T arr[] = {1, 2};
static_assert(sizeof(arr<int>) == 3 * sizeof(int));
template <> long arr<long>[] = {1};
static_assert(sizeof(arr<long>) == 2 * sizeof(long));
extern template short arr<short>[];
unsigned long c = sizeof(arr<short>);
template <class T> unsigned long f() { return sizeof(arr<T>); }
unsigned long d = f<short>();
