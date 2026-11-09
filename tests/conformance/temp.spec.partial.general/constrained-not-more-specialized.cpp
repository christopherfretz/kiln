// [temp.spec.partial.general]/9.2, [temp.func.order]/6.2.2: a partial
// specialization that is equally specialized as the primary template by
// deduction is more specialized only when it is more constrained
// ([temp.constr.order]); type-constraints count as requires-clauses. Clang
// checkMoreSpecializedThanPrimary compares the associated constraints when
// the template heads are equivalent and the arguments are the primary's.
// WP-1053.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:23 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 20:23 note "template is declared here"
//@ diag: 24:23 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 23:23 note "template is declared here"
//@ diag: 26:20 error "variable template partial specialization is not more specialized than the primary template"
//@ diag: 25:20 note "template is declared here"
template <class T> concept C = sizeof(T) > 1;
template <class T> concept D = C<T> && sizeof(T) > 2;
// Equally specialized, the primary more constrained: invalid.
template <D T> struct K {};
template <C T> struct K<T> {};
// Equally specialized and equally constrained: invalid.
template <C T> struct B {};
template <C T> struct B<T> {};
template <D T> int vk = 0;
template <C T> int vk<T> = 1;
// Equally specialized, the partial specialization more constrained: valid.
template <class T> struct A { static constexpr int v = 0; };
template <class T> requires C<T> struct A<T> { static constexpr int v = 1; };
template <C T> struct A<T *> { static constexpr int v = 2; };
template <D T> struct A<T *> { static constexpr int v = 3; };
static_assert(A<char>::v == 0);
static_assert(A<int>::v == 1);
static_assert(A<short *>::v == 2);
static_assert(A<int *>::v == 3);
template <C T> struct M {};
template <D T> struct M<T> {};
template <class T> constexpr int vm = 0;
template <class T> requires C<T> constexpr int vm<T> = 1;
static_assert(vm<char> == 0 && vm<int> == 1);
// More specialized by deduction: constraints are not compared.
template <class T> requires C<T> struct N { static constexpr int v = 0; };
template <class T> struct N<T *> { static constexpr int v = 1; };
static_assert(N<int *>::v == 1);
