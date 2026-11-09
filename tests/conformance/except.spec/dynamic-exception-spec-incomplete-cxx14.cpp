// C++14 [except.spec]/2: a type in a dynamic exception specification shall
// not be an incomplete type, a pointer or reference to an incomplete type
// other than cv void*, or an rvalue reference; arrays and functions are
// adjusted first.  Dependent types are checked at instantiation.  WP-1052.
//@ std: c++14
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 26:21 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 25:8 note "forward declaration of 'Inc'"
//@ diag: 27:23 error "pointer to incomplete type 'const Inc' is not allowed in exception specification"
//@ diag: 25:8 note "forward declaration of 'Inc'"
//@ diag: 28:17 error "incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 25:8 note "forward declaration of 'Inc'"
//@ diag: 29:54 error "pointer to incomplete type 'const Inc' is not allowed in exception specification"
//@ diag: 31:12 note "in instantiation of exception specification for 'd<Inc>' requested here"
//@ diag: 25:8 note "forward declaration of 'Inc'"
//@ diag: 30:58 error "pointer to incomplete type 'const Inc' is not allowed in exception specification"
//@ diag: 31:22 note "in instantiation of exception specification for 'e<Inc>' requested here"
//@ diag: 25:8 note "forward declaration of 'Inc'"
//@ diag: 32:16 error "rvalue reference type 'int &&' is not allowed in exception specification"
//@ diag: 32:23 error "incomplete type 'void' is not allowed in exception specification"
//@ diag: 32:64 error "reference to incomplete type 'Inc' is not allowed in exception specification"
//@ diag: 25:8 note "forward declaration of 'Inc'"
struct Inc;
void a() throw(int, Inc);
void b() throw(int,   const Inc  *);
void c() throw( Inc  );
template<class T> void d() throw(int, const   T  *, T);
template<class... T> void e() throw(int, const   T  * ...);
void u() { d<Inc>(); e<Inc>(); }
void g() throw(int&&, void, void*, const void*, int[2], int(), Inc&);
