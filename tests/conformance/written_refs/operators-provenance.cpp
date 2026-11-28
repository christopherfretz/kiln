// Operator lookup provenance and built-in candidates (spec/ast.md §18.13,
// WP-2920, WP-3701): non-member operator entries carry `how=adl`, `how=both`
// (when ordinary lookup and ADL both find the selected function) or
// `unqualified`, plus the associated namespaces/classes/enums of the
// operands (`adl-ns=`, `adl-cls=`, `adl-enum=`), including for rewritten and
// reversed comparisons; member operators carry `how=member` with no ADL set;
// built-in candidates selected by operator overload resolution on a class or
// enumeration operand carry `(builtin) ... how=builtin` and the associated
// set, while operators on fundamental/pointer operands carry no entry;
// dependent operators in templates follow the same rule per specialization.
//@ std: c++20
//@ written-refs: operators-provenance.refs
namespace n {
struct S {};
enum E { e0 };
struct Wrap {
  Wrap(E);
};
bool operator==(S, S);
int operator+(Wrap, int);
} // namespace n

namespace mem {
struct M {
  bool operator==(const M &) const;
};
} // namespace mem

namespace hid {
struct H {
  friend bool operator==(const H &, int);
};
} // namespace hid

namespace cmp {
struct Ord {
  friend bool operator<(Ord, int);
};
struct C {
  friend Ord operator<=>(C, C);
};
} // namespace cmp

namespace m {
struct Y {};
} // namespace m
namespace t {
template <class T> struct X {
  operator int() const;
};
template <class T> bool operator==(X<T>, X<T>);
} // namespace t

namespace sc {
enum class Sc { s0 };
} // namespace sc

namespace u {
constexpr int operator""_u(unsigned long long v) { return static_cast<int>(v); }
} // namespace u

constexpr int operator""_g(unsigned long long v) { return static_cast<int>(v); }

template <class T> bool dep_eq(T a, T b) { return a == b; }
template <class T> int dep_add(T a) { return a + 1; }

void use_shape(n::S a, n::S b, n::E e, int *p) {
  bool r1 = a == b;
  bool r2 = e == e;
  bool r3 = b != a;
  using n::operator==;
  bool r4 = a == b;
  int r5 = e + 1;
  int r6 = 1 + 2;
  int *r7 = p + 1;
  (void)r1;
  (void)r2;
  (void)r3;
  (void)r4;
  (void)r5;
  (void)r6;
  (void)r7;
}

void use_udir(n::S a, n::S b) {
  using namespace n;
  bool r = a == b;
  (void)r;
}

void use_other(mem::M m1, mem::M m2, hid::H h, cmp::C c1, cmp::C c2, t::X<m::Y> x1,
               t::X<m::Y> x2, sc::Sc s1, sc::Sc s2) {
  bool r1 = m1 == m2;
  bool r2 = h == 1;
  bool r3 = 1 == h;
  bool r4 = c1 < c2;
  bool r5 = x1 == x2;
  int r6 = x1 + 1;
  bool r7 = s1 == s2;
  int r8 = 1_g;
  using u::operator""_u;
  int r9 = 2_u;
  (void)r1;
  (void)r2;
  (void)r3;
  (void)r4;
  (void)r5;
  (void)r6;
  (void)r7;
  (void)r8;
  (void)r9;
}

void use_udl_dir() {
  using namespace u;
  int r = 3_u;
  (void)r;
}

void use_dep(n::S a, n::S b, n::E e, mem::M m1, mem::M m2, t::X<m::Y> x) {
  (void)dep_eq(a, b);
  (void)dep_eq(e, e);
  (void)dep_eq(m1, m2);
  (void)dep_eq(1, 2);
  (void)dep_add(x);
  (void)dep_add(e);
  (void)dep_add(1);
}

using n::operator==;
template <class T> bool dep_eq_both(T a, T b) { return a == b; }
void use_dep_both(n::S a, n::S b) { (void)dep_eq_both(a, b); }
