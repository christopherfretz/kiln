// [temp.deduct.general]/8 (P4149R1, Croydon DR): an invalid type or
// expression formed by substitution in the immediate context of the
// function type and its template parameter declarations is a substitution
// failure; the overload set falls back to the other candidates. The
// wording's example: `T* fun(T&&)` with T = int& forms a pointer to a
// reference, so `fun(i)` selects the ellipsis overload.
//@ expect: compile-ok
template <class T> T* fun(T&&);       // #1
template <class T> long fun(...);     // #2 (never deduced: T is explicit below)
long fun(...);                        // #3
int i;
static_assert(__is_same(decltype(fun(i)), long));
static_assert(__is_same(decltype(fun(0)), int*));

// Expressions in the return type are in the immediate context.
template <class T> auto ret(T t) -> decltype(t.member());
int ret(...);
static_assert(__is_same(decltype(ret(0)), int));

// So are the parameter types and the template parameter declarations.
template <class T> void par(T, typename T::type* = nullptr);
int par(...);
static_assert(__is_same(decltype(par(0)), int));
template <class T, class = typename T::type> void tpd(T);
int tpd(...);
static_assert(__is_same(decltype(tpd(0)), int));

// A lambda-expression's signature (not its body) in the return type is
// substituted in the immediate context too.
template <class T> auto lam(T) -> decltype([](typename T::type) {});
int lam(...);
static_assert(__is_same(decltype(lam(0)), int));
