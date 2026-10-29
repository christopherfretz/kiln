// [except.spec]/2: the constant-expression of a noexcept-specifier shall be a
// contextually converted constant expression of type bool
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:19 error "value of type 'const S' is not implicitly convertible to 'bool'"
//@ diag: 19:20 error "value of type 'E' is not implicitly convertible to 'bool'"
//@ diag: 20:19 error "conversion from 'std::nullptr_t' to 'bool' is not allowed in a converted constant expression"
//@ diag: 21:20 error "conversion from 'int *const' to 'bool' is not allowed in a converted constant expression"
//@ diag: 22:20 error "conversion from 'double' to 'bool' is not allowed in a converted constant expression"
//@ diag: 23:19 error "noexcept specifier argument evaluates to 2, which cannot be narrowed to type 'bool'"
//@ diag: 25:19 error "noexcept specifier argument is not a constant expression"
//@ diag: 25:19 note "read of non-const variable 'x' is not allowed in a constant expression"
//@ diag: 24:5 note "declared here"
struct S {};
constexpr S s{};
enum class E { a };
constexpr int *p = nullptr;
void a() noexcept(s);
void b0() noexcept(E::a);
void b() noexcept(nullptr);
void b1() noexcept(p);
void b2() noexcept(1.0);
void c() noexcept(2);
int x = 1;
void d() noexcept(x);
