// [temp.deduct.general]/8: a private member named in a function
// template's signature is a substitution failure (no viable function),
// not accepted; a partial specialization whose argument names it does
// not match. Naming the member at the use site itself is still checked
// there ([class.access.general]/4). Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:19 error "'type' is a private member of 'C'"
//@ diag: 18:16 note "declared private here"
//@ diag: 22:9 error "no matching function for call to 'sf'"
//@ diag: 21:23 note "candidate template ignored: substitution failure [with T = C]: 'type' is a private member of 'C'"
//@ diag: 23:15 error "static assertion failed due to requirement 'H<C, void>::value == 1'"
//@ diag: 23:27 note "expression evaluates to '0 == 1'"
template<class, class = void> struct H { static const int value = 0; };
template<class T> struct H<T, typename T::type> { static const int value = 1; };
struct C {
private:
  typedef void type;
};
int x = sizeof(C::type*);
template<class T> int sf(typename T::type*);
int y = sf<C>(0);
static_assert(H<C>::value == 1, "");
