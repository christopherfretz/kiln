// [temp.explicit]/3, [dcl.array]/3: the explicit instantiation definition
// of a variable template specialization of unknown bound is compared with
// the type its instantiated initializer completes, so neither `T[]` nor
// a different bound matches (clang `err_invalid_var_template_spec_type`).
// WP-907.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:10 error "type 'char[]' of explicit instantiation of 'arr' does not match expected type 'char[2]'"
//@ diag: 12:22 note "variable template 'arr' declared here"
//@ diag: 14:10 error "type 'int[3]' of explicit instantiation of 'arr' does not match expected type 'int[2]'"
//@ diag: 12:22 note "variable template 'arr' declared here"
template <class T> T arr[] = {1, 2};
template char arr<char>[];
template int arr<int>[3];
