// WP-3688: label written-reference rows (`RefRole::Label`) round-trip through
// a module declaration image, matching replay (`labels-module-replay.cpp`).
//@ module: labels_mod=Inputs/labels_mod.cppm
//@ two-step
//@ written-refs: labels-module.refs
//@ args: -fkiln-written-refs -fkiln-bmi-image -w
//@ std: c++20
import labels_mod;
void run() {
  jump_fn();
  jump_tmpl(1);
}
