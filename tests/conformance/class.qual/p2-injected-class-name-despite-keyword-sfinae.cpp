// [class.qual]/2, [temp.deduct.general]/8: in `typename C::template C<A>` and
// `typename C::C`, not followed by `::`, the name looked up in the class C
// is C's injected-class-name and so names the constructor, not the template
// or the type. clang takes the template or the type with a warning
// (-Winjected-class-name) outside a SFINAE context and fails the
// substitution inside one, so a requirement `typename L::template
// mapping<E>` is not satisfied by `L = mapping<E>`. kiln 1.3.0 took the
// template silently there and missed the static_assert by which libc++'s
// mdspan rejects a layout mapping passed as its layout policy (reduced from
// libc++ 22's `mdspan/mapping.verify.cpp`; WP-1889). Followed by `::` the
// name still names the template (CWG1835), and a base's injected-class-name
// found in a derived class is an ordinary member.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ diag: 49:34 warning "is a constructor name rather than a template name in this context, despite preceding 'template' keyword"
//@ diag: 52:25 note "in instantiation of template class 'S<layout_left::mapping<extents<int>>>' requested here"
//@ diag: 50:25 warning "is a constructor name rather than a type in this context, despite preceding 'typename' keyword"
template <class E> struct extents {};
struct layout_left {
  template <class E> struct mapping { using type = int; };
};
template <class L, class E>
concept has_mapping = requires { typename L::template mapping<E>; };
template <class L>
concept has_member_type = requires { typename L::mapping; };
using M = layout_left::mapping<extents<int>>;
static_assert(has_mapping<layout_left, extents<int>>);
static_assert(!has_mapping<M, extents<int>>);
static_assert(!has_member_type<M>);

// Deduction: the overload whose return type names the template is not viable.
template <class L, class E> typename L::template mapping<E> pick(int);
template <class L, class E> void pick(long);
static_assert(__is_same(decltype(pick<M, extents<int>>(0)), void));
static_assert(__is_same(decltype(pick<layout_left, extents<int>>(0)), M));

template <class L, class E>
concept nested = requires { typename L::template mapping<E>::type; };
static_assert(nested<M, extents<int>>);

template <class T> struct Base {};
struct Derived : Base<int> {};
template <class D>
concept base_named = requires { typename D::template Base<long>; };
static_assert(base_named<Derived>);

template <class T> struct S {
  using t = typename T::template mapping<int>;
  using u = typename T::mapping;
};
static_assert(__is_same(S<M>::t, layout_left::mapping<int>));
static_assert(__is_same(S<M>::u, M));
