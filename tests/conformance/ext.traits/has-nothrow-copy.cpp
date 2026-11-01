// __has_nothrow_copy(T) (clang 19 UTT_HasNothrowCopy, WP-166): POD,
// references, a trivial copy constructor, or every copy constructor
// (callable with one argument) nothrow.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-deprecated-builtins
//@ diag: 45:13 error "incomplete type 'Inc' used in type trait expression"
//@ diag: 27:8 note "forward declaration of 'Inc'"
//@ diag: 46:13 error "type trait requires 1 argument; have 2 arguments"
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
static_assert(__has_nothrow_copy(int));
static_assert(__has_nothrow_copy(int&));
static_assert(__has_nothrow_copy(int&&));
static_assert(__has_nothrow_copy(const int));
static_assert(__has_nothrow_copy(Triv));
static_assert(__has_nothrow_copy(NT));
static_assert(!__has_nothrow_copy(NT[2]));
static_assert(!__has_nothrow_copy(T));
static_assert(__has_nothrow_copy(Tmpl));
static_assert(!__has_nothrow_copy(DefArg));
static_assert(__has_nothrow_copy(Del));
static_assert(__has_nothrow_copy(Mem));
static_assert(!__has_nothrow_copy(MemT));
static_assert(__has_nothrow_copy(Virt));
static_assert(__has_nothrow_copy(MoveOnly));
static_assert(!__has_nothrow_copy(void));
static_assert(__has_nothrow_copy(U));
bool bad1 = __has_nothrow_copy(Inc);
bool bad2 = __has_nothrow_copy(int, int);
