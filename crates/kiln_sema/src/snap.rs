//! The snapshot encoding of [`Sema`] (spec/sema.md "Snapshots",
//! spec/snap.md "Hand-written roots").
//!
//! `Sema` is a configured root: it borrows the diagnostics engine and
//! holds the consumer's command-line switches, so it is not `Snap`.
//! [`Sema::snap_put`] writes the state of a quiescent `Sema` (the snapshot
//! point: the end of a prefix, at top level, before the end-of-TU work);
//! [`Sema::snap_restore`] applies it to a fresh `Sema` the consumer built
//! and configured from its own options. Both name every field through
//! [`sema_fields!`] (an exhaustive destructure), so a new field is a
//! compile error here until it is put in one of the classes below.
//!
//! Every field is in exactly one class:
//!
//! - **state**: written, as data. The default when in doubt: end-of-TU
//!   work lists, "already warned" tables, re-warn tables of template
//!   patterns, pragma stacks, memos whose hits suppress a repeated
//!   diagnostic, counters that number things, and every per-construct
//!   value that is not reset when its construct ends (it lingers into the
//!   next one, so the consumer must see the producer's).
//! - **configuration**: not written; set by the consumer from its command
//!   line (`kiln_parse`'s `configure`, [`Sema::for_target`]), covered by
//!   the snapshot's configuration fingerprint (WP-1383).
//! - **transient**: not written; must be at rest (as a fresh `Sema` has
//!   it) at the snapshot point, which [`Sema::snapshot_ready`] checks.
//!   Per-construct stacks, depths and flags set around one call.
//! - **scratch**: not written; reused buffers whose contents mean nothing
//!   between uses (each user clears them first).
//! - **rebuilt**: not written; a pure cache that refills to the same
//!   value with no observable difference.
//! - **reference**: the borrowed diagnostics engine, the consumer's.
//! - **owned root**: another crate's root, written by its own impl: the
//!   AST (`kiln_ast`) and the Oven (`kiln_oven`).
//!
//! | field | class | why |
//! |---|---|---|
//! | `ast` | owned root | `kiln_ast`'s root (WP-1380); must share the consumer's interner |
//! | `oven` | owned root | `kiln_oven`'s root (WP-1381), applied to the consumer's fresh Oven |
//! | `diags` | reference | the consumer's engine (its state is `kiln_diag`'s root) |
//! | `target`, `abi` | configuration | the target (`--target`, `-march`), [`Sema::for_target`] |
//! | `exceptions` | configuration | `-f[no-]exceptions` |
//! | `rtti` | configuration | `-f[no-]rtti` |
//! | `access_control` | configuration | `-f[no-]access-control` |
//! | `reflection` | configuration | `-freflection` |
//! | `coro_aligned_allocation` | configuration | `-fcoro-aligned-allocation` |
//! | `lax_vector_conversions` | configuration | `-flax-vector-conversions=` |
//! | `range_lifetime_extension` | configuration | `-std=` (C++23 and later) |
//! | `gnu` | configuration | `-std=gnu++NN` |
//! | `gnu89_inline` | configuration | `-fgnu89-inline` / C standard |
//! | `c` | configuration | `-x c`, `-std=cNN` |
//! | `clang_compat` | configuration | `-fclang-compat=` |
//! | `cf_protection_branch` | configuration | `-fcf-protection=` |
//! | `sve`, `sve_vector_bits` | configuration | `-march=...+sve`, `-msve-vector-bits=` |
//! | `aarch64_cpu_valid` | configuration | the driver's CPU table (a `fn`) |
//! | `target_features`, `x86_features`, `target_attr_features` | configuration | `-march`/`-mcpu`/`-target-feature`, the x86 `-march=`/`-m<feature>`/`-mgeneral-regs-only` map, the driver's `target("...")` parser (a `fn`) |
//! | `gnu_lib_builtins` | configuration | `-stdlib=libstdc++` |
//! | `no_builtin`, `no_builtin_fns`, `freestanding` | configuration | `-fno-builtin[-name]`, `-ffreestanding` |
//! | `contracts` | configuration | `-f[no-]contracts` |
//! | `sized_deallocation` | configuration | `-f[no-]sized-deallocation` |
//! | `aligned_allocation` | configuration | `-f[no-]aligned-allocation`, `-f[no-]aligned-new`, `-faligned-new=` |
//! | `contract_checks` | configuration | `-fcontracts-client-check=`, `-fcontracts-definition-check=` |
//! | `short_enums` | configuration | `-f[no-]short-enums` |
//! | `char8` | configuration | `-f[no-]char8_t` |
//! | `lib_builtin_name`, `clang_constexpr_builtin` | configuration | the preprocessor's builtin tables (`fn`s) |
//! | `fp`, `fp_eval_on_command_line` | configuration | `-ffp-model=`, `-ffast-math`, `-f[no-]math-errno`, `-ffp-eval-method=`, ... |
//! | `complex_promoted` | configuration | `-fcomplex-arithmetic=promoted`, clang 22's `-ffp-model=fast` |
//! | `cfg_dump`, `cfg_stats`, `cfg_max_elems` | configuration | `-fkiln-dump-cfg`, `-fkiln-cfg-stats`, `-fkiln-cfg-max-elems=` |
//! | `full_deduction`, `full_redecl_scan` | configuration | test hooks of `ParseOptions` |
//! | `tu_prefix` | configuration | `ParseOptions::prefix` (a header being precompiled) |
//! | `permissive` | configuration | `-fpermissive` |
//! | `host_calls` | configuration | a host-call handler is registered (spec/host-calls.md; a snapshot is never written with one) |
//! | `std` | configuration | `-std=` (set by the parser) |
//! | `inst.limits`, `inst.report.format` (inside the state `inst`) | configuration | `-ftemplate-depth=`, `-ftemplate-backtrace-limit=`, `-fkiln-instantiation-report=`: kept from the consumer's `Sema` |
//! | `spare_oven` | rebuilt | the placeholder of an Oven loan, allocated on demand |
//! | `classify_memo` | rebuilt | name classification memo, valid only for one lookup stamp |
//! | `begin_memo` | rebuilt | the last four `Sema::begin_loc` answers, valid only for one size of `expr_begin` |
//! | `imported_bodies_walked` | rebuilt | imported bodies already walked for `used` functions; a walk again hands nothing new (the handed ones are in the AST) |
//! | `cfg_body`, `uninit_scratch` | scratch | the flow graph and analysis buffers of the body being finished |
//! | `walk_stack`, `conv_walk_stack`, `equiv_scratch`, `cand_scratch`, `deduce_scratch` | scratch | reused walk and deduction buffers |
//! | `complex_div_unmet` | scratch | the prefix's own end-of-TU warning list: clang does not serialize `ExcessPrecisionNotSatisfied`, so the consumer warns only about its own divisions (WP-1795) |
//! | `ctx` | transient | the current declaration context: the translation unit at rest (else the prefix ends inside a namespace or linkage specification) |
//! | `func`, `fn_saved`, `lambdas`, `lambda_base`, `classes`, `suspended_classes`, `defining`, `suspended_defining`, `enums`, `synth` | transient | the function, lambda, class, enumeration and implicit member being defined |
//! | `prototype_params`, `declarator_records`, `requires_param_scopes`, `concept_uses`, `nsdmi_this`, `contract_ctx`, `vis_namespaces` | transient | open parameter lists, declarator scopes, the concept definition being parsed, default member initializers, contract predicates, namespaces with a visibility attribute |
//! | `subst_closures` | transient | the closures substituted in the current top-level declaration, emptied at its end (closure owners, WP-2023) |
//! | `vararg_copying` | transient | the variadic argument being copied (`Sema::check_variadic_args`) |
//! | `inst_outer_anchor` | transient | set inside `Sema::in_inst_context` only |
//! | `eval_replay` | transient | set inside `EvalSession::run` only (spec/eval-session.md §5.1) |
//! | `subst_expr_depth`, `subst_template_id`, `expansion_element_init`, `expansion_vars`, `in_typedef_instantiation`, `instantiating_friend_template`, `friend_cmp_constraints`, `friend_template_apart`, `range_for_outer_depth` | transient | set while one substitution runs |
//! | `unevaluated`, `immediate`, `constant_full_expr`, `capture_reads_variable`, `discarding_init`, `address_operand`, `deref_after_amp`, `decltype_calls`, `typeid_operands`, `pending_odr`, `ref_odr_at_build`, `potential_caps`, `misaligned`, `deferred_call_refs`, `deferring_call_ref`, `extend_collector` | transient | the expression or full-expression being built |
//! | `agg_error_mark`, `agg_excess`, `agg_list_end`, `designator_pending`, `designated_depth`, `written_lists`, `init_entity`, `init_var`, `ctor_entity`, `ctor_diag_ty`, `direct_list_entity`, `list_param_note`, `list_ctor_active`, `ref_temp_note`, `mem_inits`, `initializing` | transient | the initialization being checked |
//! | `cleanup`, `cleanup_saved`, `cleanup_marks` | transient | the cleanups of the full-expression being built |
//! | `delayed_access`, `delayed_access_depth`, `delayed_deprecations`, `depr_points`, `depr_init`, `exempt_next_delay` | transient | open access and deprecation delay points |
//! | `explicit_conv`, `cast_ignores_base_access`, `speculative_friend`, `replaying_access`, `requires_access_replay`, `access_ctx`, `oper_override`, `arith_warning` | transient | set around one conversion or access check |
//! | `trailing_notes`, `omitted_inits`, `diag_capture`, `subst_ledger`, `delayed_typos`, `typo_delay`, `open_specs`, `spec_failures`, `spec_exempt` | transient | diagnostics held for the construct being built |
//! | `nc_strict_only`, `pending_alias_vla`, `reverse_using_shadows`, `declarator_int_recovery`, `declarator_declares_fn`, `declarator_field`, `declarator_distant_exc`, `after_def_prev`, `after_def_held`, `unknown_attr_held`, `unknown_attrs_reported`, `pending_param_abis`, `pattern_member_access`, `base_annotation_packs`, `friend_tag`, `friend_class_template`, `extraneous_tag_head`, `nested_tag`, `inst_enable_if`, `declspec_nulls` | transient | handed from one action to the next within a declaration |
//! | `image_texprs`, `bmi` | transient | declaration-image and carried-constant recording (module files): a snapshot is not taken with them on |
//!
//! Every other field is **state**; [`sema_fields!`] lists them.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use kiln_ast::Ast;
use kiln_snap::{Desc, Reader, Schema, SchemaHash, Snap, SnapError, Writer};

