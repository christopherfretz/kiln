// [temp.constr.normal]/1.4, [temp.constr.atomic]/1-3, [temp.constr.op]:
// a concept-id whose concept has a parameter pack substitutes the pack's
// arguments into the parameter mappings of the atoms that name the pack.
// When one of them is invalid, only those atoms are unsatisfied; an atom
// that names no failed parameter is evaluated, so a disjunction with a
// satisfied alternative is satisfied, also through a nested concept-id
// (`cfo<T, T>` of a concept whose own argument failed) and a C++26
// fold expanded constraint ([temp.constr.fold]). GCC 16.1 and clang 22
// agree. Found running GCC's C++ testsuite (WP-1579).
//@ std: c++26
//@ requires: clang>=22
//@ strict-diags
template <class T, class... A> concept P = true;
template <class T> requires P<T, typename T::nope> constexpr int p(T) { return 1; }
static_assert(p(0) == 1);

template <class T, class... A> concept PA = (sizeof(A) + ... + 0) > 0;
template <class T> requires PA<T, typename T::nope> constexpr int pa(T) { return 1; }
constexpr int pa(...) { return 2; }
static_assert(pa(0) == 2);

template <class T, class... A> concept PO = sizeof(T) > 0 || (sizeof(A) + ... + 0) > 0;
template <class T> requires PO<T, typename T::nope> constexpr int po(T) { return 1; }
constexpr int po(...) { return 2; }
static_assert(po(0) == 1);

template <class T, class... A> concept PC = sizeof(T) > 100 && (sizeof(A) + ... + 0) > 0;
template <class T> requires PC<T, typename T::nope> constexpr int pc(T) { return 1; }
constexpr int pc(...) { return 2; }
static_assert(pc(0) == 2);

// Nested concept-ids naming a pack parameter (`constructible_from<T, T>`).
template <class T, class... A> concept cf = __is_constructible(T, A...);
template <class T, class... A> concept cfo = true || __is_constructible(T, A...);
template <class T> concept mc = cf<T, T>;
template <class T> concept mo = cfo<T, T>;
template <class T> requires mo<typename T::nope> constexpr int n1(T) { return 1; }
static_assert(n1(0) == 1);
template <class T> requires mc<typename T::nope> constexpr int n2(T) { return 1; }
constexpr int n2(...) { return 2; }
static_assert(n2(0) == 2);

// A fold expanded constraint over the pack.
template <class... A> concept All = ((sizeof(A) > 0) && ...);
template <class T, class... A> concept FA = sizeof(T) > 0 || All<A...>;
template <class T> requires FA<T, typename T::nope> constexpr int fa(T) { return 1; }
static_assert(fa(0) == 1);
template <class T, class... A> concept FB = sizeof(T) > 100 || All<A...>;
template <class T> requires FB<T, typename T::nope> constexpr int fb(T) { return 1; }
constexpr int fb(...) { return 2; }
static_assert(fb(0) == 2);

// Satisfied with valid arguments.
struct V { using nope = int; };
static_assert(pa(V{}) == 1 && pc(V{}) == 2 && fb(V{}) == 1);
