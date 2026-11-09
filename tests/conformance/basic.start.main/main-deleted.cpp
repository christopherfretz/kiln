// [basic.start.main]/3: a program that defines `main` as deleted is
// ill-formed (clang's err_deleted_main, at `delete`; WP-1015).
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:14 error "'main' is not allowed to be deleted"
int main() = delete;
