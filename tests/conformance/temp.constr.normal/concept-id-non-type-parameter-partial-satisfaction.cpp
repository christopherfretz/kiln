// [temp.constr.normal]/1.4, [temp.constr.atomic]/1-3, [temp.arg.nontype]/2:
// a concept-id's argument for a non-type parameter is part of the
// parameter mappings of the atoms that name the parameter only. When it is
// invalid (its substitution fails, or it does not convert to the
// parameter's type: a narrowing, a type mismatch) only those atoms are
// unsatisfied, so a disjunction whose other operand holds is satisfied.
// The same for a trailing non-type pack and for a non-type parameter whose
// type is an earlier parameter. clang 22 and GCC 16.1 agree. Found running
// GCC's C++ testsuite (WP-1598).
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
template <class T> concept IsInt = __is_same(T, int);
template <class T, int N> concept C = IsInt<T> || (N > 0);

// A failed substitution.
template <class T> requires C<T, T::value> constexpr int f(T) { return 1; }
constexpr int f(...) { return 2; }
static_assert(f(1) == 1 && f(1.0) == 2);

// A narrowing conversion.
template <class T, unsigned char N> concept D = IsInt<T> || (N > 0);
template <class T> requires D<T, sizeof(T) * 1000> constexpr int g(T) { return 1; }
constexpr int g(...) { return 2; }
static_assert(g(1) == 1 && g(1.0) == 2);

// A type mismatch.
template <class T> requires C<T, (T*)nullptr> constexpr int h(T) { return 1; }
constexpr int h(...) { return 2; }
static_assert(h(1) == 1 && h(1.0) == 2);

// A conjunction: the atom naming the failed parameter is unsatisfied.
template <class T, int N> concept CC = IsInt<T> && (N > 0);
template <class T> requires CC<T, T::value> constexpr int k(T) { return 1; }
constexpr int k(...) { return 2; }
static_assert(k(1) == 2);

// No atom names the failed parameter.
template <class T, int N> concept U = IsInt<T>;
template <class T> requires U<T, T::value> constexpr int u(T) { return 1; }
constexpr int u(...) { return 2; }
static_assert(u(1) == 1);

// A trailing non-type pack.
template <class T, int... Ns> concept H = IsInt<T> || ((Ns > 0) && ...);
template <class T> requires H<T, 1, T::value> constexpr int p(T) { return 1; }
constexpr int p(...) { return 2; }
static_assert(p(1) == 1 && p(1.0) == 2);

// A non-type parameter whose type is an earlier parameter: invalid with
// it, and invalid on its own.
template <class T, class V, V N> concept K = IsInt<T> || (N > 0);
template <class T> requires K<T, typename T::type, 1> constexpr int q(T) { return 1; }
constexpr int q(...) { return 2; }
static_assert(q(1) == 1 && q(1.0) == 2);
template <class T> requires K<T, int, T::value> constexpr int r(T) { return 1; }
constexpr int r(...) { return 2; }
static_assert(r(1) == 1 && r(1.0) == 2);