use crate::Sema;

/// The class of a [`Sema`] field in a snapshot (the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldClass {
    /// Written.
    State,
    /// Set by the consumer from its options; in the fingerprint.
    Configuration,
    /// At rest at the snapshot point; not written.
    Transient,
    /// Reused buffers; not written.
    Scratch,
    /// A pure cache; not written.
    Rebuilt,
    /// Borrowed from the consumer.
    Reference,
    /// Another crate's root, written by its own impl.
    OwnedRoot,
}

/// Every `Sema` field by class; `$mac` gets the lists. The single place
/// that classifies a field: the destructures below come from it.
macro_rules! sema_fields {
    ($mac:ident) => {
        $mac! {
            state: [layout_cache, sections, clang_sections, no_sanitize_not_global,
                attr_spellings, traits, reverted_traits, exc_disabled_seen, exc_disabled_pattern,
                consteval_only_pattern, consteval_only_inits, immediate_vars, early_odr_uses,
                default_leaves, call_recovery, nns_start, no_builtin_pending, redecl_index,
                explicit_memo, glibc_float128_alias, cfg_out, inferred_noreturn,
                gsl_inferred_pointers, pack, ms_struct_pragma, fp_pragma, scopes,
                volatile_assigns, imm, invalid_defining, local_externs, c_param_tags,
                dropped_qualified_locals, vis_stack, weak_pending, extname_pending, pragma_attrs,
                missing_typename_fns, extern_c,
                global_ops, braced_args, unowned_params, decomp_written,
                binding_packs, shadow_ctor, binding_pack_elems, refs_minus_assign, attr_macros,
                declared_returns, handler_type_locs, type_spec_locs, inline_fn_begin_locs,
                ref_locs, inner_chunk_locs, pending_chunk_loc, constexpr_begin_locs,
                friend_fn_decls, virt_spec_locs, delayed_bodies,
                constinit_locs, ref_usable, auto_referent, empty_loop_bodies, group_placeholders,
                va_list_type, int128_typedefs, ext_vector_typedefs, splice_fast, meta_subst, ovl,
                failures, designator_rewarn, null_deref_rewarn, null_compare_rewarn,
                vararg_recheck, diagnose_if_recheck, any_diagnose_if, null_deref_seen,
                implicit_declared, special, dependent_access, std_meta_ns, access_failed,
                access_checks, constant_outer_cleanups, vtable_uses, vtables_used, vtable_finals,
                module_vtables, synth_noted, delegating, c_tentatives, undefined_used,
                undefined_used_seen, typo_count, typo_exprs, dropped_typos, cond_typo_at,
                pending_cond_typo, expr_errors, delete_cxx26, unparsed_exc, delayed_exc_equiv,
                unparsed_default_args, uninstantiated_exc, dyn_exc_rparen, implicit_exc_dtors,
                uninstantiated_annots, annot_of_attr, annot_copyable, member_refs, splice_objects, elided_ctors,
                declarator_type_invalid,
                declspec_restrict_error, friend_late_exc, member_friends, inherited_defaults,
                inherited_aligned, inh_ctors, inh_ctor_base, anon_namespaces, has_contracts,
                post_param_diagnosed, post_param_pattern_locs, friend_pattern_prev, templ, inst,
                abstract_noted, long_double_reported, srcloc, meta_names, meta_members, meta_packed_elems, lambda_packs,
                lambda_mangle,
                size_t_sugar, signed_size_t_sugar, ptrdiff_t_sugar, modules],
            configuration: [target, abi, exceptions, rtti, access_control, reflection, coro_aligned_allocation,
                lax_vector_conversions, range_lifetime_extension, gnu, gnu89_inline, c,
                clang_compat, cf_protection_branch, sve, sve_vector_bits, aarch64_cpu_valid,
                target_features, x86_features, target_attr_features,
                gnu_lib_builtins, no_builtin, no_builtin_fns, freestanding, contracts, sized_deallocation,
                aligned_allocation, contract_checks, short_enums, char8, lib_builtin_name, clang_constexpr_builtin, fp, fp_eval_on_command_line, complex_promoted, cfg_dump,
                cfg_stats, full_deduction, full_redecl_scan, tu_prefix, permissive, cfg_max_elems, std,
                host_calls],
            // `ctx` first: the most telling refusal (a prefix ending inside
            // a namespace or `extern "C" {`); then the construct stacks.
            transient: [ctx, classes, func, lambdas, enums, defining, suspended_defining, synth, fn_saved,
                suspended_classes, lambda_base, prototype_params, declarator_records,
                requires_param_scopes, concept_uses, nsdmi_this, subst_closures, contract_ctx, vis_namespaces,
                subst_expr_depth, subst_template_id, expansion_element_init,
                expansion_vars, in_typedef_instantiation, instantiating_friend_template, friend_cmp_constraints,
                friend_template_apart, range_for_outer_depth, unevaluated, immediate,
                constant_full_expr, capture_reads_variable, discarding_init, address_operand, deref_after_amp,
                decltype_calls, typeid_operands, pending_odr, ref_odr_at_build, potential_caps, misaligned,
                deferred_call_refs, deferring_call_ref, extend_collector, agg_error_mark,
                agg_excess, agg_list_end, designator_pending, designated_depth, written_lists,
                init_entity, init_var, ctor_entity,
                ctor_diag_ty, direct_list_entity, list_param_note, list_ctor_active, ref_temp_note, mem_inits,
                initializing, cleanup, cleanup_saved, cleanup_marks, delayed_access,
                delayed_access_depth, delayed_deprecations, depr_points, depr_init,
                exempt_next_delay, explicit_conv, cast_ignores_base_access, speculative_friend,
                replaying_access, requires_access_replay, access_ctx, oper_override, arith_warning,
                trailing_notes, omitted_inits, diag_capture, subst_ledger, delayed_typos, typo_delay, open_specs,
                spec_failures, spec_exempt, nc_strict_only, pending_alias_vla,
                reverse_using_shadows, declarator_int_recovery, declarator_declares_fn, declarator_field,
                declarator_distant_exc, after_def_prev, after_def_held, unknown_attr_held, unknown_attrs_reported, pending_param_abis, this_watch, pattern_member_access, base_annotation_packs, friend_tag,
                pending_qualified_friend, pending_unsupported_friend,
                pending_friend_class_head, pending_friend_class_qual,
                friend_class_template, extraneous_tag_head, nested_tag, inst_enable_if,
                declspec_nulls, image_texprs, bmi, extensions_silenced,
                temp_copy_second_step, vararg_copying, inst_outer_anchor, eval_replay],
            scratch: [cfg_body, uninit_scratch, walk_stack, conv_walk_stack, equiv_scratch,
                cand_scratch, deduce_scratch, complex_div_unmet],
            rebuilt: [spare_oven, classify_memo, begin_memo, imported_bodies_walked],
            reference: [diags],
            owned_root: [ast, oven],
        }
    };
}

