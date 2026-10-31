// [except.handle]/1: the exception-declaration shall not denote an
// incomplete type, an abstract class type, an rvalue reference type, or a
// pointer or reference to an incomplete type other than cv void*.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:19 error "cannot catch incomplete type 'In'"
//@ diag: 15:8 note "forward declaration of 'In'"
//@ diag: 19:21 error "cannot catch pointer to incomplete type 'In'"
//@ diag: 15:8 note "forward declaration of 'In'"
//@ diag: 20:21 error "cannot catch reference to incomplete type 'In'"
//@ diag: 15:8 note "forward declaration of 'In'"
//@ diag: 21:23 error "cannot catch exceptions by rvalue reference"
//@ diag: 22:20 error "variable type 'Abs' is an abstract class"
//@ diag: 16:27 note "unimplemented pure virtual method 'f' in 'Abs'"
struct In;
struct Abs { virtual void f() = 0; };
void f() {
  try {} catch (In) {}
  try {} catch (In *) {}
  try {} catch (In &) {}
  try {} catch (int &&) {}
  try {} catch (Abs) {}
  try {} catch (void *) {}
  try {} catch (const void *) {}
  try {} catch (int[3]) {}
  try {} catch (void (*)()) {}
}
