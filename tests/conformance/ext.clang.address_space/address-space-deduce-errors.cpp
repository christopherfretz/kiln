// The negative twin of address-space-types.cpp (WP-1803): a parameter in
// an address space deduces only from an argument in that space, and
// overloads differing only in space do not take the other space's
// arguments.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:3 error "no matching function for call to 'f'"
//@ diag: 17:25 note "candidate template ignored: deduced type '__attribute__((address_space(1))) int *' of 1st parameter does not match adjusted type 'int *' of argument [with T = int]"
//@ diag: 20:3 error "no matching function for call to 'f'"
//@ diag: 17:25 note "candidate template ignored: deduced type '__attribute__((address_space(1))) int *' of 1st parameter does not match adjusted type '__attribute__((address_space(2))) int *' of argument [with T = int]"
//@ diag: 23:19 error "no matching function for call to 'g'"
//@ diag: 22:25 note "candidate template ignored: cannot deduce a type for 'T' that would make '__attribute__((address_space(1))) T' equal 'int'"
//@ diag: 26:19 error "no matching function for call to 'h'"
//@ diag: 24:6 note "candidate function not viable: cannot pass pointer to generic address space as a pointer to address space '1' in 1st argument"
//@ diag: 25:6 note "candidate function not viable: cannot pass pointer to generic address space as a pointer to address space '2' in 1st argument"

template <class T> void f(__attribute__((address_space(1))) T *);
void uf(int *p, __attribute__((address_space(2))) int *q) {
  f(p);
  f(q);
}
template <class T> void g(__attribute__((address_space(1))) T &);
void ug(int &r) { g(r); }
void h(__attribute__((address_space(1))) int *);
void h(__attribute__((address_space(2))) int *);
void uh(int *p) { h(p); }
