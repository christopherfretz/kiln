// [special]/6, [class.prop]/1, [class.default.ctor]/3, [class.copy.ctor]/11:
// triviality is decided per eligible special member. A member whose
// constraints are unsatisfied, or that is less constrained than another of
// the same kind and parameter-type-list, is not eligible and does not make
// the class non-trivial; a trivial eligible member makes the class "have" a
// trivial one even next to a non-trivial or deleted overload (clang
// `HasTrivialSpecialMembers` / `hasNonTrivial*`, WP-1109). Deleted members
// stay eligible (clang). Oracle-checked: clang++-19 and clang++-22 agree.
//@ args: -Wno-deprecated-builtins
//@ strict-diags

// The probe: a deleted variadic constructor next to a defaulted one.
struct N { N(...) = delete; N() = default; };
static_assert(__has_trivial_constructor(N));
static_assert(__is_pod(N));
static_assert(!__is_trivial(N));
static_assert(!__is_trivially_constructible(N));
static_assert(__is_trivially_copyable(N));

// A deleted variadic constructor alone: no trivial default constructor.
struct ND { ND(...) = delete; };
static_assert(!__has_trivial_constructor(ND));
static_assert(!__is_trivial(ND));
static_assert(!__is_pod(ND));

// `= default` next to a user-provided overload with another parameter list.
struct A { A(int = 0); A() = default; };
static_assert(__has_trivial_constructor(A));
static_assert(__is_pod(A));
static_assert(!__is_trivial(A));
static_assert(!__is_trivially_constructible(A));

// A deleted copy constructor next to a defaulted move constructor.
struct C { C(const C&) = delete; C(C&&) = default; C() = default; };
static_assert(__has_trivial_copy(C));
static_assert(__is_trivially_copyable(C));
static_assert(__is_trivial(C));
static_assert(__is_pod(C));
static_assert(!__is_trivially_constructible(C, const C&));
static_assert(__is_trivially_constructible(C, C&&));

// A defaulted copy constructor next to a user-provided `C2(C2&)`: both
// are eligible copy constructors (different parameter types).
struct C2 { C2(const C2&) = default; C2(C2&) {} };
static_assert(!__has_trivial_copy(C2));
static_assert(!__is_trivially_copyable(C2));
static_assert(__is_trivially_constructible(C2, const C2&));
static_assert(!__is_trivially_constructible(C2, C2&));

// A deleted move constructor next to a defaulted copy constructor.
struct D { D(D&&) = delete; D(const D&) = default; };
static_assert(__is_trivially_copyable(D));
static_assert(__has_trivial_move_constructor(D));

// A deleted destructor is trivial.
struct DD { ~DD() = delete; };
static_assert(__has_trivial_destructor(DD));
static_assert(__is_trivially_copyable(DD));
static_assert(__is_trivial(DD));

// Constrained default constructors in a class template instantiated both
// ways: only the satisfied one is eligible.
template <class T> concept Big = sizeof(T) > 4;
template <class T> struct S {
  T t;
  S() requires Big<T> = default;
  S() requires (!Big<T>) {}
};
static_assert(!__has_trivial_constructor(S<int>));
static_assert(!__is_trivial(S<int>));
static_assert(!__is_trivially_constructible(S<int>));
static_assert(__has_trivial_constructor(S<double>));
static_assert(__is_trivial(S<double>));
static_assert(__is_pod(S<double>));
static_assert(__is_trivially_constructible(S<double>));

// An unconstrained defaulted constructor is not eligible when a satisfied
// constrained one exists (it is more constrained).
template <class T> struct U {
  T t;
  U() = default;
  U() requires Big<T> {}
};
static_assert(__has_trivial_constructor(U<int>));
static_assert(__is_trivial(U<int>));
static_assert(!__has_trivial_constructor(U<double>));
static_assert(!__is_trivial(U<double>));

// Constrained copy constructors: the ineligible non-trivial one does not
// make the class non-trivially-copyable.
template <bool B> struct K {
  K(const K&) requires B = default;
  K(const K&) {}
  K() = default;
};
static_assert(__is_trivially_copyable(K<true>));
static_assert(__has_trivial_copy(K<true>));
static_assert(!__is_trivially_copyable(K<false>));
static_assert(!__has_trivial_copy(K<false>));