/// The classification as data (for the configuration fingerprint and the
/// tests).
macro_rules! field_table {
    (state: [$($s:ident),*], configuration: [$($c:ident),*], transient: [$($t:ident),*],
     scratch: [$($x:ident),*], rebuilt: [$($b:ident),*], reference: [$($f:ident),*],
     owned_root: [$($o:ident),*],) => {
        &[
            $((stringify!($s), FieldClass::State),)*
            $((stringify!($c), FieldClass::Configuration),)*
            $((stringify!($t), FieldClass::Transient),)*
            $((stringify!($x), FieldClass::Scratch),)*
            $((stringify!($b), FieldClass::Rebuilt),)*
            $((stringify!($f), FieldClass::Reference),)*
            $((stringify!($o), FieldClass::OwnedRoot),)*
        ]
    };
}

/// Every `Sema` field with its class, in class order.
pub const SNAPSHOT_FIELDS: &[(&str, FieldClass)] = sema_fields!(field_table);

/// Whether a transient field is at rest: as a fresh [`Sema`] has it.
trait AtRest {
    fn at_rest(&self) -> bool;
}

impl AtRest for bool {
    fn at_rest(&self) -> bool {
        !*self
    }
}

impl AtRest for u32 {
    fn at_rest(&self) -> bool {
        *self == 0
    }
}

