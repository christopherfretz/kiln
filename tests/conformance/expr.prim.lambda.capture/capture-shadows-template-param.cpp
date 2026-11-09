// [expr.prim.lambda.capture]/5: an identifier in a simple-capture or an
// init-capture's name shall not appear as a template parameter of the
// lambda's template-parameter-list: "declaration of 'y' shadows template
// parameter" at the capture, the parameter noted (clang test
// expr.prim.lambda.capture/p5.cpp:7; WP-1091).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:13 error "declaration of 'y' shadows template parameter"
//@ diag: 19:29 note "template parameter is declared here"
//@ diag: 20:14 error "declaration of 'x' shadows template parameter"
//@ diag: 20:26 note "template parameter is declared here"
//@ diag: 21:15 error "declaration of 'r' shadows template parameter"
//@ diag: 21:31 note "template parameter is declared here"
//@ diag: 22:14 error "declaration of 'x' shadows template parameter"
//@ diag: 22:21 note "template parameter is declared here"
//@ diag: 23:17 error "declaration of 'y' shadows template parameter"
//@ diag: 23:32 note "template parameter is declared here"
void f(int x, int y) {
  auto h = [y = 0]<typename y>(y) { return 0; };
  auto h2 = [x]<typename x>(x) { return 0; };
  auto h3 = [&r = x]<typename r>() { return 0; };
  auto h4 = [x]<int x>() { return 0; };
  auto h5 = [x, y]<typename... y>() {};
}
