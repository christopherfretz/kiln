// [basic.indet]: what clang's -Wuninitialized analysis leaves alone: a variable
// whose address is taken, bound to a non-const reference or passed by
// non-const reference, captured by reference (a copy capture is a use), used
// only in unevaluated operands or a (void) cast, initialized on the path a
// goto skips, an asm output, arrays, members of a record and statics. The
// analysis follows asm goto edges. __attribute__((uninitialized)) and
// [[indeterminate]] (P2795) do not silence clang 19's warning; clang 19 does
// not know [[indeterminate]].
//@ expect: compile-ok
//@ args: -Wuninitialized -Wconditional-uninitialized -Wno-unknown-attributes
//@ strict-diags
//@ diag: 54:13 warning "variable 'c' is uninitialized when used here"
//@ diag: 53:8 note "initialize the variable 'c' to silence this warning"
//@ diag: 71:7 warning "variable 'c' is used uninitialized whenever 'if' condition is true"
//@ diag: 75:10 note "uninitialized use occurs here"
//@ diag: 71:3 note "remove the 'if' if its condition is always false"
//@ diag: 70:8 note "initialize the variable 'c' to silence this warning"
//@ diag: 89:10 warning "variable 'c' may be uninitialized when used here"
//@ diag: 85:8 note "initialize the variable 'c' to silence this warning"
//@ diag: 94:10 warning "variable 'c' is uninitialized when used here"
//@ diag: 93:8 note "initialize the variable 'c' to silence this warning"
//@ diag: 99:10 warning "variable 'c' is uninitialized when used here"
//@ diag: 98:8 note "initialize the variable 'c' to silence this warning"
int g(int);
void take(int *);
void take_ref(int &);

int address_taken() {
  int c;
  take(&c);
  return c;
}

int reference_bound() {
  int c;
  int &r = c;
  return r;
}

int non_const_ref_arg() {
  int c;
  take_ref(c);
  return c;
}

int captured_by_ref() {
  int c;
  auto l = [&] { return c; };
  return l();
}

int captured_by_copy() {
  int c;
  auto l = [=] { return c; };
  return l();
}

int unevaluated() {
  int c;
  return sizeof(c) + sizeof(decltype(c));
}

int void_cast() {
  int c;
  (void)c;
  return 0;
}

int via_goto(bool b) {
  int c;
  if (b)
    goto done;
  c = 1;
done:
  return c;
}

int asm_output() {
  int c;
  asm("" : "=r"(c));
  return c;
}

int asm_goto(bool b) {
  int c;
  asm goto("" :::: skip);
  c = 1;
skip:
  return c;
}

int attr_uninitialized() {
  int c __attribute__((uninitialized));
  return c;
}

int attr_indeterminate() {
  int c [[indeterminate]];
  return c;
}

int array_element() {
  int a[3];
  return a[0];
}

struct P {
  int x;
};
int record_member() {
  P p;
  return p.x;
}

int static_local() {
  static int s;
  return s;
}

int initialized() {
  int c = 0;
  return c;
}

[[noreturn]] void die();
int noreturn_path(bool b) {
  int c;
  if (b)
    c = 1;
  else
    die();
  return c;
}
