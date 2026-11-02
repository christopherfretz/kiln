// [temp.deduct.general]/11.1, /11.10, [temp.deduct.general]/8: substituting a
// template argument that makes a function parameter type invalid (`void`, an
// array of `void` or of an abstract class, a pointer to a reference, a
// reference to `void`, a function returning an array) is a deduction failure,
// not a `void` parameter: the candidate is dropped and another is chosen.
// The libc++ `MoveOnly` shape: a deleted `operator,(const MoveOnly&, T)`
// must not be selected for `(x = y), void()`.
//@ expect: run
//@ strict-diags
struct M {
  int v = 0;
  M() = default;
  M(M&&) = default;
  M& operator=(M&&) = default;
  template <class T> friend void operator,(const M&, T) = delete;
  template <class T> friend void operator,(T, const M&) = delete;
};
struct A { virtual void f() = 0; };

template <class T> constexpr int byval(T) { return 1; }
template <class T> constexpr int byval(...) { return 2; }
template <class T> constexpr int named(int, T x) { return 1; }
template <class T> constexpr int named(...) { return 2; }
template <class... T> constexpr int pack(T...) { return 1; }
template <class... T> constexpr int pack(...) { return 2; }
template <class T> constexpr int fnptr(void (*)(T)) { return 1; }
template <class T> constexpr int fnptr(...) { return 2; }
template <class T> constexpr int arr(T[3]) { return 1; }
template <class T> constexpr int arr(...) { return 2; }
template <class T> constexpr int abs_arr(T (*)[2]) { return 1; }
template <class T> constexpr int abs_arr(...) { return 2; }
template <class T> constexpr int ptr_ref(T*) { return 1; }
template <class T> constexpr int ptr_ref(...) { return 2; }
template <class T> constexpr int ref_void(T&) { return 1; }
template <class T> constexpr int ref_void(...) { return 2; }
template <class T> constexpr int ret_arr(T (*)()) { return 1; }
template <class T> constexpr int ret_arr(...) { return 2; }

static_assert(byval<void>(0) == 2);
static_assert(byval<int>(0) == 1);
static_assert(named<void>(0) == 2);
static_assert(pack<int, void>(0, 0) == 2);
static_assert(pack<int>(0) == 1);
static_assert(fnptr<void>(nullptr) == 2);
static_assert(arr<void>(nullptr) == 2);
static_assert(arr<int>(nullptr) == 1);
static_assert(abs_arr<A>(nullptr) == 2);
static_assert(ptr_ref<int&>(nullptr) == 2);
static_assert(ref_void<void>(0) == 2);
static_assert(ret_arr<int[2]>(nullptr) == 2);

int main() {
  M x, y;
  // The built-in comma operator: `T = void` is deduced from the `void()`
  // operand and substituting it fails, for both templates.
  ((x = static_cast<M&&>(y)), void());
  (void(), (x = static_cast<M&&>(y)));
  int p[3] = {};
  return arr<int>(p) == 1 ? 0 : 1;
}
