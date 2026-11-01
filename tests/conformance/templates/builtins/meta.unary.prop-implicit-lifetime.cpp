// [meta.unary.prop] (C++23 P2674) is_implicit_lifetime:
// __builtin_is_implicit_lifetime(T) is true for scalar and array types
// and implicit-lifetime classes ([class.prop]/9: an aggregate whose
// destructor is not user-provided, or a class with a trivial non-deleted
// destructor and at least one trivial non-deleted eligible constructor)
// (spec/sema-templates.md §9.1; WP-263). Oracle-checked by hand with
// clang++-22 (22.1.8): same answers and the same diagnostics.
//@ expect: compile-fail
//@ skip(clang): clang 19 lacks __builtin_is_implicit_lifetime (added in clang 20)
//@ strict-diags
//@ diag: 54:12 error "incomplete type 'Inc' used in type trait expression"
//@ diag: 15:8 note "forward declaration of 'Inc'"
struct Empty {};
struct Agg { int x; int &r; };
struct Inc;
struct AggDtor { int x; ~AggDtor(); };
struct AggDefDtor { int x; ~AggDefDtor() = default; };
struct UserCtor { UserCtor(int); };
struct UserCtorCopy { UserCtorCopy(int); UserCtorCopy(const UserCtorCopy &) = default; };
struct UserAll {
  UserAll(int);
  UserAll(const UserAll &);
  UserAll(UserAll &&);
};
struct UserAllDefault { UserAllDefault(int); UserAllDefault() = default; UserAllDefault(const UserAllDefault &); UserAllDefault(UserAllDefault &&); };
struct DeletedDtor { ~DeletedDtor() = delete; };
struct DelDtorCtor { DelDtorCtor(int); ~DelDtorCtor() = delete; };
struct Virt { virtual void f(); };
struct VirtBase : virtual Empty {};
struct NonTrivMember { UserAll u; };
struct Priv { Priv(int); private: int x; };
union Un { int i; float f; };
union UnUser { int i; UnUser(int); UnUser(const UnUser &); ~UnUser(); };
enum En { e0 };

static_assert(__builtin_is_implicit_lifetime(int));
static_assert(__builtin_is_implicit_lifetime(const int *));
static_assert(__builtin_is_implicit_lifetime(En));
static_assert(__builtin_is_implicit_lifetime(int Empty::*));
static_assert(__builtin_is_implicit_lifetime(UserAll[2]));
static_assert(__builtin_is_implicit_lifetime(Inc[]));
static_assert(!__builtin_is_implicit_lifetime(int &));
static_assert(!__builtin_is_implicit_lifetime(void));
static_assert(!__builtin_is_implicit_lifetime(int()));
static_assert(__builtin_is_implicit_lifetime(Empty) && __builtin_is_implicit_lifetime(Agg));
static_assert(!__builtin_is_implicit_lifetime(AggDtor) && __builtin_is_implicit_lifetime(AggDefDtor));
static_assert(__builtin_is_implicit_lifetime(UserCtor) && __builtin_is_implicit_lifetime(UserCtorCopy));
static_assert(!__builtin_is_implicit_lifetime(UserAll) && __builtin_is_implicit_lifetime(UserAllDefault));
static_assert(__builtin_is_implicit_lifetime(DeletedDtor) && !__builtin_is_implicit_lifetime(DelDtorCtor));
static_assert(!__builtin_is_implicit_lifetime(Virt));
static_assert(!__builtin_is_implicit_lifetime(VirtBase) && __builtin_is_implicit_lifetime(NonTrivMember));
static_assert(__builtin_is_implicit_lifetime(Priv) && __builtin_is_implicit_lifetime(Un));
static_assert(!__builtin_is_implicit_lifetime(UnUser));
bool bad = __builtin_is_implicit_lifetime(Inc);
