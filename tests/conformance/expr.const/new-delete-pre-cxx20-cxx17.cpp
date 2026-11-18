// C++17 [expr.const]/2.18-2.19: a new-expression and a delete-expression
// are not core constant expressions before C++20 (P0784R7 made them
// constant). Covered: scalar and array new and delete, an allocation on the
// branch a constant evaluation takes, deleting a null pointer, a default
// member initializer (also of a class template), and placement new through
// std::construct_at. clang 22 rejects each with its note; a function whose
// every path allocates also never produces a constant expression. GCC 16.1
// rejects all but deleting a null pointer (accepted): the text decides.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -std=c++17
//@ strict-diags
//@ diag: 43:15 error "constexpr function never produces a constant expression [-Winvalid-constexpr]"
//@ diag: 43:35 note "dynamic memory allocation is not permitted in constant expressions until C++20"
//@ diag: 44:15 error "constexpr variable 's' must be initialized by a constant expression"
//@ diag: 43:35 note "dynamic memory allocation is not permitted in constant expressions until C++20"
//@ diag: 44:19 note "in call to 'scalar()'"
//@ diag: 46:15 error "constexpr function never produces a constant expression [-Winvalid-constexpr]"
//@ diag: 46:34 note "dynamic memory allocation is not permitted in constant expressions until C++20"
//@ diag: 47:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 46:34 note "dynamic memory allocation is not permitted in constant expressions until C++20"
//@ diag: 47:19 note "in call to 'array()'"
//@ diag: 58:15 error "constexpr variable 't' must be initialized by a constant expression"
//@ diag: 51:14 note "dynamic memory allocation is not permitted in constant expressions until C++20"
//@ diag: 58:19 note "in call to 'sometimes(true)'"
//@ diag: 62:15 error "constexpr variable 'n' must be initialized by a constant expression"
//@ diag: 60:31 note "dynamic memory allocation is not permitted in constant expressions until C++20"
//@ diag: 61:24 note "in call to 'drop(nullptr)'"
//@ diag: 62:19 note "in call to 'null()'"
//@ diag: 66:15 error "constexpr variable 'm' must be initialized by a constant expression"
//@ diag: 64:26 note "dynamic memory allocation is not permitted in constant expressions until C++20"
//@ diag: 65:33 note "in call to 'Member()'"
//@ diag: 66:19 note "in call to 'member()'"
//@ diag: 70:15 error "static assertion expression is not an integral constant expression"
//@ diag: 68:44 note "dynamic memory allocation is not permitted in constant expressions until C++20"
//@ diag: 69:41 note "in call to 'TMember()'"
//@ diag: 70:15 note "in call to 'tmember()'"
//@ diag: 78:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 75:61 note "dynamic memory allocation is not permitted in constant expressions until C++20"
//@ diag: 77:34 note "in call to 'construct_at<int>(&i)'"
//@ diag: 78:19 note "in call to 'construct(5)'"

constexpr int scalar() { int *p = new int(1); int r = *p; delete p; return r; }
constexpr int s = scalar();

constexpr int array() { int *p = new int[3]{1, 2, 3}; int r = p[1]; delete[] p; return r; }
constexpr int a = array();

constexpr int sometimes(bool b) {
  if (b) {
    int *p = new int(2);
    int r = *p;
    delete p;
    return r;
  }
  return 0;
}
constexpr int t = sometimes(true);

constexpr void drop(int *p) { delete p; }
constexpr int null() { drop(nullptr); return 1; }
constexpr int n = null();

struct Member { int *p = new int(4); };
constexpr int member() { Member m; int r = *m.p; delete m.p; return r; }
constexpr int m = member();

template <class T> struct TMember { T *p = new T(); };
constexpr bool tmember() { TMember<int> m; bool r = *m.p == 0; delete m.p; return r; }
static_assert(tmember(), "");

typedef decltype(sizeof 0) size_t;
void *operator new(size_t, void *p) noexcept;
namespace std {
template <class T> constexpr T *construct_at(T *p) { return ::new ((void *)p) T(); }
}
constexpr int construct(int i) { std::construct_at(&i); return i; }
constexpr int c = construct(5);
