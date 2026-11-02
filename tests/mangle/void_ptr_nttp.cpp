// `void *` non-type template arguments ([temp.arg.nontype], [temp.type]/2.4):
// the argument keeps the object it designates, so `(void *)&s` and
// `(void *)&s.first` are different arguments, mangled as clang does with a
// conversion `cv <pointer type>` of the designated subobject's address.
// An array variable and its first element are the same argument (clang
// ignores the decay).
struct E {};
struct S : E { int first; const int c; int arr[3]; };
S s{{}, 1, 2};
int arr[3];
const int ca[2] = {1, 2};
template <const void *P> int f() { return 0; }
template <void *P> int g() { return 0; }
template <const int *P> int h() { return 0; }
struct VP { const void *p; };
template <VP V> int k() { return 0; }
using CVP = const void *;
constexpr CVP p1 = &s, p2 = &s.first, p3 = &s.c, p4 = static_cast<const E *>(&s),
              p5 = s.arr, p6 = &s.arr, p7 = &s.arr[2], p8 = arr, p9 = &arr, p10 = arr + 1,
              p11 = ca, p12 = &ca[1], p13 = &ca, p14 = arr + 3, p15 = &arr + 1;
constexpr void *q1 = &s, *q2 = &s.first;
int x = f<p1>() + f<p2>() + f<p3>() + f<p4>() + f<p5>() + f<p6>() + f<p7>() + f<p8>() +
        f<p9>() + f<p10>() + f<p11>() + f<p12>() + f<p13>() + f<p14>() + f<p15>() +
        g<q1>() + g<q2>() + h<&s.c>() + h<arr + 3>() + k<VP{p1}>() + k<VP{p2}>();
