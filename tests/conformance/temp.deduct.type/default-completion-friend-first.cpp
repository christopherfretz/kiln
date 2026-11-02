// [temp.deduct.type]/9, [temp.arg.general]/9, [temp.param]/12: a
// dependent template-id `P` = `V<T>` is completed with `V`'s default
// template arguments (`V<T, A<T>>`) before it is deduced against
// `A` = `V<int, A<int>>`. The defaults are those of the declaration that
// stands for the template, even when the first declaration of `V` is a
// friend redeclaration nested in a class template (deeper parameters,
// no defaults of its own; libc++ `__wrap_iter` befriends `vector` before
// `<__fwd/vector.h>` declares `vector<_Tp, _Alloc = allocator<_Tp>>`).
// Verified against clang 19.
//@ expect: run
template <class T> struct A {};
template <class X> struct W {
  template <class T, class Al> friend class V;
};
template <class T, class Al = A<T> > class V {};

template <class I, class T> int x(V<T>&) { return 2; }
template <class R> int x(R&&) { return 1; }

template <class T, class U> struct same { static constexpr bool v = false; };
template <class T> struct same<T, T> { static constexpr bool v = true; };
template <class I, class T> constexpr bool deduces_int(V<T>*) { return same<T, int>::v; }

static_assert(deduces_int<char>(static_cast<V<int>*>(nullptr)));

int main() {
  V<int> v;
  return x<char>(v) == 2 ? 0 : 1;
}
