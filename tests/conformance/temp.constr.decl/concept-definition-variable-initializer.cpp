// [temp.constr.atomic]/3, [temp.inst]/8: a non-dependent atom of a
// concept's constraint-expression names variables whose initializers
// are needed when it is checked, although the definition is an
// unevaluated operand. Found running GCC's C++ testsuite (WP-1497).
//@ expect: compile-ok
//@ std: c++20
template <int a> struct c { static constexpr int d = a; };
template <class> constexpr bool i = c<1>::d;
struct b;
template <class> concept bb = i<b>;
static_assert(bb<int>);
template <class T> struct S { static constexpr bool v = true; };
template <class> concept cc = S<b>::v && sizeof(int) == 4;
static_assert(cc<int>);
