//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:26 error "no matching member function for call to 'get'"
//@ diag: 31:27 note "in implicit initialization of binding declaration 'x'"
//@ diag: 27:24 note "candidate function template not viable: requires 1 argument, but 0 were provided"
//@ diag: 39:24 error "no matching function for call to 'get'"
//@ diag: 39:24 note "in implicit initialization of binding declaration 'x'"
//@ diag: 35:22 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 39:27 error "no matching function for call to 'get'"
//@ diag: 39:27 note "in implicit initialization of binding declaration 'y'"
//@ diag: 35:22 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 39:30 error "no matching function for call to 'get'"
//@ diag: 39:30 note "in implicit initialization of binding declaration 'z'"
//@ diag: 35:22 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 54:33 error "implicit instantiation of undefined template 'std::tuple_element<0, D>'"
//@ diag: 54:33 note "in implicit initialization of binding declaration 'x'"
//@ diag: 23:44 note "template is declared here"
// The tuple-like protocol ([dcl.struct.bind]/7): a member `get` template
// that cannot be called is reported once, at the first binding, while a
// free `get` found by ADL is reported per binding, as clang does.
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
} // namespace std

struct A {
  template <int I> int get(int) const { return I; }
};
template <> struct std::tuple_size<A> { static constexpr int value = 3; };
template <unsigned long I> struct std::tuple_element<I, A> { using type = int; };
void member_get() { auto [x, y, z] = A{}; }

namespace n {
struct B {};
template <int I> int get(B &, int);
} // namespace n
template <> struct std::tuple_size<n::B> { static constexpr int value = 3; };
template <unsigned long I> struct std::tuple_element<I, n::B> { using type = int; };
void adl_get() { auto [x, y, z] = n::B{}; }

// A member `get` that is not a template is not the protocol's: ADL is used.
struct C {
  int get() const;
};
template <int I> int get(const C &) { return I; }
template <> struct std::tuple_size<C> { static constexpr int value = 2; };
template <unsigned long I> struct std::tuple_element<I, C> { using type = int; };
void non_template_member() { auto [x, y] = C{}; }

struct D {
  template <int I> int get() const { return I; }
};
template <> struct std::tuple_size<D> { static constexpr int value = 2; };
void no_tuple_element() { auto [x, y] = D{}; }
