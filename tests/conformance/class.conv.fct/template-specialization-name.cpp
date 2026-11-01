// [class.conv.fct], [temp.mem]/5: a conversion function template
// specialization is named by its substituted target type
// (`operator int<int>`, `operator int *<int>`) in notes, instantiation
// notes and constant-evaluation call notes.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:5 error "conversion function from 'A' to 'int' invokes a deleted function"
//@ diag: 16:30 note "'operator int<int>' has been explicitly marked deleted here"
//@ diag: 19:22 error "conversion function from 'C' to 'int *' invokes a deleted function"
//@ diag: 18:30 note "'operator int *<int>' has been explicitly marked deleted here"
//@ diag: 20:72 warning "division by zero is undefined"
//@ diag: 21:19 note "in instantiation of function template specialization 'G::operator int<int>' requested here"
//@ diag: 21:15 error "constexpr variable 'z' must be initialized by a constant expression"
//@ diag: 20:72 note "division by zero"
//@ diag: 21:19 note "in call to 'G().operator int<int>()'"
struct A { template<class T> operator T() const = delete; };
int x = A();
struct C { template<class T> operator T*() const = delete; };
void h() { C c; int *p = c; (void)p; }
struct G { template<class T> constexpr operator T() const { return T(1)/T(0); } };
constexpr int z = G();
