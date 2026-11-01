// [dcl.init.list]/3.7, [over.match.list] (WP-241): the constructor call of
// a variable's copy-list-initialization (`S s = {...}`, also binding a
// reference) is located at the braced list, as clang's
// PerformConstructorInitialization does, so a private constructor and an
// initializer-list constructor's call frame are reported at the `{`;
// overload, explicit and deleted errors stay at the variable.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:3 error "no matching constructor for initialization of 'S'"
//@ diag: 38:51 note "candidate constructor not viable: requires 3 arguments, but 4 were provided"
//@ diag: 38:29 note "candidate constructor not viable: requires 2 arguments, but 4 were provided"
//@ diag: 38:12 note "candidate constructor not viable: requires 1 argument, but 4 were provided"
//@ diag: 38:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 4 were provided"
//@ diag: 38:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 4 were provided"
//@ diag: 40:3 error "chosen constructor is explicit in copy-initialization"
//@ diag: 38:29 note "explicit constructor declared here"
//@ diag: 41:7 error "calling a private constructor of class 'S'"
//@ diag: 38:51 note "declared private here"
//@ diag: 42:14 error "calling a private constructor of class 'S'"
//@ diag: 38:51 note "declared private here"
//@ diag: 43:16 error "calling a private constructor of class 'S'"
//@ diag: 38:51 note "declared private here"
//@ diag: 44:26 error "calling a private constructor of class 'S'"
//@ diag: 38:51 note "declared private here"
//@ diag: 46:3 error "call to deleted constructor of 'D'"
//@ diag: 45:12 note "'D' has been explicitly marked deleted here"
//@ diag: 49:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:59 note "division by zero"
//@ diag: 48:28 note "in call to 'L({&{1, 2}[0], 2})'"
//@ diag: 49:15 note "in call to 'li()'"
//@ diag: 50:12 error "calling a private constructor of class 'S'"
//@ diag: 38:51 note "declared private here"
//@ diag: 51:12 error "calling a private constructor of class 'S'"
//@ diag: 38:51 note "declared private here"
//@ diag: 52:3 error "calling a private constructor of class 'S'"
//@ diag: 38:51 note "declared private here"
namespace std { template<class E> class initializer_list { const E* b; decltype(sizeof 0) n; public: constexpr initializer_list() : b(nullptr), n(0) {} constexpr decltype(sizeof 0) size() const { return n; } }; }
struct S { S(int); explicit S(int, int); private: S(int, int, int); };
S a = {1, 2, 3, 4};
S b = {1, 2};
S c = {1, 2, 3};
const S& r = {1, 2, 3};
S g() { return {1, 2, 3}; }
void h(S); void hh() { h({1, 2, 3}); }
struct D { D(int) = delete; };
D d = {1};
struct L { constexpr L(std::initializer_list<int> l) : v(1/(int)(l.size()-2)) {} int v; };
constexpr int li() { L l = {1, 2}; return l.v; }
static_assert(li());
S arr[] = {{1, 2, 3}};
S* p = new S{1, 2, 3};
S q{1, 2, 3};