impl AtRest for usize {
    fn at_rest(&self) -> bool {
        *self == 0
    }
}

impl<T> AtRest for Option<T> {
    fn at_rest(&self) -> bool {
        self.is_none()
    }
}

impl<T> AtRest for Vec<T> {
    fn at_rest(&self) -> bool {
        self.is_empty()
    }
}

impl<K, V, S> AtRest for HashMap<K, V, S> {
    fn at_rest(&self) -> bool {
        self.is_empty()
    }
}

impl<K, S> AtRest for HashSet<K, S> {
    fn at_rest(&self) -> bool {
        self.is_empty()
    }
}

impl<T: AtRest + Copy> AtRest for Cell<T> {
    fn at_rest(&self) -> bool {
        self.get().at_rest()
    }
}

impl<T: AtRest> AtRest for RefCell<T> {
    fn at_rest(&self) -> bool {
        self.try_borrow().is_ok_and(|v| v.at_rest())
    }
}

impl AtRest for crate::subst_failed::SubstLedger {
    fn at_rest(&self) -> bool {
        self.is_empty()
    }
}

impl AtRest for crate::class::CleanupState {
    fn at_rest(&self) -> bool {
        *self == Self::default()
    }
}

impl AtRest for crate::class::ThisWatch {
    fn at_rest(&self) -> bool {
        *self == Self::Off
    }
}

