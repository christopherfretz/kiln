// WP-3688: replay counterpart of `labels-module-image.cpp` (same line numbers
// so both share `labels-module.refs`).
//@ module: labels_mod=Inputs/labels_mod.cppm
//@ two-step
//@ written-refs: labels-module.refs
//@ args: -fkiln-written-refs -fno-kiln-bmi-image -w
//@ std: c++20
import labels_mod;
void run() {
  jump_fn();
  jump_tmpl(1);
}
