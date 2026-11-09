// [temp.deduct.type]/9, [temp.spec.partial.general]/8-9: the checks on a
// partial specialization whose arguments contain a pack expansion. A pack
// expansion that is not the last argument makes the whole argument list a
// non-deduced context (clang hasPackExpansionBeforeEnd), so none of its
// parameters is deducible (-Wunusable-partial-specialization); the primary
// template's own arguments (`G<Ts...>`) do not specialize it; `H2<Ts...>`
// is not more specialized than `template <class T, class... Ts>`. WP-1053.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 40:30 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 40:20 note "non-deducible template parameter 'T'"
//@ diag: 44:28 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 44:18 note "non-deducible template parameter 'N'"
//@ diag: 46:40 error "class template partial specialization contains template parameters that cannot be deduced; this partial specialization will never be used"
//@ diag: 46:17 note "non-deducible template parameter 'T'"
//@ diag: 46:29 note "non-deducible template parameter 'Us'"
//@ diag: 48:40 error "class template partial specialization contains template parameters that cannot be deduced; this partial specialization will never be used"
//@ diag: 48:17 note "non-deducible template parameter 'T'"
//@ diag: 48:29 note "non-deducible template parameter 'Us'"
//@ diag: 51:40 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 51:29 note "non-deducible template parameter 'Us'"
//@ diag: 53:37 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 53:27 note "non-deducible template parameter 'N'"
//@ diag: 56:31 error "class template partial specialization does not specialize any template argument; to define the primary template, remove the template argument list"
//@ diag: 56:31 error "redefinition of 'G'"
//@ diag: 55:31 note "previous definition is here"
//@ diag: 58:40 error "class template partial specialization does not specialize any template argument; to define the primary template, remove the template argument list"
//@ diag: 58:40 error "redefinition of 'H3'"
//@ diag: 57:40 note "previous definition is here"
//@ diag: 61:31 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 60:40 note "template is declared here"
//@ diag: 63:31 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 62:40 note "template is declared here"
//@ diag: 63:31 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 63:20 note "non-deducible template parameter 'Ts'"
template <class... Ts> struct tup {};
// A pack expansion that is not last: no parameter is deducible.
template <class... Ts> struct F { static constexpr int v = 0; };
template <class... T> struct F<T..., int> { static constexpr int v = 1; };
// It still matches when its pack is empty (as clang).
static_assert(F<int>::v == 1 && F<char, int>::v == 0);
template <int... N> struct I {};
template <int... N> struct I<N..., 0> {};
template <class T, class... Ts> struct X {};
template <class T, class... Us> struct X<tup<T, Us..., int>> {};
template <class... Ts> struct Z {};
template <class T, class... Us> struct Z<T, Us..., int, T> {};
// Not deducible otherwise.
template <class T, class U = int> struct W {};
template <class T, class... Us> struct W<T *> {};
template <class T> struct Y {};
template <class T, int... N> struct Y<T[sizeof...(N)]> {};
// The primary template's own arguments.
template <class... Ts> struct G {};
template <class... Ts> struct G<Ts...> {};
template <class T, class... Ts> struct H3 {};
template <class T, class... Ts> struct H3<T, Ts...> {};
// Not more specialized than the primary template.
template <class T, class... Ts> struct H2 {};
template <class... Ts> struct H2<Ts...> {};
template <class T, class... Ts> struct J {};
template <class... Ts> struct J<Ts..., int> {};
// Valid.
template <class... Ts> struct H {};
template <class T, class... Ts> struct H<T, Ts...> {};
template <class T, class... Ts> struct H4 {};
template <class... Ts> struct H4<int, Ts...> {};
template <class... Ts> struct H5 {};
template <class... Ts> struct H5<Ts *...> {};
template <class... Ts> struct H6 {};
template <class... Ts, class U> struct H6<U, Ts...> {};
template <class T> struct Y<tup<T *>> {};
template <class... Us> struct Y<tup<Us *...>> {};
template <class... Us> struct Y<void(Us...)> {};
template <class T, class... Us> struct Y<tup<T, tup<Us...>>> {};
template <int... N> struct I<0, N...> {};
