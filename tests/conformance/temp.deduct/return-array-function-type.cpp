// [temp.deduct.general]/11.10: substituting into a function template's signature so that its
// return type is an array or function type is a deduction failure (libc++ bit_cast.compile:
// `bit_cast<char[2]>(from)` is not a valid expression), with clang's substitution-failure note
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:24 error "no matching function for call to 'cast'"
//@ diag: 10:31 note "candidate template ignored: substitution failure [with T = char[2], F = From]: function cannot return array type 'char[2]'"
//@ diag: 21:24 error "no matching function for call to 'cast'"
//@ diag: 10:31 note "candidate template ignored: substitution failure [with T = int (), F = From]: function cannot return function type 'int ()'"
template <class T, class F> T cast(const F&);
template <class T, class F> concept castable = requires(F f) { cast<T>(f); };
struct From { char a, b; };
static_assert(!castable<char[2], From>);
static_assert(!castable<int(), From>);
static_assert(castable<char (&)[2], From>);
static_assert(castable<int (*)(), From>);
template <class T> constexpr auto g(int) -> decltype(cast<T>(0), true) { return true; }
template <class T> constexpr bool g(...) { return false; }
static_assert(!g<char[2]>(0) && g<int>(0));
void f(From x) { (void)cast<char[2]>(x); }
void h(From x) { (void)cast<int()>(x); }