impl AtRest for crate::class::MemInitState {
    fn at_rest(&self) -> bool {
        self.is_at_rest()
    }
}

impl AtRest for crate::bmi_image::TexprState {
    fn at_rest(&self) -> bool {
        self.is_at_rest()
    }
}

impl AtRest for crate::bmi_ast::BmiState {
    fn at_rest(&self) -> bool {
        self.is_at_rest()
    }
}

/// `ctx` is at rest when it is the translation unit; [`Sema::snapshot_ready`]
/// checks it first, by hand.
impl AtRest for kiln_ast::DeclId {
    fn at_rest(&self) -> bool {
        true
    }
}

/// Adds `field` to a decoding error's path.
fn at<T>(field: &'static str, r: Result<T, SnapError>) -> Result<T, SnapError> {
    r.map_err(|e| e.within(field))
}

/// Registers `name` with the type `field` returns (a field's type, named
/// without writing it).
fn reg<T: Snap>(
    d: Desc,
    name: &str,
    _field: for<'x> fn(&'x Sema<'static, 'static>) -> &'x T,
) -> Desc {
    d.field::<T>(name)
}

/// [`reg`] for the constant hash: folds `name` and the constant of the
/// type `field` returns.
const fn reg_hash<T: Snap>(
    h: SchemaHash,
    name: &str,
    _field: for<'x> fn(&'x Sema<'static, 'static>) -> &'x T,
) -> SchemaHash {
    h.field::<T>(name)
}

fn recurse<T: Snap>(s: &mut Schema, _field: for<'x> fn(&'x Sema<'static, 'static>) -> &'x T) {
    T::schema(s);
}

/// Writes the AST through its own root (`kiln_ast`, spec/ast.md
/// "Snapshots"), its interner included as a shared `Arc`.
fn put_ast(ast: &Ast, w: &mut Writer) {
    w.put(ast);
    // The written-reference section (spec/ast.md §18.10.1): the table
    // when recording is on, else one byte.
    ast.put_written_refs(w);
}

