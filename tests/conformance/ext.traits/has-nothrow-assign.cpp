// __has_nothrow_assign(T) (GCC extension; clang 19 EvaluateUnaryTypeTrait
// UTT_HasNothrowAssign, WP-166): POD or a class whose copy assignment
// operators are all nothrow; never for const or reference operands.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-deprecated-builtins
//@ diag: 51:13 error "incomplete type 'Inc' used in type trait expression"
//@ diag: 27:8 note "forward declaration of 'Inc'"
//@ diag: 52:13 error "type trait requires 1 argument; have 2 arguments"
struct Triv { int x; };
struct NT { NT& operator=(const NT&) noexcept; NT(const NT&) noexcept; NT() noexcept; NT& operator=(NT&&) noexcept; };
struct T { T& operator=(const T&); T(const T&); T(); T& operator=(T&&); };
struct Mixed { Mixed& operator=(const Mixed&) noexcept; Mixed& operator=(volatile Mixed&); };
struct Tmpl { template<class U> Tmpl& operator=(const U&); Tmpl(const Tmpl&) = default; template<class U> Tmpl(U); };
struct DefArg { DefArg(const DefArg&, int = 0) noexcept; DefArg(int = 0) noexcept; };
struct Del { Del& operator=(const Del&) = delete; Del(const Del&) = delete; Del() = delete; };
struct Mem { NT n; };
struct MemT { T t; };
struct Virt { virtual void f(); };
struct NoCopyAssignDecl { NoCopyAssignDecl& operator=(int) noexcept; };
struct Base { Base& operator=(const Base&) noexcept; };
struct Der : Base { using Base::operator=; };
struct MoveOnly { MoveOnly(MoveOnly&&) noexcept; MoveOnly& operator=(MoveOnly&&) noexcept; };
union U { int a; float b; };
union UNT { NT n; UNT& operator=(const UNT&) noexcept; };
enum E { e };
struct Inc;
static_assert(__has_nothrow_assign(int));
static_assert(!__has_nothrow_assign(const int));
static_assert(!__has_nothrow_assign(int&));
static_assert(__has_nothrow_assign(int[2]));
static_assert(!__has_nothrow_assign(const int[2]));
static_assert(__has_nothrow_assign(Triv));
static_assert(!__has_nothrow_assign(const Triv));
static_assert(__has_nothrow_assign(NT));
static_assert(!__has_nothrow_assign(T));
static_assert(!__has_nothrow_assign(Mixed));
static_assert(__has_nothrow_assign(Tmpl));
static_assert(__has_nothrow_assign(Del));
static_assert(__has_nothrow_assign(Mem));
static_assert(!__has_nothrow_assign(MemT));
static_assert(__has_nothrow_assign(Virt));
static_assert(__has_nothrow_assign(NoCopyAssignDecl));
static_assert(__has_nothrow_assign(MoveOnly));
static_assert(__has_nothrow_assign(U));
static_assert(__has_nothrow_assign(UNT));
static_assert(__has_nothrow_assign(E));
static_assert(!__has_nothrow_assign(void));
static_assert(!__has_nothrow_assign(NT[2]));
static_assert(!__has_nothrow_assign(int()));
bool bad1 = __has_nothrow_assign(Inc);
bool bad2 = __has_nothrow_assign(int, int);
