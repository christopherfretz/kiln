// [meta.unary.prop] is_empty, [intro.object]/9, [dcl.attr.nouniqueaddr]: a
// `[[no_unique_address]]` member of empty class type is a subobject of zero
// size and leaves its class empty (clang `FieldDecl::isZeroSize`); one of
// array type or of non-empty class type does not. libc++ mdspan `extents`.
struct E {};
struct A { [[no_unique_address]] E e; };
struct B { [[no_unique_address]] E e; [[no_unique_address]] E f; };
struct C { [[no_unique_address]] A a; };
struct D : A { [[no_unique_address]] E e; };
struct N { [[no_unique_address]] int i; };
struct G { [[no_unique_address]] E e[1]; };
template <class T, unsigned long N> struct arr { [[no_unique_address]] T v[N]; };
template <class T> struct arr<T, 0> {};
template <class T, unsigned long N> struct holder {
  [[no_unique_address]] arr<T, N> vals;
};
static_assert(__is_empty(A));
static_assert(__is_empty(B));
static_assert(__is_empty(C));
static_assert(__is_empty(D));
static_assert(!__is_empty(N));
static_assert(!__is_empty(G));
static_assert(__is_empty(holder<int, 0>));
static_assert(!__is_empty(holder<int, 2>));
static_assert(sizeof(A) == 1 && sizeof(C) == 1);
