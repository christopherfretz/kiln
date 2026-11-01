// [meta.unary.prop]/9, /12 (clang's __is_trivially_constructible /
// __is_trivially_assignable): the initialization or assignment is well-formed
// and calls no operation that is not trivial -- the selected constructor or
// assignment operator is trivial, no user-defined conversion runs, and a
// constructed temporary has a trivial destructor. libc++'s
// is_trivially_constructible / is_trivially_assignable use the builtins.
//@ expect: compile-ok
//@ strict-diags
struct Triv { int x; };
struct UserDef { UserDef(); };
struct UserCopy { UserCopy() = default; UserCopy(const UserCopy&); };
struct ConvFrom { ConvFrom() = default; ConvFrom(int); };
struct NTDtor { ~NTDtor(); };
struct Assign { Assign& operator=(const Assign&); };
struct DefAssign { DefAssign& operator=(const DefAssign&) = default; };
struct Conv { operator int() const; };
struct Virt { virtual void f(); };
struct HasNT { UserDef u; };
struct Del { Del() = delete; };
struct Base { int b; };
struct Der : Base {};
struct Mv { Mv(Mv&&) = default; Mv(const Mv&) = delete; };
template <class T> struct W { T t; };
#define C(...) __is_trivially_constructible(__VA_ARGS__)
#define A(...) __is_trivially_assignable(__VA_ARGS__)
static_assert(C(int) && C(int, int) && C(int, long) && C(int, double));
static_assert(C(int&, int&) && !C(int&, int) && C(const int&, int));
static_assert(C(Triv) && C(Triv, Triv) && C(Triv, const Triv&) && C(Triv, int));
static_assert(!C(UserDef) && C(UserCopy) && !C(UserCopy, UserCopy));
static_assert(C(ConvFrom) && !C(ConvFrom, int) && !C(int, Conv));
static_assert(!C(NTDtor) && !C(NTDtor, NTDtor) && !C(NTDtor[2]) && C(NTDtor&&, NTDtor));
static_assert(!C(Virt) && !C(Virt, Virt) && !C(HasNT) && !C(Del) && !C(void) && !C(int[]));
static_assert(C(Der) && C(Base, Der) && C(Base&, Der&) && C(int[3]));
static_assert(C(W<int>) && !C(W<UserDef>) && C(W<int>, W<int>&));
static_assert(C(Mv, Mv) && !C(Mv, Mv&) && C(int*, decltype(nullptr)));
static_assert(A(int&, int) && !A(int, int) && A(int&, double) && !A(const int&, int));
static_assert(A(Triv&, Triv) && A(Triv&, const Triv&) && !A(Assign&, Assign));
static_assert(A(DefAssign&, DefAssign) && A(NTDtor&, NTDtor) && !A(Virt&, Virt) && A(HasNT&, HasNT));
static_assert(!A(int&, Conv) && A(Base&, Der) && A(W<int>&, W<int>) && !A(ConvFrom&, int));
static_assert(!A(void, int) && A(int*&, decltype(nullptr)));
template <class T, class... Args>
constexpr bool tc = __is_trivially_constructible(T, Args...);
template <class T, class U>
constexpr bool ta = __is_trivially_assignable(T, U);
static_assert(tc<Triv, const Triv&> && !tc<UserCopy, const UserCopy&> && ta<Triv&, Triv>);
static_assert(!ta<Assign&, const Assign&>);
