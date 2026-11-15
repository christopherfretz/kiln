// Debug info (WP-1414): the specializations of a generic lambda's
// conversion function and static invoker ([expr.prim.lambda.closure]/9)
// are implicit like their patterns, as clang's instantiation copies
// `isImplicit`: `DIFlagArtificial`, a declaration without a file or line,
// and an invoker parameter without one (clang's `IsArtificial`). clang 19
// and 22 agree.
//@ args: -g
int use() {
  auto l = [](auto x) { return x + 1; };
  int (*fp)(int) = l;
  long (*gp)(long) = l;
  return fp(2) + (int)gp(3);
}
