// [expr.prim.req.type]/1, [temp.names]/9, [temp.constr.decl]/3: a
// type-requirement naming a specialization of a constrained alias (or
// class) template is satisfied only if the template's associated
// constraints are; an unsatisfied one is a substitution failure in the
// immediate context, so the requires-expression is false. libc++'s
// `std::projected` is such an alias (projected.compile.pass.cpp; WP-309).
//@ expect: compile-ok
//@ strict-diags
template <class T> concept Four = sizeof(T) == 4;
template <class T> concept Ptr = requires(T t) { *t; };

template <Four T> using A = T;
template <Four T> struct B {};
template <class T> struct impl { using type = T *; };
template <Ptr I, Four F> using P = typename impl<I>::type;

template <class T> constexpr bool has_a = requires { typename A<T>; };
template <class T> constexpr bool has_b = requires { typename B<T>; };
template <class I, class F> constexpr bool has_p = requires { typename P<I, F>; };

static_assert(has_a<int>);
static_assert(!has_a<char>);
static_assert(has_b<int>);
static_assert(!has_b<char>);
static_assert(has_p<int *, int>);
static_assert(!has_p<int, int>);    // first constraint fails
static_assert(!has_p<int *, char>); // second constraint fails

// The constraint is checked in the immediate context of deduction too.
template <class T> constexpr int pick(A<T> *) { return 1; }
template <class T> constexpr int pick(...) { return 2; }
static_assert(pick<int>(nullptr) == 1);
static_assert(pick<char>(nullptr) == 2);