/// Reads the AST [`put_ast`] wrote into `into`. The decoded AST must share
/// `into`'s interner: the consumer reads the interner earlier in the same
/// stream (the preprocessor's root) and builds its fresh `Sema` over it,
/// so the AST's `Arc` is a back-reference to that allocation; a fresh
/// interner here would not be the preprocessor's.
///
/// The written-reference section after it is decoded only when `into`
/// records (the consumer's option, set before the restore); otherwise it
/// is skipped. `files` is the number of files the restored source manager
/// holds, for the gap row of a section written with recording off
/// (spec/ast.md §18.10.1).
fn get_ast(r: &mut Reader<'_>, into: &mut Ast, files: u32) -> Result<(), SnapError> {
    let mut ast: Ast = r.get()?;
    if !std::ptr::eq(ast.symbols(), into.symbols()) {
        return Err(r.invalid::<Ast>(
            "the AST's interner is not the consumer's (read the interner first, in the same stream)",
        ));
    }
    ast.get_written_refs(r, into.written_refs.is_on(), files)?;
    *into = ast;
    Ok(())
}

/// Writes the Oven through its own root (`kiln_oven`, spec/oven.md
/// "Snapshots"); [`Sema::snapshot_ready`] has checked it is at rest.
fn put_oven(oven: &kiln_oven::Oven, w: &mut Writer) -> Result<(), String> {
    oven.snap_put(w)
}

/// Applies the Oven [`put_oven`] wrote to `into`, the consumer's fresh
/// Oven (configured by [`Sema::set_constexpr_limits`]).
fn get_oven(r: &mut Reader<'_>, into: &mut kiln_oven::Oven) -> Result<(), SnapError> {
    into.snap_restore(r)
}

