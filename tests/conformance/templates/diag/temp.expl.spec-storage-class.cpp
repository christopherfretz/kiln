// [temp.expl.spec]/2, [dcl.stc]: an explicit specialization of a variable
// template shall not have a storage-class-specifier other than the ones
// on the primary.
//@ strict-diags
//@ diag: 7:13 warning "explicit specialization cannot have a storage class"
template <class T> int v = 0;
template <> static int v<int> = 1;
