// [class.pre]/1, [temp.inst]/3: the class-virt-specifier `final` of a class
// template (or partial specialization) is part of every specialization's
// definition, so `__is_final` of an implicit instantiation is true and
// deriving from one is an error. libc++'s compressed-pair padding of a final
// allocator (`containers/associative/map/abi.compile.pass.cpp`) relies on it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:12 error "base 'F' is marked 'final'"
//@ diag: 10:27 note "'F' declared here"
template <class T> struct F final {};
template <class T> struct P {};
template <class T> struct P<T*> final {};
template <class T> struct N {};

static_assert(__is_final(F<int>));
static_assert(__is_final(P<int*>));
static_assert(!__is_final(P<int>));
static_assert(!__is_final(N<int>));

// libc++'s `__is_reference_or_unpadded_object`: an empty final class is
// padded.
template <class T>
constexpr bool unpadded = (__is_empty(T) && !__is_final(T)) || sizeof(T) == __datasizeof(T);
static_assert(!unpadded<F<char>> == (sizeof(F<char>) != __datasizeof(F<char>)));
static_assert(unpadded<N<char>>);

struct D : F<int> {};
