// [stmt.label]/1: a label is declared once per function, also far down
// a long run of labels; the error names the first definition, and a
// `goto` to a label never declared is an error ([stmt.goto]/1; the label
// searches are hashed since WP-2478).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:1 error "redefinition of label 'l1'"
//@ diag: 11:6 note "previous definition is here"
//@ diag: 18:8 error "use of undeclared label 'l64'"
void f() {
l0:; l1:; l2:; l3:; l4:; l5:; l6:; l7:; l8:; l9:; l10:; l11:; l12:; l13:; l14:; l15:;
l16:; l17:; l18:; l19:; l20:; l21:; l22:; l23:; l24:; l25:; l26:; l27:; l28:; l29:; l30:; l31:;
l32:; l33:; l34:; l35:; l36:; l37:; l38:; l39:; l40:; l41:; l42:; l43:; l44:; l45:; l46:; l47:;
l48:; l49:; l50:; l51:; l52:; l53:; l54:; l55:; l56:; l57:; l58:; l59:; l60:; l61:; l62:; l63:;
  goto l63;
l1:;
  goto l0;
  goto l64;
}
