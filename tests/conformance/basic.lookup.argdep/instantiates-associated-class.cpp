// [basic.lookup.argdep]/3, [temp.inst]/3: an associated class that is a
// class template specialization not yet instantiated is instantiated by
// argument-dependent lookup (clang `addAssociatedClassesAndNamespaces`
// calls `isCompleteType`), so its hidden friends and its bases take part.
// libc++ test_iterators.h `sentinel_wrapper<It>::base_` is
// `decltype(base(std::declval<It>()))`, where only ADL completes `It`.
// WP-223.
//@ expect: run
//@ stdout: 1 2 3
//@ exit: 0
extern "C" int printf(const char*, ...);

template <class T> T&& declval();
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };

// The generic fallback: a worse match than a hidden friend once found.
template <class I> constexpr I base(I i) { return i; }

template <class It> class fwd {
  It it_;
public:
  constexpr explicit fwd(It it) : it_(it) {}
  friend constexpr It base(const fwd& i) { return i.it_; }
};
static_assert(same<decltype(base(declval<fwd<int*>>())), int*>::value);

// Through a pointer, a template argument and a base class.
namespace n {
template <class T> struct B { friend constexpr int get(const B*) { return 2; } };
template <class T> struct D : B<T> {};
template <class T> struct Tag { friend constexpr int tag(Tag<T>*) { return 3; } };
}
template <class T> struct W {};
constexpr int get(...) { return 0; }
constexpr int tag(...) { return 0; }
static_assert(get(static_cast<n::D<int>*>(nullptr)) == 2);
static_assert(tag(static_cast<n::Tag<long>*>(nullptr)) == 3);

// sentinel_wrapper's shape.
template <class It> class sw {
public:
  constexpr explicit sw(const It& it) : base_(base(it)) {}
  constexpr bool operator==(const It& other) const { return base_ == base(other); }
private:
  decltype(base(declval<It>())) base_;
};
static_assert(same<decltype(base(declval<fwd<long*>>())), long*>::value);

int main() {
  int a[2] = {1, 2};
  fwd<int*> i(a);
  sw<fwd<int*>> s(i);
  printf("%d %d %d\n", int(s == i), get(static_cast<n::D<int>*>(nullptr)),
         tag(static_cast<n::Tag<long>*>(nullptr)));
}
