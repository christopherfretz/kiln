// Shared by class_decl_two_units.cpp and Inputs/class_decl_defs.cpp.
namespace gadgets {
struct Widget {
  int a;
  int b;
};
} // namespace gadgets

gadgets::Widget *make_widget();
int widget_sum(gadgets::Widget *w);
