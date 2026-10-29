// [over.match.list]: list-initialization tries the initializer-list
// constructors with the list as one argument first, then all
// constructors with the elements; an empty list value-initializes
// through the default constructor. (Building the initializer_list
// object itself is not implemented yet: only the selection is tested.)
//@ expect: compile-ok
//@ diag: 24:36 warning "braces around scalar initializer"
namespace std {
template <class E> class initializer_list {
  const E *b;
  decltype(sizeof 0) n;

public:
  constexpr initializer_list() : b(nullptr), n(0) {}
};
} // namespace std
struct S {
  S(std::initializer_list<int>);
  S(int, int);
  S();
};
char f(S);
long f(long);
static_assert(__is_same(decltype(f({1})), long));
struct T {
  T(int, int);
  T(double, double);
};
char g(T);
static_assert(__is_same(decltype(g({1, 2})), char));
S b{};
T d{1, 2};
// Phase 1 finds no viable initializer-list constructor: phase 2.
struct U {
  U(std::initializer_list<int *>);
  U(int, int);
};
U u{1, 2};
U v = {1, 2};
char h(U);
long h(...);
static_assert(__is_same(decltype(h({1, 2})), char));
T e = {1.0, 2.0};
