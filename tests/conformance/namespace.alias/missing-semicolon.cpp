// [namespace.alias]/1: a namespace-alias-definition ends with ';'
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:16 error "expected ';' after namespace name"
namespace N {}
namespace Q = N int w;
