// [dcl.init.list]/3.10: a reference list-initializes a temporary of the referenced
// type. An excess element in an aggregate temporary takes clang's 'in
// initialization of temporary ... created to list-initialize this reference'
// note (a scalar temporary does not), and a deleted constructor is named with
// the reference type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:22 error "excess elements in struct initializer"
//@ diag: 30:10 note "in initialization of temporary of type 'const H' created to list-initialize this reference"
//@ diag: 31:29 error "excess elements in array initializer"
//@ diag: 31:13 note "in initialization of temporary of type 'const int[2]' created to list-initialize this reference"
//@ diag: 32:19 error "excess elements in union initializer"
//@ diag: 32:10 note "in initialization of temporary of type 'const U' created to list-initialize this reference"
//@ diag: 33:19 error "excess elements in scalar initializer"
//@ diag: 33:10 note "in initialization of temporary of type 'const H' created to list-initialize this reference"
//@ diag: 34:26 error "excess elements in struct initializer"
//@ diag: 34:10 note "in initialization of temporary of type 'const K' created to list-initialize this reference"
//@ diag: 35:23 error "excess elements in struct initializer"
//@ diag: 35:11 note "in initialization of temporary of type 'const H' created to list-initialize this reference"
//@ diag: 36:12 error "excess elements in scalar initializer"
//@ diag: 38:10 error "call to deleted constructor of 'const J &'"
//@ diag: 37:12 note "'J' has been explicitly marked deleted here"
//@ diag: 40:10 error "call to deleted constructor of 'const G &'"
//@ diag: 39:12 note "'G' has been explicitly marked deleted here"
//@ diag: 42:11 error "call to deleted constructor of 'const Q &&'"
//@ diag: 41:20 note "'Q' has been explicitly marked deleted here"
struct H { int a; int b; };
union U { int a; float b; };
struct K { int a; H h; };
const H& h1 = {1, 2, 3};
const int (&a1)[2] = {1, 2, 3};
const U& u1 = {1, 2};
const H& h2 = {1, {2, 3}};
const K& k1 = {1, {2, 3, 4}};
const H&& h3 = {1, 2, 3};
const int& i1 = {1, 2};
struct J { J(int) = delete; };
const J& j1 = {1};
struct G { G() = delete; };
const G& g1 = {};
struct Q { Q(int); Q(double) = delete; };
const Q&& q1 = {1.0};
