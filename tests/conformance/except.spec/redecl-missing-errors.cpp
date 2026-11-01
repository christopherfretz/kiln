// [except.spec]/5: if any declaration of a function has a non-throwing
// exception specification, all declarations shall have a compatible one.
// Outside clang's C-library allowance a missing specification is an error,
// spelled as the previous declaration wrote it; the redeclaration still
// inherits it (WP-268).
//@ expect: compile-fail
//@ strict-diags
//@ args: -isystem %{test_dir}/Inputs
//@ diag: 23:8 error "'sys_cxx' is missing exception specification 'noexcept'"
//@ diag: 25:19 error "'user_c' is missing exception specification 'noexcept'"
//@ diag: 24:19 note "previous declaration is here"
//@ diag: 27:6 error "'t' is missing exception specification 'noexcept(true)'"
//@ diag: 26:6 note "previous declaration is here"
//@ diag: 29:6 error "'u' is missing exception specification 'throw()'"
//@ diag: 28:6 note "previous declaration is here"
//@ diag: 32:19 error "'sys_throw' is missing exception specification 'throw()'"
//@ diag: 31:19 note "previous declaration is here"
//@ diag: 35:7 error "'operator new' is missing exception specification 'noexcept'"
//@ diag: 34:7 note "previous declaration is here"
//@ diag: 38:24 error "'v' is missing exception specification 'noexcept(sizeof(T) > 1)'"
//@ diag: 37:24 note "previous declaration is here"
#include <redecl_sys.h>
double sys_cxx(double);
extern "C" double user_c(double) noexcept;
extern "C" double user_c(double);
void t() noexcept(true);
void t();
void u() throw();
void u();
// The second redeclaration follows the first, which is not in a system header.
extern "C" double sys_throw(double);
extern "C" double sys_throw(double);
// Not a replaceable global allocation function.
void* operator new(std::size_t, int) noexcept;
void* operator new(std::size_t, int);
static_assert(noexcept(t()));
template<class T> void v() noexcept(sizeof(T) > 1);
template<class T> void v();
