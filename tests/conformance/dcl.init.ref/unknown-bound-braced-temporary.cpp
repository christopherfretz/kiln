// [dcl.init.ref]/5.4.2, [dcl.array]/7 (P0388R4): a reference to an array
// of unknown bound initialized by a braced list binds a lifetime-extended
// array temporary of the list's length, and the variable's type becomes a
// reference to that array ([class.temporary]/6); one bound to an lvalue
// keeps its type. Also at block scope, static, constexpr and in a template.
//@ strict-diags
using A = int[];
const A& r1 = {1, 2};
static_assert(sizeof(r1) == 2 * sizeof(int));
constexpr const int (&r2)[] = {1, 2, 3};
static_assert(sizeof(r2) == 3 * sizeof(int) && r2[2] == 3);
const int (&&r3)[] = {4};
static_assert(sizeof(r3) == sizeof(int));
extern const int arr[];
const int (&r4)[] = arr;
const int arr[2] = {};
void f() {
  const int (&l)[] = {1, 2};
  static_assert(sizeof(l) == 2 * sizeof(int));
  static constexpr const long (&s)[] = {1, 2, 3};
  static_assert(sizeof(s) == 3 * sizeof(long) && s[1] == 2);
}
template <class T> void g() {
  const T (&l)[] = {1, 2};
  static_assert(sizeof(l) == 2 * sizeof(T));
}
template void g<long>();
struct D { int v; };
constexpr const D& d = {5};
static_assert(d.v == 5);
constexpr const D (&da)[] = {{1}, {2}};
static_assert(sizeof(da) == 2 * sizeof(D) && da[1].v == 2);
static_assert(__is_same(decltype(r1), const int (&)[2]));
static_assert(__is_same(decltype(r3), const int (&&)[1]));
static_assert(__is_same(decltype(r4), const int (&)[]));
