// [cpp.embed]: a diagnostic in an `#embed`'s bytes is reported at the
// directive's `#` (also an indented `  #  embed`), with no "expanded
// from" note, as clang 19 and 22; a range around the bytes keeps its
// written ends (WP-2544).
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-c23-extensions
//@ diag: 14:1 error "cannot initialize a member subobject of type 'int *' with an rvalue of type"
//@ diag: 18:3 error "cannot initialize a member subobject of type 'int *' with an rvalue of type"
//@ diag: 20:6 error "cannot initialize a variable of type 'int *' with an rvalue of type 'int'"
//@ diag: 27:4 error "excess elements in array initializer"
struct T { int *q; };
T t = {
#embed "Inputs/p1967-data.txt" limit(1)
};
struct U { int *a; int b; };
U u = {
  #  embed "Inputs/p1967-data.txt" limit(1)
};
int *p = (
#embed "Inputs/p1967-data.txt" limit(1)
);
int ok[] = {
#embed "Inputs/p1967-data.txt"
};
int s[1] = {
   #embed "Inputs/p1967-data.txt" limit(2)
};
