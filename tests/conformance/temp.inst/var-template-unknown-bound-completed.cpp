// [temp.inst]/7, [dcl.array]/3: a variable template specialization of
// unknown bound is completed by its instantiated initializer where the
// type needs to be complete (clang `getCompletedType`): the operand of
// sizeof and alignof, an argument deduced against a reference parameter,
// a reference or `auto&` bound to it, the range of a range-based for.
// Merely naming a non-constexpr one does not instantiate it: decltype
// sees `T[]` until something completes it, then `T[N]`. The bound comes
// from a list, a string literal or a pack expansion; a partial or
// explicit specialization has its own bound; a static data member
// template and a member template of a class template behave the same (WP-907).
//@ skip(gcc): pins clang's choice of when the bound is completed (GCC 16.1's decltype sees T[N] at once; [temp.inst]/7 leaves the point open)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 23:15 warning "'alignof' applied to an expression is a GNU extension"
template <class, class> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

template <class T> T arr[] = {1, 2};
static_assert(same<decltype(arr<int>), int[]>);
static_assert(sizeof(arr<long>) == 2 * sizeof(long));
static_assert(same<decltype(arr<long>), long[2]>);
static_assert(sizeof((arr<char>)) == 2);
static_assert(alignof(arr<short>) == alignof(short));

template <class T> constexpr T carr[] = {1, 2, 3};
static_assert(sizeof carr<int> == 3 * sizeof(int));
static_assert(same<decltype(carr<int>), const int[3]>);
static_assert(sizeof carr<char> == 3);

template <class T> constexpr char str[] = "abcd";
static_assert(sizeof(str<int>) == 5);

template <class... T> constexpr int packed[] = {sizeof(T)...};
static_assert(sizeof(packed<char, short, int>) == 3 * sizeof(int));

template <class T> constexpr T carr<T *>[] = {1, 2, 3, 4, 5};
static_assert(sizeof(carr<int *>) == 5 * sizeof(int));
template <> constexpr int carr<long>[] = {9};
static_assert(sizeof(carr<long>) == sizeof(int));

struct S {
  template <class T> static constexpr T m[] = {1, 2, 3, 4};
  template <class T> static const T n[];
};
template <class T> const T S::n[] = {1, 2};
static_assert(sizeof(S::m<int>) == 4 * sizeof(int));
static_assert(sizeof(S::n<int>) == 2 * sizeof(int));

template <class U> struct C { template <class T> static const T m[]; };
template <class U> template <class T> const T C<U>::m[] = {1, 2, 3};
static_assert(sizeof(C<int>::m<char>) == 3);

template <class T> unsigned long size_of() { return sizeof(arr<T>); }
template <class T> constexpr unsigned long count_v = sizeof arr<T> / sizeof arr<T>[0];
static_assert(count_v<double> == 2);

template <class T, unsigned long N> constexpr unsigned long count(T (&)[N]) { return N; }
template <class T> T vals[] = {1, 2, 3};
int sum() {
  int s = 0;
  for (int x : vals<int>)
    s += x;
  return s;
}
unsigned long deduced() { return count(vals<long>); }
unsigned long bound() {
  auto &q = vals<short>;
  unsigned (&r)[3] = vals<unsigned>;
  return sizeof q + sizeof r + size_of<float>();
}
