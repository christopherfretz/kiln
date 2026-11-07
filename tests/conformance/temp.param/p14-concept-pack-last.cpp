// [temp.param]/14 for concepts (P2841R7): a template parameter pack of a
// concept's head, type, concept or variable template parameter alike, must
// be its last parameter. As in clang 19 and 22 the default-argument
// ordering rule is not applied to a concept's head (X1, X3), and a
// function template's pack need not be last (spec/sema-templates.md
// §2.1.1). WP-771.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:20 error "template parameter pack must be the last template parameter"
//@ diag: 16:39 error "template parameter pack must be the last template parameter"
//@ diag: 17:36 error "template parameter pack must be the last template parameter"
//@ diag: 18:18 error "template parameter pack must be the last template parameter"
template <class T> concept Int = __is_same(T, int);
template <class... Ts, class U> concept X2 = true;
template <template <class> concept... Cs, class U> concept X4 = true;
template <template <class> auto... Vs, class U> concept X6 = true;
template <int... Ns, template <class> concept C> concept X8 = true;
// Accepted.
template <class T = int, class U> concept X1 = true;
template <template <class> concept C = Int, class U> concept X3 = C<U>;
template <class U, template <class> concept... Cs> concept X5 = (Cs<U> && ...);
template <template <class> concept... Cs, class U> void f(U);
static_assert(X1<int, int>);
static_assert(X3<Int, int>);
static_assert(X5<int, Int>);
