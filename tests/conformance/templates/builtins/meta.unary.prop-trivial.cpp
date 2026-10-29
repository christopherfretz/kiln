// [meta.unary.prop]: destructibility and the older __has_trivial_* family,
// positive and negative (spec/sema-templates.md §9.1).
//@ strict-diags
//@ diag: 33:15 warning "builtin __has_trivial_destructor is deprecated; use __is_trivially_destructible instead"
//@ diag: 33:53 warning "builtin __has_trivial_destructor is deprecated; use __is_trivially_destructible instead"
//@ diag: 34:15 warning "builtin __has_trivial_copy is deprecated; use __is_trivially_copyable instead"
//@ diag: 34:47 warning "builtin __has_trivial_copy is deprecated; use __is_trivially_copyable instead"
//@ diag: 35:15 warning "builtin __has_trivial_assign is deprecated; use __is_trivially_assignable instead"
//@ diag: 35:49 warning "builtin __has_trivial_assign is deprecated; use __is_trivially_assignable instead"
//@ diag: 36:15 warning "builtin __has_trivial_constructor is deprecated; use __is_trivially_constructible instead"
//@ diag: 36:54 warning "builtin __has_trivial_constructor is deprecated; use __is_trivially_constructible instead"
//@ diag: 37:15 warning "builtin __has_trivial_move_constructor is deprecated; use __is_trivially_constructible instead"
//@ diag: 37:59 warning "builtin __has_trivial_move_constructor is deprecated; use __is_trivially_constructible instead"
//@ diag: 38:15 warning "builtin __has_trivial_move_assign is deprecated; use __is_trivially_assignable instead"
//@ diag: 38:54 warning "builtin __has_trivial_move_assign is deprecated; use __is_trivially_assignable instead"
struct Trivial { int x; };
struct UserDtor { ~UserDtor(); };
struct DeletedDtor { ~DeletedDtor() = delete; };
class PrivateDtor { ~PrivateDtor(); };
struct HasUserDtorMember { UserDtor m[2]; };
struct UserCopy { UserCopy(const UserCopy &); };
struct UserAssign { UserAssign &operator=(const UserAssign &); };
struct UserDefault { UserDefault(); };
struct UserMove { UserMove(UserMove &&); };
struct UserMoveAssign { UserMoveAssign &operator=(UserMoveAssign &&); };

static_assert(__is_destructible(int) && __is_destructible(Trivial) && __is_destructible(UserDtor) && __is_destructible(int &));
static_assert(!__is_destructible(DeletedDtor) && !__is_destructible(void) && !__is_destructible(int[]) && !__is_destructible(void()));
static_assert(__is_nothrow_destructible(int) && __is_nothrow_destructible(UserDtor) && __is_nothrow_destructible(HasUserDtorMember));
static_assert(!__is_nothrow_destructible(PrivateDtor) && !__is_nothrow_destructible(DeletedDtor) && !__is_destructible(PrivateDtor));
static_assert(__is_trivially_destructible(int) && __is_trivially_destructible(Trivial) && __is_trivially_destructible(int[2]));
static_assert(!__is_trivially_destructible(UserDtor) && !__is_trivially_destructible(DeletedDtor) && !__is_trivially_destructible(void));
static_assert(__has_trivial_destructor(Trivial) && !__has_trivial_destructor(UserDtor));
static_assert(__has_trivial_copy(Trivial) && !__has_trivial_copy(UserCopy));
static_assert(__has_trivial_assign(Trivial) && !__has_trivial_assign(UserAssign));
static_assert(__has_trivial_constructor(Trivial) && !__has_trivial_constructor(UserDefault));
static_assert(__has_trivial_move_constructor(Trivial) && !__has_trivial_move_constructor(UserMove));
static_assert(__has_trivial_move_assign(Trivial) && !__has_trivial_move_assign(UserMoveAssign));
