// [driver] A call through a pointer to a function type whose parameter
// is `noescape` marks the argument `nocapture`, as clang 19's
// ConstructAttributeList does from the FunctionProtoType's
// ExtParameterInfo; kiln reads noescape only from the callee's
// declaration. WP-813.
//@ requires: clang<22
//@ expect: ir
//@ xfail(kiln): WP-813 follow-up, noescape on a function type's parameter is not kept in the type
void (*fp)(__attribute__((noescape)) int *);
void use(int *p) { fp(p); }