macro_rules! root_impl {
    (state: [$($s:ident),*], configuration: [$($c:ident),*], transient: [$($t:ident),*],
     scratch: [$($x:ident),*], rebuilt: [$($b:ident),*], reference: [$($f:ident),*],
     owned_root: [$($o:ident),*],) => {
        impl<'a, 'sm> Sema<'a, 'sm> {
            /// Whether `self` is at rest for a snapshot (spec/sema.md
            /// "Snapshots"): at top level (the translation unit is the
            /// declaration context and its scope the only one), no
            /// construct open, no evaluation running, nothing held. The
            /// error names the first field that is not at rest.
            ///
            /// # Errors
            /// The field that is not at rest, and why.
            pub fn snapshot_ready(&self) -> Result<(), String> {
                let Sema { $($s: _,)* $($c: _,)* $($t,)* $($x: _,)* $($b: _,)* $($f: _,)* $($o: _,)* } = self;
                let scopes = &self.scopes;
                if self.ctx != self.ast.tu {
                    return Err(format!(
                        "Sema.ctx: inside {} (the prefix ends inside a declaration context)",
                        self.context_name(self.ctx)
                    ));
                }
                if scopes.len() != 1 {
                    let kind = scopes.last().map(|s| s.kind);
                    return Err(format!(
                        "Sema.scopes: {} scopes open (innermost {kind:?}), not only the translation unit's",
                        scopes.len()
                    ));
                }
                $(
                    if !AtRest::at_rest($t) {
                        return Err(concat!("Sema.", stringify!($t), ": not at rest").to_owned());
                    }
                )*
                match &self.oven {
                    None => return Err("Sema.oven: an evaluation is running".to_owned()),
                    Some(o) => o.snap_ready().map_err(|why| format!("Sema.oven > {why}"))?,
                }
                self.state_ready()
            }

            /// Writes the state of a quiescent `Sema` and its AST and Oven
            /// (spec/sema.md "Snapshots"); the writer's mode decides the
            /// map order ([`Writer::canonical`]).
            ///
            /// # Errors
            /// [`Self::snapshot_ready`]'s: nothing is written then.
            pub fn snap_put(&self, w: &mut Writer) -> Result<(), String> {
                self.snapshot_ready()?;
                let Sema { $($s,)* $($c: _,)* $($t: _,)* $($x: _,)* $($b: _,)* $($f: _,)* $($o: _,)* } = self;
                put_ast(&self.ast, w);
                if let Some(o) = &self.oven {
                    put_oven(o, w)?;
                }
                $( w.put($s); )*
                Ok(())
            }

            /// Applies a snapshot written by [`Self::snap_put`] to `self`,
            /// a `Sema` the consumer has just built and configured from its
            /// own options ([`Sema::for_target`], the parser's
            /// configuration, [`Sema::set_constexpr_limits`]) over the
            /// restored source manager, interner and diagnostics engine.
            /// Configuration, references, transient and scratch fields keep
            /// the consumer's values; everything else is the producer's,
            /// with every id unchanged. On an error `self` is unchanged
            /// except for the AST and the Oven, which the caller then drops.
            ///
            /// # Errors
            /// A corrupt snapshot ([`SnapError`], never a panic), or a
            /// `Sema` that is not fresh (`ErrorKind::Invalid`).
            pub fn snap_restore(&mut self, r: &mut Reader<'_>) -> Result<(), SnapError> {
                self.snap_restore_split(r, &mut |_| {})
            }

            /// [`Self::snap_restore`], calling `mark` after each part is
            /// decoded (`"ast"`, `"oven"`, `"fields"`): the per-root load
            /// timer of the snapshot prototype (WP-1383).
            ///
            /// # Errors
            /// As [`Self::snap_restore`].
            pub fn snap_restore_split(
                &mut self,
                r: &mut Reader<'_>,
                mark: &mut dyn FnMut(&'static str),
            ) -> Result<(), SnapError> {
                if self.snapshot_ready().is_err() || !self.is_fresh() {
                    return Err(r.invalid::<Self>("restore into a used Sema"));
                }
                let files = u32::try_from(self.diags.sm().file_count()).unwrap_or(u32::MAX);
                at("Sema.ast", get_ast(r, &mut self.ast, files))?;
                mark("ast");
                if let Some(o) = &mut self.oven {
                    at("Sema.oven", get_oven(r, o))?;
                }
                mark("oven");
                $( let $s = at(concat!("Sema.", stringify!($s)), r.get())?; )*
                let limits = self.inst.limits;
                let report_format = self.inst.report.format;
                // The consumer's stack guard (a per-thread measure).
                let guard = std::mem::take(&mut self.inst.guard);
                let Sema { $($s: _,)* $($c: _,)* $($t: _,)* $($x: _,)* $($b: _,)* $($f: _,)* $($o: _,)* } = self;
                $( self.$s = $s; )*
                self.inst.limits = limits;
                self.inst.report.format = report_format;
                self.inst.guard = guard;
                // The Oven's and these fields' ids index the AST decoded
                // before them (spec/snap.md "Id ranges").
                if let Err(why) = self.check_restored().and_then(|()| self.ast.check_ids(r)) {
                    return Err(r.invalid::<Self>(why));
                }
                mark("fields");
                Ok(())
            }

            /// The encoded size of each part [`Self::snap_put`] writes, in
            /// write order (`ast`, `oven`, then each state field): what a
            /// snapshot's size is made of. Normal mode.
            pub fn snap_sizes(&self) -> Vec<(&'static str, usize)> {
                let Sema { $($s,)* $($c: _,)* $($t: _,)* $($x: _,)* $($b: _,)* $($f: _,)* $($o: _,)* } = self;
                let mut out = Vec::new();
                let mut w = Writer::new();
                put_ast(&self.ast, &mut w);
                out.push(("ast", w.position()));
                let mut w = Writer::new();
                if let Some(o) = &self.oven {
                    // At rest when `snapshot_ready` is; else nothing is
                    // written and the size is 0.
                    let _ = put_oven(o, &mut w);
                }
                out.push(("oven", w.position()));
                $(
                    let mut w = Writer::new();
                    w.put($s);
                    out.push((stringify!($s), w.position()));
                )*
                out
            }

            /// The constant schema hash of the root ([`Self::snap_schema`],
            /// item for item).
            pub const SNAP_SCHEMA_HASH: u64 = {
                let mut h = SchemaHash::new("configured root", "Sema")
                    .field::<Ast>("ast")
                    .root_field("written refs", kiln_ast::written_refs::CARRY_SCHEMA_HASH)
                    .root_field("oven", kiln_oven::Oven::SNAP_SCHEMA_HASH);
                $( h = reg_hash(h, stringify!($s), |x| &x.$s); )*
                $( h = h.skip(stringify!($c), "()"); )*
                $( h = h.skip(stringify!($t), "()"); )*
                $( h = h.skip(stringify!($x), "()"); )*
                $( h = h.skip(stringify!($b), "()"); )*
                h.finish()
            };

            /// Registers the snapshot's descriptor (the fields
            /// [`Self::snap_put`] writes besides the AST and the Oven) and
            /// those of the types it encodes.
            pub fn snap_schema(s: &mut Schema) {
                let mut d = Desc::new("configured root")
                    .hash(Self::SNAP_SCHEMA_HASH)
                    .field::<Ast>("ast")
                    .field::<kiln_ast::written_refs::WrittenRefsSection>("written refs")
                    .field::<kiln_oven::Oven>("oven");
                $( d = reg(d, stringify!($s), |x| &x.$s); )*
                $( d = d.skip::<()>(stringify!($c)); )*
                $( d = d.skip::<()>(stringify!($t)); )*
                $( d = d.skip::<()>(stringify!($x)); )*
                $( d = d.skip::<()>(stringify!($b)); )*
                if s.register::<Sema<'static, 'static>>(d) {
                    Ast::schema(s);
                    <kiln_ast::written_refs::WrittenRefsSection as kiln_snap::Snap>::schema(s);
                    kiln_oven::Oven::snap_schema(s);
                    $( recurse(s, |x| &x.$s); )*
                }
            }
        }
    };
}

sema_fields!(root_impl);

impl Sema<'_, '_> {
    /// How the refusal of a prefix that ends inside `ctx` names it.
    fn context_name(&self, ctx: kiln_ast::DeclId) -> String {
        let kind = self.ast.decls.get(ctx).kind();
        let name = self.qualified_decl_name(ctx);
        if name.is_empty() { format!("{kind:?}") } else { format!("{kind:?} '{name}'") }
    }

    /// The parts of state fields that are transient (the instantiation
    /// stack, open template heads, the pending immediate invocations of a
    /// full-expression): at rest at the snapshot point.
    fn state_ready(&self) -> Result<(), String> {
        if !self.inst.stack.is_empty() {
            return Err("Sema.inst.stack: an instantiation is running".to_owned());
        }
        if let Some(why) = self.inst.not_at_rest() {
            return Err(format!("Sema.inst.{why}: not at rest"));
        }
        if let Some(why) = self.templ.not_at_rest() {
            return Err(format!("Sema.templ.{why}: not at rest"));
        }
        if let Some(why) = self.imm.not_at_rest() {
            return Err(format!("Sema.imm.{why}: not at rest"));
        }
        Ok(())
    }

    /// Whether nothing was analysed yet: the consumer's `Sema` before a
    /// restore.
    fn is_fresh(&self) -> bool {
        self.failures.get() == 0
            && self.undefined_used.is_empty()
            && self.vtable_uses.is_empty()
            && self.inst.pending.is_empty()
            && self.scopes.len() == 1
    }

    /// The invariants of a restored `Sema` that its queries rely on
    /// without a check (spec/snap.md "Hand-written roots", rule 3): one
    /// scope, the translation unit's, and the translation unit as the
    /// context.
    fn check_restored(&mut self) -> Result<(), &'static str> {
        let tu = self.ast.tu;
        match self.scopes.as_slice() {
            [s] if s.kind == crate::ScopeKind::TranslationUnit && s.entity == Some(tu) => {}
            _ => return Err("the scopes are not the translation unit's alone"),
        }
        self.ctx = tu;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::put_ast;
    use kiln_ast::Ast;
    use kiln_ast::written_refs::{RefLookup, RefQualifier, RefRole, WrittenRef};
    use kiln_snap::Writer;
    use kiln_source::{Loc, Span};
    use std::sync::Arc;

    fn ast(interner: &Arc<kiln_source::symbol::Interner>) -> Ast {
        Ast::new(Arc::clone(interner), kiln_ast::types::TypeContext::default())
    }

    fn bytes(f: impl FnOnce(&mut Writer)) -> Vec<u8> {
        let mut w = Writer::canonical();
        f(&mut w);
        w.into_bytes()
    }

    /// WP-1934 (spec/ast.md §18.10.4): with recording off, the snapshot
    /// is the AST's own encoding and one empty-section byte; with it on,
    /// the AST's encoding is unchanged and the table follows it.
    #[test]
    fn the_table_off_adds_one_byte_to_the_ast() {
        let interner = Arc::new(kiln_source::symbol::Interner::new());
        let off = ast(&interner);
        let plain = bytes(|w| w.put(&off));
        let mut want = plain.clone();
        want.push(0);
        assert_eq!(bytes(|w| put_ast(&off, w)), want);
        let mut on = ast(&interner);
        on.written_refs.set_on(true);
        let _ = on.written_refs.push(WrittenRef {
            range: Span::new(Loc::from_raw(3), Loc::from_raw(4)),
            entity: None,
            found: None,
            enclosing: None,
            qualifier: RefQualifier::NONE,
            role: RefRole::Expr,
            flags: 0,
            how: RefLookup::None,
        });
        let with = bytes(|w| put_ast(&on, w));
        assert!(with.len() > want.len() && with.starts_with(&plain));
        // The reading side (skipped off, a gap row for an empty section)
        // is `kiln_ast`'s `off_writes_one_byte_and_reads_nothing`.
    }
}
