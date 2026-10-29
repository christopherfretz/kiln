// [class.init]/1, [dcl.init.general]/16.6.2, [dcl.init.list]/3.7, [over.match.list]/1:
// an explicit constructor chosen for copy-list-initialization is ill-formed;
// a deleted or inaccessible constructor chosen for direct-, copy- (as a
// converting constructor, [over.match.copy]) or list-initialization, of a
// variable, a return value or a temporary, is diagnosed at the entity.
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:3 error "chosen constructor is explicit in copy-initialization"
//@ diag: 29:21 note "explicit constructor declared here"
//@ diag: 32:3 error "call to deleted constructor of 'D'"
//@ diag: 31:12 note "'D' has been explicitly marked deleted here"
//@ diag: 33:3 error "calling a private constructor of class 'D'"
//@ diag: 31:62 note "declared private here"
//@ diag: 34:3 error "conversion function from 'int' to 'D' invokes a deleted function"
//@ diag: 31:12 note "'D' has been explicitly marked deleted here"
//@ diag: 35:3 error "calling a private constructor of class 'D'"
//@ diag: 31:53 note "declared private here"
//@ diag: 36:17 error "conversion function from 'int' to 'D' invokes a deleted function"
//@ diag: 31:12 note "'D' has been explicitly marked deleted here"
//@ diag: 37:17 error "calling a private constructor of class 'D'"
//@ diag: 31:53 note "declared private here"
//@ diag: 38:12 error "calling a private constructor of class 'D'"
//@ diag: 31:53 note "declared private here"
//@ diag: 38:22 error "call to deleted constructor of 'D'"
//@ diag: 31:12 note "'D' has been explicitly marked deleted here"
//@ diag: 38:30 error "call to deleted constructor of 'D'"
//@ diag: 31:12 note "'D' has been explicitly marked deleted here"
struct E { explicit E(int, int); };
E g = {1, 2};
struct D { D(int) = delete; D(long, long); private: D(char); D(char, char); };
D d1(1);
D d2('a', 'b');
D d3 = 1;
D d4 = 'c';
D r1() { return 1; }
D r2() { return 'c'; }
void h() { D{'c'}; D x{1}; D y = {1}; }
