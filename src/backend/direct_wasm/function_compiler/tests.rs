use std::collections::{HashMap, HashSet};

use crate::backend::direct_wasm::is_internal_user_function_identifier;
use crate::backend::direct_wasm::state::{StaticThrowValue, StaticValueKind};
use crate::frontend;
use crate::ir::hir::{CallArgument, FunctionKind, ObjectEntry};

use super::collect_referenced_binding_names_from_expression;
use super::{
    DirectWasmCompiler, Expression, FunctionCompiler, IteratorSourceKind, LocalFunctionBinding,
    OrdinaryToPrimitiveAnalysis, SimpleGeneratorStep, SimpleGeneratorStepOutcome, Statement,
    StaticEvalOutcome, collect_eval_local_function_declarations, internal_function_name_hint,
    namespace_eval_program_internal_function_names, object_binding_lookup_value,
};

#[test]
fn collects_eval_local_function_declarations_in_ordinary_eval_context() {
    let compiler = DirectWasmCompiler::default();
    let program = compiler
        .parse_eval_program_in_ordinary_function_context_static(
            "initial = f; function f() { return 33; }",
        )
        .expect("ordinary eval wrapper should parse");

    let local_function_names = program
        .functions
        .iter()
        .filter(|function| !function.register_global)
        .map(|function| function.name.clone())
        .collect::<HashSet<_>>();
    let declarations =
        collect_eval_local_function_declarations(&program.statements, &local_function_names);

    assert_eq!(declarations.len(), 1);
    let binding_name = declarations
        .keys()
        .next()
        .expect("expected one eval-local function declaration binding");
    assert!(binding_name.starts_with("__ayy_scope$f$"));
}

#[test]
fn collects_parameter_object_bindings_for_repeated_getter_spread_calls() {
    let program = frontend::parse(
        r#"
            let getterCallCount = 0;
            let repeated = {
              get a() {
                return ++getterCallCount;
              }
            };
            (function(second) {
              console.log(second.a, second.c, second.d, Object.keys(second).length);
            })({ ...repeated, c: 4, d: 5, a: 42, ...repeated });
            "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler.compile(&program).expect("program should compile");
    compiler.register_user_function_capture_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    assert!(
        compiler
            .infer_global_member_getter_binding(
                &Expression::Identifier("repeated".to_string()),
                &Expression::String("a".to_string())
            )
            .is_some(),
        "expected repeated.a getter binding"
    );
    assert_eq!(
        compiler.infer_global_member_getter_return_value(
            &Expression::Identifier("repeated".to_string()),
            &Expression::String("a".to_string())
        ),
        Some(Expression::Number(1.0))
    );

    let repeated_copy = compiler
        .infer_global_copy_data_properties_binding(&Expression::Identifier("repeated".to_string()))
        .expect("expected repeated copy binding");
    assert_eq!(
        object_binding_lookup_value(&repeated_copy, &Expression::String("a".to_string())),
        Some(&Expression::Number(1.0))
    );

    let call_argument = match &program.statements[2] {
        crate::ir::hir::Statement::Expression(Expression::Call { arguments, .. }) => {
            match arguments.first().expect("expected callback call argument") {
                crate::ir::hir::CallArgument::Expression(expression)
                | crate::ir::hir::CallArgument::Spread(expression) => expression,
            }
        }
        _ => panic!("expected top-level callback call expression"),
    };
    let object_argument_binding = compiler
        .infer_global_object_binding(call_argument)
        .expect("expected object literal binding");
    assert_eq!(
        object_binding_lookup_value(
            &object_argument_binding,
            &Expression::String("a".to_string())
        ),
        Some(&Expression::Number(2.0))
    );

    let (_, _, _, object_bindings) = compiler.collect_user_function_parameter_bindings(&program);

    let function_name = program
        .functions
        .iter()
        .find(|function| {
            function
                .params
                .iter()
                .map(|param| param.name.as_str())
                .eq(["second"])
        })
        .expect("expected anonymous callback function")
        .name
        .clone();
    let binding = object_bindings
        .get(&function_name)
        .and_then(|bindings| bindings.get("second"))
        .cloned()
        .flatten()
        .expect("expected second parameter object binding");

    assert_eq!(
        object_binding_lookup_value(&binding, &Expression::String("a".to_string())),
        Some(&Expression::Number(2.0))
    );
    assert_eq!(
        object_binding_lookup_value(&binding, &Expression::String("c".to_string())),
        Some(&Expression::Number(4.0))
    );
    assert_eq!(
        object_binding_lookup_value(&binding, &Expression::String("d".to_string())),
        Some(&Expression::Number(5.0))
    );
}

#[test]
fn collects_parameter_object_bindings_for_symbol_spread_calls() {
    let program = frontend::parse(
        r#"
            let symbol = Symbol('foo');
            let o = {};
            o[symbol] = 1;
            (function(obj) {
              console.log(obj[symbol], obj.c, obj.d);
            }.apply(null, [{...o, c: 4, d: 5}]));
            "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let (_, _, _, object_bindings) = compiler.collect_user_function_parameter_bindings(&program);

    let function_name = program
        .functions
        .iter()
        .find(|function| {
            function
                .params
                .iter()
                .map(|param| param.name.as_str())
                .eq(["obj"])
        })
        .expect("expected anonymous callback function")
        .name
        .clone();
    let binding = object_bindings
        .get(&function_name)
        .and_then(|bindings| bindings.get("obj"))
        .cloned()
        .flatten()
        .expect("expected obj parameter object binding");

    assert_eq!(
        object_binding_lookup_value(&binding, &Expression::Identifier("symbol".to_string())),
        Some(&Expression::Number(1.0))
    );
    assert_eq!(
        object_binding_lookup_value(&binding, &Expression::String("c".to_string())),
        Some(&Expression::Number(4.0))
    );
    assert_eq!(
        object_binding_lookup_value(&binding, &Expression::String("d".to_string())),
        Some(&Expression::Number(5.0))
    );
}

#[test]
fn allows_explicit_call_frame_inlining_for_class_prototype_descriptor_helper_calls() {
    let program = frontend::parse(
        r#"
            function assertAccessorDescriptor(object, name) {
              var desc = Object.getOwnPropertyDescriptor(object, name);
              assert.sameValue(desc.configurable, true);
              assert.sameValue(desc.enumerable, false);
              assert.sameValue(typeof desc.get, 'function');
              assert.sameValue(typeof desc.set, 'function');
            }

            class C {
              get x() { return this._x; }
              set x(v) { this._x = v; }
              static get staticX() { return this._x; }
              static set staticX(v) { this._x = v; }
            }

            assertAccessorDescriptor(C.prototype, 'x');
            assertAccessorDescriptor(C, 'staticX');
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);
    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;

    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| function.name == "assertAccessorDescriptor")
        .cloned()
        .expect("expected helper function");

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        true,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");

    assert!(
        function_compiler.can_inline_user_function_call_with_explicit_call_frame(
            &user_function,
            &[
                Expression::Member {
                    object: Box::new(Expression::Identifier("C".to_string())),
                    property: Box::new(Expression::String("prototype".to_string())),
                },
                Expression::String("x".to_string()),
            ],
            &Expression::This,
        )
    );
}

#[test]
fn emits_explicit_call_frame_inline_summary_for_class_prototype_descriptor_helper_calls() {
    let program = frontend::parse(
        r#"
            function assertAccessorDescriptor(object, name) {
              var desc = Object.getOwnPropertyDescriptor(object, name);
              assert.sameValue(desc.configurable, true);
              assert.sameValue(desc.enumerable, false);
              assert.sameValue(typeof desc.get, 'function');
              assert.sameValue(typeof desc.set, 'function');
            }

            class C {
              get x() { return this._x; }
              set x(v) { this._x = v; }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| function.name == "assertAccessorDescriptor")
        .cloned()
        .expect("expected helper function");

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        true,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("top-level statement should emit");
    }
    assert!(
        function_compiler
            .resolve_member_getter_binding(
                &Expression::Member {
                    object: Box::new(Expression::Identifier("C".to_string())),
                    property: Box::new(Expression::String("prototype".to_string())),
                },
                &Expression::String("x".to_string()),
            )
            .is_some(),
        "expected prototype getter binding after class emission",
    );
    assert!(
        function_compiler
            .resolve_member_setter_binding(
                &Expression::Member {
                    object: Box::new(Expression::Identifier("C".to_string())),
                    property: Box::new(Expression::String("prototype".to_string())),
                },
                &Expression::String("x".to_string()),
            )
            .is_some(),
        "expected prototype setter binding after class emission",
    );
    let function_declaration = program
        .functions
        .iter()
        .find(|function| function.name == user_function.name)
        .expect("expected helper declaration");
    let Statement::Var { value, .. } = &function_declaration.body[0] else {
        panic!("expected substituted descriptor initializer");
    };
    let substituted_value = function_compiler.substitute_user_function_call_frame_bindings(
        value,
        &user_function,
        &[
            CallArgument::Expression(Expression::Member {
                object: Box::new(Expression::Identifier("C".to_string())),
                property: Box::new(Expression::String("prototype".to_string())),
            }),
            CallArgument::Expression(Expression::String("x".to_string())),
        ],
        &Expression::This,
        &Expression::Array(vec![
            crate::ir::hir::ArrayElement::Expression(Expression::Member {
                object: Box::new(Expression::Identifier("C".to_string())),
                property: Box::new(Expression::String("prototype".to_string())),
            }),
            crate::ir::hir::ArrayElement::Expression(Expression::String("x".to_string())),
        ]),
    );
    assert!(
        function_compiler
            .resolve_descriptor_binding_from_expression(&substituted_value)
            .is_some(),
        "expected substituted descriptor expression to resolve",
    );
    function_compiler
        .emit_statement(&Statement::Var {
            name: "desc".to_string(),
            value: substituted_value,
        })
        .expect("substituted descriptor var should emit");
    let descriptor = function_compiler
        .state
        .speculation
        .static_semantics
        .objects
        .local_descriptor_bindings
        .get("desc")
        .cloned()
        .expect("expected inline desc descriptor binding");
    assert!(descriptor.configurable);
    assert!(!descriptor.enumerable);
    assert!(descriptor.has_get);
    assert!(descriptor.has_set);
}

#[test]
fn resolves_static_math_intrinsic_numbers_through_bound_identifiers() {
    let program = frontend::parse(
        r#"
            let atanNegZero = Math.atan(-0);
            let maxNaN = Math.max({});
            let maxSignedZero = 1 / Math.max(-0, 0);
            let minSignedZero = 1 / Math.min(-0, 0);

            console.log(
              1 / atanNegZero,
              maxNaN !== maxNaN,
              maxSignedZero,
              minSignedZero
            );
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..4] {
        function_compiler
            .emit_statement(statement)
            .expect("let binding should emit");
    }

    let Statement::Print { values } = &program.statements[4] else {
        panic!("expected trailing print statement");
    };

    assert_eq!(
        function_compiler.resolve_static_number_value(&values[0]),
        Some(f64::NEG_INFINITY)
    );
    assert_eq!(
        function_compiler.resolve_static_boolean_expression(&values[1]),
        Some(true)
    );
    assert_eq!(
        function_compiler.resolve_static_number_value(&values[2]),
        Some(f64::INFINITY)
    );
    assert_eq!(
        function_compiler.resolve_static_number_value(&values[3]),
        Some(f64::NEG_INFINITY)
    );
}

#[test]
fn tracks_descriptor_locals_from_get_own_property_descriptor_with_bound_parameter_name() {
    let program = frontend::parse(
        r#"
            function f(object, name) {
              var desc = Object.getOwnPropertyDescriptor(object, name);
              console.log(desc.configurable, typeof desc.get, typeof desc.set);
            }
            class C {
              get x() { return this._x; }
              set x(v) { this._x = v; }
            }
            f(C.prototype, "x");
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;

    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| {
            function
                .params
                .iter()
                .map(|param| param.as_str())
                .eq(["object", "name"])
        })
        .cloned()
        .expect("expected helper function");
    let function_declaration = program
        .functions
        .iter()
        .find(|function| function.name == user_function.name)
        .cloned()
        .expect("expected helper declaration");
    let function_parameter_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_value_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_array_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_object_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let object_parameter_value = function_parameter_value_bindings
        .get("object")
        .cloned()
        .flatten()
        .expect("expected object parameter value binding");
    assert!(matches!(
        function_parameter_value_bindings.get("name"),
        Some(Some(Expression::String(name))) if name == "x"
    ));
    assert!(matches!(
        function_parameter_value_bindings.get("object"),
        Some(Some(_))
    ));

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        Some(&user_function),
        false,
        false,
        user_function.strict,
        &function_parameter_bindings,
        &function_parameter_value_bindings,
        &function_parameter_array_bindings,
        &function_parameter_object_bindings,
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&function_declaration.body)
        .expect("bindings should register");
    assert!(
        function_compiler
            .resolve_object_binding_from_expression(&object_parameter_value)
            .is_some()
    );
    assert!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .objects
            .local_object_bindings
            .contains_key("object")
    );
    function_compiler
        .emit_statement(&function_declaration.body[0])
        .expect("descriptor local should emit");

    let descriptor = function_compiler
        .state
        .speculation
        .static_semantics
        .objects
        .local_descriptor_bindings
        .get("desc")
        .cloned()
        .expect("expected desc descriptor binding");
    assert!(descriptor.configurable);
    assert!(!descriptor.enumerable);
    assert!(descriptor.has_get);
    assert!(descriptor.has_set);
    assert_eq!(
        function_compiler.resolve_static_if_condition_value(&Expression::Member {
            object: Box::new(Expression::Identifier("desc".to_string())),
            property: Box::new(Expression::String("configurable".to_string())),
        }),
        Some(true)
    );
    assert!(matches!(
        function_compiler.infer_typeof_operand_kind(&Expression::Member {
            object: Box::new(Expression::Identifier("desc".to_string())),
            property: Box::new(Expression::String("get".to_string())),
        }),
        Some(StaticValueKind::Function)
    ));
    assert!(matches!(
        function_compiler.infer_typeof_operand_kind(&Expression::Member {
            object: Box::new(Expression::Identifier("desc".to_string())),
            property: Box::new(Expression::String("set".to_string())),
        }),
        Some(StaticValueKind::Function)
    ));
    assert!(matches!(
        function_compiler.materialize_static_expression(&Expression::Member {
            object: Box::new(Expression::Identifier("desc".to_string())),
            property: Box::new(Expression::String("configurable".to_string())),
        }),
        Expression::Bool(true)
    ));
    assert!(matches!(
        function_compiler.materialize_static_expression(&Expression::Member {
            object: Box::new(Expression::Identifier("desc".to_string())),
            property: Box::new(Expression::String("enumerable".to_string())),
        }),
        Expression::Bool(false)
    ));
}

#[test]
fn resolves_descriptor_member_reads_through_hidden_inline_local_bindings() {
    let program = frontend::parse(
        r#"
            function f(object, name) {
              var desc = Object.getOwnPropertyDescriptor(object, name);
              console.log(desc.configurable, typeof desc.get, typeof desc.set);
            }
            class C {
              get x() { return this._x; }
              set x(v) { this._x = v; }
            }
            f(C.prototype, "x");
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);
    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;

    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| function.name == "f")
        .cloned()
        .expect("expected helper function");
    let function_declaration = program
        .functions
        .iter()
        .find(|function| function.name == user_function.name)
        .cloned()
        .expect("expected helper declaration");
    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        true,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    function_compiler
        .emit_statement(&program.statements[0])
        .expect("class statement should emit before helper call");

    let hidden_desc_name = function_compiler
        .allocate_named_hidden_local("inline_local_desc", StaticValueKind::Unknown);
    function_compiler
        .state
        .emission
        .lexical_scopes
        .active_scoped_lexical_bindings
        .entry("desc".to_string())
        .or_default()
        .push(hidden_desc_name.clone());

    let Statement::Var { value, .. } = &function_declaration.body[0] else {
        panic!("expected descriptor initializer");
    };
    let substituted_value = function_compiler.substitute_user_function_call_frame_bindings(
        value,
        &user_function,
        &[
            CallArgument::Expression(Expression::Member {
                object: Box::new(Expression::Identifier("C".to_string())),
                property: Box::new(Expression::String("prototype".to_string())),
            }),
            CallArgument::Expression(Expression::String("x".to_string())),
        ],
        &Expression::This,
        &Expression::Array(vec![
            crate::ir::hir::ArrayElement::Expression(Expression::Member {
                object: Box::new(Expression::Identifier("C".to_string())),
                property: Box::new(Expression::String("prototype".to_string())),
            }),
            crate::ir::hir::ArrayElement::Expression(Expression::String("x".to_string())),
        ]),
    );
    let substituted_descriptor = function_compiler
        .resolve_descriptor_binding_from_expression(&substituted_value)
        .expect("expected substituted descriptor binding");
    assert!(substituted_descriptor.has_get);
    assert!(substituted_descriptor.has_set);
    function_compiler
        .emit_statement(&Statement::Var {
            name: "desc".to_string(),
            value: substituted_value,
        })
        .expect("descriptor local should emit");

    let hidden_descriptor = function_compiler
        .state
        .speculation
        .static_semantics
        .objects
        .local_descriptor_bindings
        .get(&hidden_desc_name)
        .cloned()
        .expect("expected hidden inline descriptor binding");
    assert!(hidden_descriptor.configurable);
    assert!(!hidden_descriptor.enumerable);
    assert!(hidden_descriptor.has_get);
    assert!(hidden_descriptor.has_set);

    assert!(matches!(
        function_compiler.resolve_static_if_condition_value(&Expression::Member {
            object: Box::new(Expression::Identifier("desc".to_string())),
            property: Box::new(Expression::String("configurable".to_string())),
        }),
        Some(true)
    ));
    assert!(matches!(
        function_compiler.infer_typeof_operand_kind(&Expression::Member {
            object: Box::new(Expression::Identifier("desc".to_string())),
            property: Box::new(Expression::String("get".to_string())),
        }),
        Some(StaticValueKind::Function)
    ));
    assert!(matches!(
        function_compiler.infer_typeof_operand_kind(&Expression::Member {
            object: Box::new(Expression::Identifier("desc".to_string())),
            property: Box::new(Expression::String("set".to_string())),
        }),
        Some(StaticValueKind::Function)
    ));
}

#[test]
fn resolves_descriptor_binding_from_hidden_inline_parameter_locals() {
    let program = frontend::parse(
        r#"
            function f(object, name) {
              var desc = Object.getOwnPropertyDescriptor(object, name);
              console.log(desc.configurable, typeof desc.get, typeof desc.set);
            }
            class C {
              get x() { return this._x; }
              set x(v) { this._x = v; }
            }
            f(C.prototype, "x");
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);
    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;

    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| function.name == "f")
        .cloned()
        .expect("expected helper function");
    let function_declaration = program
        .functions
        .iter()
        .find(|function| function.name == user_function.name)
        .cloned()
        .expect("expected helper declaration");
    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        true,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    function_compiler
        .emit_statement(&program.statements[0])
        .expect("class statement should emit before helper call");

    let hidden_object_name = function_compiler
        .allocate_named_hidden_local("inline_param_object", StaticValueKind::Unknown);
    function_compiler
        .update_capture_slot_binding_from_expression(
            &hidden_object_name,
            &Expression::Member {
                object: Box::new(Expression::Identifier("C".to_string())),
                property: Box::new(Expression::String("prototype".to_string())),
            },
        )
        .expect("hidden object slot should update");
    function_compiler
        .state
        .emission
        .lexical_scopes
        .active_scoped_lexical_bindings
        .entry("object".to_string())
        .or_default()
        .push(hidden_object_name);

    let hidden_name_name = function_compiler
        .allocate_named_hidden_local("inline_param_name", StaticValueKind::Unknown);
    function_compiler
        .update_capture_slot_binding_from_expression(
            &hidden_name_name,
            &Expression::String("x".to_string()),
        )
        .expect("hidden name slot should update");
    function_compiler
        .state
        .emission
        .lexical_scopes
        .active_scoped_lexical_bindings
        .entry("name".to_string())
        .or_default()
        .push(hidden_name_name);

    let resolved_object = function_compiler
        .resolve_bound_alias_expression(&Expression::Identifier("object".to_string()))
        .expect("expected hidden object alias resolution");
    if !matches!(
        &resolved_object,
        Expression::Member { object, property }
            if matches!(object.as_ref(), Expression::Identifier(name) if name == "C")
                && matches!(property.as_ref(), Expression::String(name) if name == "prototype")
    ) {
        panic!("resolved object: {resolved_object:?}");
    }

    let Statement::Var { value, .. } = &function_declaration.body[0] else {
        panic!("expected descriptor initializer");
    };
    let descriptor = function_compiler
        .resolve_descriptor_binding_from_expression(value)
        .expect("expected descriptor binding through hidden inline params");
    assert!(descriptor.configurable);
    assert!(!descriptor.enumerable);
    assert!(descriptor.has_get);
    assert!(descriptor.has_set);
}

#[test]
fn preserves_descriptor_return_snapshots_through_extra_local_builtin_alias_calls() {
    let program = frontend::parse(
        r#"
            var __getOwnPropertyDescriptor = Object.getOwnPropertyDescriptor;
            var __getOwnPropertyNames = Object.getOwnPropertyNames;
            function verifyProperty(obj, name, expected) {
              var originalDesc = __getOwnPropertyDescriptor(obj, name);
              var names = __getOwnPropertyNames(expected);
              return originalDesc;
            }
            class C {
              set x(v) { this._x = v; }
            }
            var desc = verifyProperty(C.prototype, "x", { enumerable: false });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        true,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let verify_call = Expression::Call {
        callee: Box::new(Expression::Identifier("verifyProperty".to_string())),
        arguments: vec![
            CallArgument::Expression(Expression::Member {
                object: Box::new(Expression::Identifier("C".to_string())),
                property: Box::new(Expression::String("prototype".to_string())),
            }),
            CallArgument::Expression(Expression::String("x".to_string())),
            CallArgument::Expression(Expression::Object(vec![ObjectEntry::Data {
                key: Expression::String("enumerable".to_string()),
                value: Expression::Bool(false),
            }])),
        ],
    };
    let static_result = function_compiler
        .resolve_static_call_result_expression_with_context(
            match &verify_call {
                Expression::Call { callee, .. } => callee,
                _ => unreachable!(),
            },
            match &verify_call {
                Expression::Call { arguments, .. } => arguments,
                _ => unreachable!(),
            },
            None,
        )
        .map(|(value, _)| value);
    assert!(
        static_result.is_some(),
        "expected static call result for verifyProperty",
    );
    let static_result = static_result.expect("expected static result");
    let descriptor = function_compiler
        .resolve_descriptor_binding_from_expression(&Expression::Identifier("desc".to_string()))
        .expect("expected returned call assignment to preserve descriptor binding");
    assert!(
        !descriptor.has_get,
        "expected returned descriptor for setter-only accessor to omit getter binding; static result was {static_result:#?}",
    );
    assert!(
        descriptor.has_set,
        "expected returned descriptor to preserve setter binding; static result was {static_result:#?}",
    );
    let setter_expression = descriptor
        .setter
        .clone()
        .expect("expected returned descriptor to preserve setter expression");
    if let Expression::Identifier(name) = &setter_expression {
        assert!(
            function_compiler
                .state
                .speculation
                .static_semantics
                .values
                .local_function_bindings
                .contains_key(name)
                || function_compiler
                    .backend
                    .global_semantics
                    .functions
                    .function_bindings
                    .contains_key(name)
                || is_internal_user_function_identifier(name),
            "expected returned descriptor setter identifier to name a function binding; setter={setter_expression:#?}",
        );
        assert!(
            function_compiler
                .resolve_current_local_binding(name)
                .is_none(),
            "expected returned descriptor setter identifier to avoid colliding local value bindings; setter={setter_expression:#?}",
        );
    }
    assert!(
        function_compiler
            .resolve_function_binding_from_expression(&setter_expression)
            .is_some(),
        "expected returned descriptor setter expression to resolve as a function binding; setter={setter_expression:#?}; static result was {static_result:#?}",
    );
}

#[test]
fn resolves_static_descriptor_binding_from_hidden_inline_parameter_locals() {
    let program = frontend::parse(
        r#"
            function f(object, name) {
              var desc = Object.getOwnPropertyDescriptor(object, name);
              console.log(desc.configurable, typeof desc.get, typeof desc.set);
            }
            class C {
              static get staticX() { return this._x; }
              static set staticX(v) { this._x = v; }
            }
            f(C, "staticX");
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);
    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;

    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| function.name == "f")
        .cloned()
        .expect("expected helper function");
    let function_declaration = program
        .functions
        .iter()
        .find(|function| function.name == user_function.name)
        .cloned()
        .expect("expected helper declaration");
    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        true,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    function_compiler
        .emit_statement(&program.statements[0])
        .expect("class statement should emit before helper call");

    let hidden_object_name = function_compiler
        .allocate_named_hidden_local("inline_param_object_static", StaticValueKind::Unknown);
    function_compiler
        .update_capture_slot_binding_from_expression(
            &hidden_object_name,
            &Expression::Identifier("C".to_string()),
        )
        .expect("hidden object slot should update");
    function_compiler
        .state
        .emission
        .lexical_scopes
        .active_scoped_lexical_bindings
        .entry("object".to_string())
        .or_default()
        .push(hidden_object_name);

    let hidden_name_name = function_compiler
        .allocate_named_hidden_local("inline_param_name_static", StaticValueKind::Unknown);
    function_compiler
        .update_capture_slot_binding_from_expression(
            &hidden_name_name,
            &Expression::String("staticX".to_string()),
        )
        .expect("hidden name slot should update");
    function_compiler
        .state
        .emission
        .lexical_scopes
        .active_scoped_lexical_bindings
        .entry("name".to_string())
        .or_default()
        .push(hidden_name_name);

    let Statement::Var { value, .. } = &function_declaration.body[0] else {
        panic!("expected descriptor initializer");
    };
    let descriptor = function_compiler
        .resolve_descriptor_binding_from_expression(value)
        .expect("expected static descriptor binding through hidden inline params");
    assert!(descriptor.configurable);
    assert!(!descriptor.enumerable);
    assert!(descriptor.has_get);
    assert!(descriptor.has_set);
}

#[test]
fn allows_explicit_call_frame_inlining_for_symbol_descriptor_helper_calls() {
    let program = frontend::parse(
        r#"
            function probe(object, name) {
              var getter = Object.getOwnPropertyDescriptor(object, name).get;
              console.log(typeof getter, getter.name);
            }

            var anonSym = Symbol();
            class A {
              get [anonSym]() {}
            }

            probe(A.prototype, anonSym);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);
    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;

    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| function.name == "probe")
        .cloned()
        .expect("expected helper function");

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        true,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");

    assert!(
        function_compiler.can_inline_user_function_call_with_explicit_call_frame(
            &user_function,
            &[
                Expression::Member {
                    object: Box::new(Expression::Identifier("A".to_string())),
                    property: Box::new(Expression::String("prototype".to_string())),
                },
                Expression::Identifier("anonSym".to_string()),
            ],
            &Expression::This,
        )
    );
}

#[test]
fn resolves_descriptor_binding_from_hidden_inline_symbol_parameter_locals() {
    let program = frontend::parse(
        r#"
            function probe(object, name) {
              var getter = Object.getOwnPropertyDescriptor(object, name).get;
              console.log(typeof getter, getter.name);
            }

            var anonSym = Symbol();
            class A {
              get [anonSym]() {}
            }

            probe(A.prototype, anonSym);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);
    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;

    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| function.name == "probe")
        .cloned()
        .expect("expected helper function");
    let function_declaration = program
        .functions
        .iter()
        .find(|function| function.name == user_function.name)
        .cloned()
        .expect("expected helper declaration");
    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        true,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..2] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let hidden_object_name = function_compiler
        .allocate_named_hidden_local("inline_symbol_object", StaticValueKind::Unknown);
    function_compiler
        .update_capture_slot_binding_from_expression(
            &hidden_object_name,
            &Expression::Member {
                object: Box::new(Expression::Identifier("A".to_string())),
                property: Box::new(Expression::String("prototype".to_string())),
            },
        )
        .expect("hidden object slot should update");
    function_compiler
        .state
        .emission
        .lexical_scopes
        .active_scoped_lexical_bindings
        .entry("object".to_string())
        .or_default()
        .push(hidden_object_name);

    let hidden_name_name = function_compiler
        .allocate_named_hidden_local("inline_symbol_name", StaticValueKind::Unknown);
    function_compiler
        .update_capture_slot_binding_from_expression(
            &hidden_name_name,
            &Expression::Identifier("anonSym".to_string()),
        )
        .expect("hidden symbol slot should update");
    function_compiler
        .state
        .emission
        .lexical_scopes
        .active_scoped_lexical_bindings
        .entry("name".to_string())
        .or_default()
        .push(hidden_name_name);

    let Statement::Var { value, .. } = &function_declaration.body[0] else {
        panic!("expected getter initializer");
    };

    assert!(matches!(
        function_compiler.resolve_symbol_identity_expression(&Expression::Identifier("name".to_string())),
        Some(Expression::Identifier(name)) if name == "anonSym"
    ));
    assert!(matches!(
        function_compiler.resolve_property_key_expression(&Expression::Identifier("name".to_string())),
        Some(Expression::Identifier(name)) if name == "anonSym"
    ));

    let getter_value = function_compiler.materialize_static_expression(value);
    let getter_binding = function_compiler
        .resolve_function_binding_from_expression(&getter_value)
        .expect("expected hidden inline symbol descriptor getter binding");
    assert!(matches!(getter_binding, LocalFunctionBinding::User(_)));
}

#[test]
fn propagates_custom_iterator_member_bindings_through_symbol_iterator_calls() {
    let program = frontend::parse(
        r#"
            var iterable = {};
            iterable[Symbol.iterator] = function() {
              return {
                next: function() { return { value: 9, done: false }; },
                return: function() { return {}; }
              };
            };
            var iter = iterable[Symbol.iterator]();
            var step = iter.next();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    let iterator_property = Expression::Member {
        object: Box::new(Expression::Identifier("Symbol".to_string())),
        property: Box::new(Expression::String("iterator".to_string())),
    };
    let getter_binding = function_compiler
        .resolve_member_getter_binding(
            &Expression::Identifier("obj".to_string()),
            &iterator_property,
        )
        .expect("expected sync iterator getter binding");
    assert!(matches!(
        function_compiler.resolve_static_function_outcome_from_binding_with_context(
            &getter_binding,
            &[],
            None,
        ),
        Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(Expression::Identifier(name))))
            if name == "reason"
    ));
    let iter_initializer = program
        .statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Var { name, value } if name == "iter" => Some(value.clone()),
            _ => None,
        })
        .expect("expected iter initializer");
    assert!(
        function_compiler
            .resolve_simple_generator_source(&iter_initializer)
            .is_some(),
        "expected iter initializer to resolve as simple generator source",
    );

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
        eprintln!("stmt: {statement:?}");
        eprintln!(
            "static getter={:?} setter={:?}",
            function_compiler.resolve_member_getter_binding(
                &Expression::Identifier("C".to_string()),
                &Expression::Number(2.0),
            ),
            function_compiler.resolve_member_setter_binding(
                &Expression::Identifier("C".to_string()),
                &Expression::Number(2.0),
            )
        );
        eprintln!(
            "instance getter={:?} setter={:?}",
            function_compiler.resolve_member_getter_binding(
                &Expression::New {
                    callee: Box::new(Expression::Identifier("C".to_string())),
                    arguments: Vec::new(),
                },
                &Expression::Number(2.0),
            ),
            function_compiler.resolve_member_setter_binding(
                &Expression::New {
                    callee: Box::new(Expression::Identifier("C".to_string())),
                    arguments: Vec::new(),
                },
                &Expression::Number(2.0),
            )
        );
    }

    let Statement::Var {
        value: iter_call, ..
    } = &program.statements[2]
    else {
        panic!("expected iter initializer");
    };
    let Expression::Call { callee, .. } = iter_call else {
        panic!("expected iter call initializer");
    };
    assert!(
        function_compiler
            .resolve_user_function_from_expression(callee)
            .is_some(),
        "expected iterable[Symbol.iterator] function binding",
    );
    assert!(
        function_compiler
            .inherited_member_function_bindings(iter_call)
            .iter()
            .any(|binding| binding.property == "next"),
        "expected iterator call to expose returned next binding",
    );

    assert!(
        function_compiler
            .resolve_function_binding_from_expression(&Expression::Member {
                object: Box::new(Expression::Identifier("iter".to_string())),
                property: Box::new(Expression::String("next".to_string())),
            })
            .is_some(),
        "expected iter.next member binding",
    );

    let step_binding = function_compiler
        .resolve_object_binding_from_expression(&Expression::Identifier("step".to_string()))
        .expect("expected step object binding");
    assert_eq!(
        object_binding_lookup_value(&step_binding, &Expression::String("value".to_string())),
        Some(&Expression::Number(9.0))
    );
    assert_eq!(
        object_binding_lookup_value(&step_binding, &Expression::String("done".to_string())),
        Some(&Expression::Bool(false))
    );
}

#[test]
fn propagates_getter_capture_slots_through_returned_call_results() {
    let program = frontend::parse(
        r#"
            var obj = {
              make() {
                var nextCount = 0;
                return {
                  get next() {
                    return function() {
                      nextCount++;
                      return nextCount;
                    };
                  }
                };
              }
            };
            var iter = obj.make();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);
    assert!(
        compiler
            .state
            .function_registry
            .analysis
            .user_function_capture_bindings
            .values()
            .any(|bindings| bindings.contains_key("nextCount")),
        "expected nested getter-returned closure to register nextCount as a capture",
    );

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let Statement::Var {
        value: iter_call, ..
    } = &program.statements[1]
    else {
        panic!("expected iter initializer");
    };
    assert!(
        !function_compiler
            .inherited_member_getter_bindings(iter_call)
            .is_empty(),
        "expected iter initializer call to expose returned getter bindings",
    );
    let helper_capture_slots = function_compiler
        .initialize_returned_member_capture_slots_for_bindings(
            "iter",
            iter_call,
            0,
            &function_compiler.inherited_member_getter_bindings(iter_call),
        )
        .expect("returned getter capture slots should initialize");
    assert!(
        helper_capture_slots.contains_key("next"),
        "expected returned getter capture slot helper to populate next",
    );

    let getter_binding = function_compiler.resolve_member_getter_binding(
        &Expression::Identifier("iter".to_string()),
        &Expression::String("next".to_string()),
    );
    assert!(
        getter_binding.is_some(),
        "expected iter.next getter binding"
    );

    let capture_slots = function_compiler
        .resolve_member_function_capture_slots(
            &Expression::Identifier("iter".to_string()),
            &Expression::String("next".to_string()),
        )
        .expect("expected iter.next getter capture slots");
    assert!(
        capture_slots.contains_key("nextCount"),
        "expected getter capture slots to bind nextCount"
    );

    let (returned_user_function, returned_capture_slots) = function_compiler
        .resolve_member_getter_returned_user_function(
            &Expression::Identifier("iter".to_string()),
            &Expression::String("next".to_string()),
        )
        .expect("expected iter.next getter to resolve returned user function");
    assert!(
        returned_capture_slots.contains_key("nextCount"),
        "expected returned getter call to preserve nextCount capture slots",
    );
    let prepared_bound_captures = function_compiler
        .prepare_bound_user_function_capture_bindings(
            &returned_user_function,
            &returned_capture_slots,
        )
        .expect("returned getter call should prepare bound capture bindings");
    assert!(
        !prepared_bound_captures.is_empty(),
        "expected returned getter call to prepare at least one bound capture binding",
    );
}

#[test]
fn preserves_specialized_values_for_getter_returned_closure_aliases() {
    let program = frontend::parse(
        r#"
            var obj = {
              make() {
                var nextCount = 0;
                return {
                  get next() {
                    return function() {
                      nextCount++;
                      return nextCount;
                    };
                  }
                };
              }
            };
            var iter = obj.make();
            var n = iter.next;
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);
    assert!(
        compiler
            .state
            .function_registry
            .analysis
            .user_function_capture_bindings
            .values()
            .any(|bindings| bindings.contains_key("nextCount")),
        "expected nested getter-returned closure to register nextCount as a capture",
    );

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    assert!(
        function_compiler
            .resolve_specialized_function_value_from_expression(&Expression::Identifier(
                "n".to_string()
            ))
            .is_some(),
        "expected getter-returned closure alias to keep specialized function metadata",
    );
    let specialized = function_compiler
        .resolve_specialized_function_value_from_expression(&Expression::Identifier(
            "n".to_string(),
        ))
        .expect("expected getter-returned closure alias specialized value");
    let mut referenced_names = HashSet::new();
    for effect in &specialized.summary.effects {
        match effect {
            crate::backend::direct_wasm::InlineFunctionEffect::Assign { value, .. } => {
                collect_referenced_binding_names_from_expression(value, &mut referenced_names);
            }
            crate::backend::direct_wasm::InlineFunctionEffect::Update { name, .. } => {
                referenced_names.insert(name.clone());
            }
            crate::backend::direct_wasm::InlineFunctionEffect::Expression(expression) => {
                collect_referenced_binding_names_from_expression(expression, &mut referenced_names);
            }
        }
    }
    if let Some(return_value) = specialized.summary.return_value.as_ref() {
        collect_referenced_binding_names_from_expression(return_value, &mut referenced_names);
    }
    assert!(
        !referenced_names.contains("nextCount"),
        "expected getter-returned closure specialization to bind nextCount through a capture slot",
    );
}

#[test]
fn does_not_fold_effectful_getter_returned_closure_alias_calls_to_static_numbers() {
    let program = frontend::parse(
        r#"
            var obj = {
              make() {
                var nextCount = 0;
                return {
                  get next() {
                    return function() {
                      nextCount++;
                      return nextCount;
                    };
                  }
                };
              }
            };
            var iter = obj.make();
            var n = iter.next;
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let specialized = function_compiler
        .resolve_specialized_function_value_from_expression(&Expression::Identifier(
            "n".to_string(),
        ))
        .expect("expected getter-returned closure alias specialized value");
    let update_name = specialized
        .summary
        .effects
        .iter()
        .find_map(|effect| match effect {
            crate::backend::direct_wasm::InlineFunctionEffect::Update { name, .. } => {
                Some(name.clone())
            }
            _ => None,
        })
        .expect("expected specialized summary update effect");
    let Expression::Identifier(return_name) = specialized
        .summary
        .return_value
        .clone()
        .expect("expected specialized summary return value")
    else {
        panic!("expected specialized summary return identifier");
    };
    assert_eq!(update_name, return_name);
    assert_ne!(return_name, "nextCount");

    assert_eq!(
        function_compiler.resolve_static_number_value(&Expression::Call {
            callee: Box::new(Expression::Identifier("n".to_string())),
            arguments: Vec::new(),
        }),
        None
    );
    assert_eq!(
        function_compiler.resolve_static_primitive_expression_with_context(
            &Expression::Call {
                callee: Box::new(Expression::Identifier("n".to_string())),
                arguments: Vec::new(),
            },
            None,
        ),
        None
    );
}

#[test]
fn nested_getter_returned_function_resolves_capture_hidden_name() {
    let program = frontend::parse(
        r#"
            var obj = {
              make() {
                var nextCount = 0;
                return {
                  get next() {
                    return function() {
                      nextCount++;
                      return nextCount;
                    };
                  }
                };
              }
            };
            var iter = obj.make();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let returned_function_name = compiler
        .state
        .function_registry
        .analysis
        .user_function_capture_bindings
        .iter()
        .find_map(|(function_name, bindings)| {
            bindings
                .contains_key("nextCount")
                .then_some(function_name.clone())
        })
        .expect("expected nested returned function capture bindings");
    let returned_user_function = compiler
        .state
        .function_registry
        .catalog
        .user_function_map
        .get(&returned_function_name)
        .cloned()
        .expect("expected nested returned user function");

    let nested_function_compiler = FunctionCompiler::new(
        &mut compiler,
        Some(&returned_user_function),
        true,
        false,
        returned_user_function.strict,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("nested function compiler should initialize");

    assert_eq!(
        nested_function_compiler.resolve_user_function_capture_hidden_name("nextCount"),
        Some(format!(
            "__ayy_capture_binding__{}__nextCount",
            returned_user_function.name
        ))
    );
}

#[test]
fn getter_returned_closure_alias_keeps_capture_slots_for_function_prototype_call() {
    let program = frontend::parse(
        r#"
            var obj = {
              make() {
                var nextCount = 0;
                return {
                  get next() {
                    return function(value) {
                      nextCount++;
                      return value;
                    };
                  }
                };
              }
            };
            var iter = obj.make();
            var n = iter.next;
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let iter_next_capture_slots = function_compiler.resolve_member_function_capture_slots(
        &Expression::Identifier("iter".to_string()),
        &Expression::String("next".to_string()),
    );
    assert!(
        iter_next_capture_slots.is_some(),
        "expected iter.next to retain member capture slots before alias resolution",
    );
    assert!(
        function_compiler.binding_name_is_global("iter"),
        "expected iter binding to be treated as global at top level",
    );
    let iter_next_key = function_compiler
        .member_function_binding_key(
            &Expression::Identifier("iter".to_string()),
            &Expression::String("next".to_string()),
        )
        .expect("expected iter.next binding key");
    assert!(
        function_compiler
            .backend
            .global_semantics
            .members
            .member_function_capture_slots
            .contains_key(&iter_next_key),
        "expected iter.next capture slots to be mirrored in global metadata for alias materialization",
    );

    let resolved_alias = function_compiler
        .resolve_bound_alias_expression(&Expression::Identifier("n".to_string()))
        .expect("expected alias resolution");
    assert!(
        matches!(resolved_alias, Expression::Member { .. }),
        "expected n to remain aliased to iter.next for call/apply capture recovery, got {resolved_alias:?}",
    );

    let capture_slots = function_compiler
        .resolve_function_expression_capture_slots(&Expression::Identifier("n".to_string()))
        .expect("expected n.call(...) to recover bound capture slots");
    assert!(
        capture_slots.contains_key("nextCount"),
        "expected alias function expression capture slots to include nextCount",
    );
    let user_function = function_compiler
        .resolve_user_function_from_expression(&Expression::Identifier("n".to_string()))
        .expect("expected n to resolve to a user function")
        .clone();
    let prepared_bound_captures = function_compiler
        .prepare_bound_user_function_capture_bindings(&user_function, &capture_slots)
        .expect("expected n.call(...) to prepare bound capture globals");
    assert!(
        !prepared_bound_captures.is_empty(),
        "expected n.call(...) to prepare at least one bound capture binding",
    );
}

#[test]
fn resolves_bound_snapshot_result_for_getter_returned_closure_call_with_this_and_arguments() {
    let program = frontend::parse(
        r#"
            var obj = {
              make() {
                var nextCount = 0;
                return {
                  name: "syncIterator",
                  get next() {
                    return function(v) {
                      nextCount++;
                      return { count: nextCount, thisName: this && this.name, arg: v };
                    };
                  }
                };
              }
            };
            var iter = obj.make();
            var n = iter.next;
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let user_function = function_compiler
        .resolve_user_function_from_expression(&Expression::Identifier("n".to_string()))
        .expect("expected n to resolve to a user function")
        .clone();
    let capture_slots = function_compiler
        .resolve_function_expression_capture_slots(&Expression::Identifier("n".to_string()))
        .expect("expected n.call(...) to recover bound capture slots");
    let capture_snapshot = capture_slots
        .iter()
        .map(|(capture_name, slot_name)| {
            (
                capture_name.clone(),
                function_compiler.snapshot_bound_capture_slot_expression(slot_name),
            )
        })
        .collect::<HashMap<_, _>>();
    let (result, _) = function_compiler
        .resolve_bound_snapshot_user_function_result_with_arguments_and_this(
            &user_function.name,
            &capture_snapshot,
            &[Expression::String("a".to_string())],
            &Expression::Identifier("iter".to_string()),
        )
        .expect("expected bound snapshot result");
    let object_binding = function_compiler
        .resolve_object_binding_from_expression(&result)
        .expect("expected bound snapshot result object binding");

    assert_eq!(
        object_binding_lookup_value(&object_binding, &Expression::String("count".to_string())),
        Some(&Expression::Number(1.0)),
    );
    assert_eq!(
        object_binding_lookup_value(&object_binding, &Expression::String("thisName".to_string())),
        Some(&Expression::String("syncIterator".to_string())),
    );
    assert_eq!(
        object_binding_lookup_value(&object_binding, &Expression::String("arg".to_string())),
        Some(&Expression::String("a".to_string())),
    );
}

#[test]
fn does_not_infer_object_parameter_bindings_for_undefined_member_argument_values() {
    let program = frontend::parse(
        r#"
            function sameValue(left, right) {
              return left === right;
            }
            var log = [];
            log[7] = { args: [undefined] };
            var x = log[7].args[0];
            sameValue(x, undefined);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);

    let same_value_function = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| function.name == "sameValue")
        .expect("expected sameValue user function");

    let same_value_parameter_values = parameter_value_bindings
        .get(&same_value_function.name)
        .expect("expected sameValue parameter value bindings");
    let same_value_parameter_objects = parameter_object_bindings
        .get(&same_value_function.name)
        .expect("expected sameValue parameter object bindings");
    assert!(matches!(
        same_value_parameter_objects.get("left"),
        Some(None)
    ));

    let function_parameter_bindings = parameter_bindings
        .get(&same_value_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_array_bindings = parameter_array_bindings
        .get(&same_value_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_object_bindings = parameter_object_bindings
        .get(&same_value_function.name)
        .cloned()
        .unwrap_or_default();
    let same_value_function = same_value_function.clone();

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        Some(&same_value_function),
        false,
        false,
        same_value_function.strict,
        &function_parameter_bindings,
        same_value_parameter_values,
        &function_parameter_array_bindings,
        &function_parameter_object_bindings,
    )
    .expect("function compiler should initialize");

    assert!(!matches!(
        function_compiler.state.speculation.static_semantics.values.local_value_bindings.get("left"),
        Some(Expression::Identifier(name)) if name == "x"
    ));
}

#[test]
fn preserves_unknown_parameter_object_binding_after_stateful_callback_object_call() {
    let program = frontend::parse(
        r#"
            function sameValue(left, right) {
              return left === right;
            }

            sameValue("a", "a");
            Promise.resolve({ marker: 1 }).then(function(v) {
              sameValue(v, undefined);
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let (_, _, _, parameter_object_bindings) =
        compiler.collect_user_function_parameter_bindings(&program);
    let same_value_parameter_objects = parameter_object_bindings
        .get("sameValue")
        .expect("expected sameValue parameter object bindings");
    assert!(matches!(
        same_value_parameter_objects.get("left"),
        Some(None)
    ));
}

#[test]
fn does_not_infer_object_parameter_bindings_for_direct_undefined_member_argument_values() {
    let program = frontend::parse(
        r#"
            function sameValue(left, right) {
              return left === right;
            }
            var log = [];
            log[7] = { args: [undefined] };
            sameValue(log[7].args[0], undefined);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let (_, parameter_value_bindings, _, parameter_object_bindings) =
        compiler.collect_user_function_parameter_bindings(&program);
    let same_value_parameter_values = parameter_value_bindings
        .get("sameValue")
        .expect("expected sameValue parameter value bindings");
    let same_value_parameter_objects = parameter_object_bindings
        .get("sameValue")
        .expect("expected sameValue parameter object bindings");
    assert_eq!(
        same_value_parameter_values.get("left"),
        Some(&Some(Expression::Undefined))
    );
    assert!(matches!(
        same_value_parameter_objects.get("left"),
        Some(None)
    ));
}

#[test]
fn does_not_infer_object_parameter_bindings_for_callback_undefined_member_argument_values() {
    let program = frontend::parse(
        r#"
            function sameValue(left, right) {
              return left === right;
            }
            var log = [];
            log[7] = { args: [undefined] };
            Promise.resolve().then(function() {
              sameValue(log[7].args[0], undefined);
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let (_, parameter_value_bindings, _, parameter_object_bindings) =
        compiler.collect_user_function_parameter_bindings(&program);
    let same_value_parameter_values = parameter_value_bindings
        .get("sameValue")
        .expect("expected sameValue parameter value bindings");
    let same_value_parameter_objects = parameter_object_bindings
        .get("sameValue")
        .expect("expected sameValue parameter object bindings");
    assert_eq!(
        same_value_parameter_values.get("left"),
        Some(&Some(Expression::Undefined))
    );
    assert!(matches!(
        same_value_parameter_objects.get("left"),
        Some(None)
    ));
}

#[test]
fn does_not_infer_object_parameter_bindings_for_nested_callback_undefined_member_argument_values() {
    let program = frontend::parse(
        r#"
            function sameValue(left, right) {
              return left === right;
            }
            var log = [];
            log[7] = { args: [undefined] };
            Promise.resolve().then(function() {
              Promise.resolve().then(function() {
                sameValue(log[7].args[0], undefined);
              });
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let (_, parameter_value_bindings, _, parameter_object_bindings) =
        compiler.collect_user_function_parameter_bindings(&program);
    let same_value_parameter_values = parameter_value_bindings
        .get("sameValue")
        .expect("expected sameValue parameter value bindings");
    let same_value_parameter_objects = parameter_object_bindings
        .get("sameValue")
        .expect("expected sameValue parameter object bindings");
    assert_eq!(
        same_value_parameter_values.get("left"),
        Some(&Some(Expression::Undefined))
    );
    assert!(matches!(
        same_value_parameter_objects.get("left"),
        Some(None)
    ));
}

#[test]
fn preserves_unknown_parameter_value_bindings_for_sync_yield_star_return_samevalue_chain() {
    let program = frontend::parse(
        r#"
            function sameValue(left, right, message) {
              if (left === right) {
                return;
              }
              throw new Error(message);
            }

            var log = [];
            var obj = {
              [Symbol.iterator]() {
                var returnCount = 0;
                return {
                  get next() {
                    return function() {
                      return { value: "next-value-1", done: false };
                    };
                  },
                  get return() {
                    return function() {
                      log.push({ args: [...arguments] });
                      returnCount++;
                      if (returnCount === 1) {
                        return {
                          get value() { return "return-value-1"; },
                          get done() { return false; }
                        };
                      }
                      return {
                        get value() { return "return-value-2"; },
                        get done() { return true; }
                      };
                    };
                  }
                };
              }
            };

            class C {
              async *gen() {
                yield* obj;
              }
            }

            var iter = C.prototype.gen();
            iter.next().then(function(v) {
              sameValue(v.value, "next-value-1", "next value");
              sameValue(v.done, false, "next done");
              iter.return("return-arg-1").then(function(v2) {
                sameValue(v2.value, "return-value-1", "return1 value");
                sameValue(v2.done, false, "return1 done");
                iter.return().then(function(v3) {
                  sameValue(log[0].args[0], undefined, "return args[0]");
                });
              });
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let (_, parameter_value_bindings, _, parameter_object_bindings) =
        compiler.collect_user_function_parameter_bindings(&program);
    let same_value_parameter_values = parameter_value_bindings
        .get("sameValue")
        .expect("expected sameValue parameter value bindings");
    let same_value_parameter_objects = parameter_object_bindings
        .get("sameValue")
        .expect("expected sameValue parameter object bindings");
    assert!(matches!(
        same_value_parameter_values.get("left"),
        Some(None)
    ));
    assert!(matches!(
        same_value_parameter_objects.get("left"),
        Some(None)
    ));
}

#[test]
fn preserves_unknown_parameter_value_bindings_for_printed_samevalue_calls() {
    let program = frontend::parse(
        r#"
            function sameValue(left, right) {
              if (left === right) {
                return left !== 0 || 1 / left === 1 / right;
              }
              return left !== left && right !== right;
            }

            console.log(sameValue(true, true));
            console.log(sameValue(false, false));
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let (_, parameter_value_bindings, _, _) =
        compiler.collect_user_function_parameter_bindings(&program);
    let same_value_parameter_values = parameter_value_bindings
        .get("sameValue")
        .expect("expected sameValue parameter value bindings");

    assert!(matches!(
        same_value_parameter_values.get("left"),
        Some(None)
    ));
    assert!(matches!(
        same_value_parameter_values.get("right"),
        Some(None)
    ));
}

#[test]
fn resolves_static_same_value_for_constructor_prototype_aliases() {
    let program = frontend::parse(
        r#"
            class C {}
            class D extends C {}
            var ctor = Object.getPrototypeOf(D);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        true,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");

    let computed_ctor_proto = Expression::Call {
        callee: Box::new(Expression::Member {
            object: Box::new(Expression::Identifier("Object".to_string())),
            property: Box::new(Expression::String("getPrototypeOf".to_string())),
        }),
        arguments: vec![CallArgument::Expression(Expression::Identifier(
            "D".to_string(),
        ))],
    };

    assert_eq!(
        function_compiler.resolve_static_same_value_result_with_context(
            &Expression::Identifier("ctor".to_string()),
            &Expression::Identifier("C".to_string()),
            None,
        ),
        Some(true)
    );
    assert_eq!(
        function_compiler.resolve_static_same_value_result_with_context(
            &computed_ctor_proto,
            &Expression::Identifier("C".to_string()),
            None,
        ),
        Some(true)
    );
}

#[test]
fn derived_super_call_updates_current_this_object_binding() {
    let program = frontend::parse(
        r#"
            class Base {
              constructor(a, b) {
                var o = new Object();
                o.prp = a + b;
                return o;
              }
            }
            class Subclass extends Base {
              constructor(a, b) {
                super(a, b);
                return this;
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| {
            compiler
                .state
                .function_registry
                .catalog
                .registered_function_declarations
                .iter()
                .find(|declaration| declaration.name == function.name)
                .is_some_and(|declaration| {
                    declaration.derived_constructor
                        && declaration.self_binding.as_deref() == Some("Subclass")
                })
        })
        .cloned()
        .expect("expected derived constructor user function");
    let function = compiler
        .state
        .function_registry
        .catalog
        .registered_function_declarations
        .iter()
        .find(|function| function.name == user_function.name)
        .cloned()
        .expect("expected derived constructor declaration");

    let parameter_value_bindings = HashMap::from([
        ("a".to_string(), Some(Expression::Number(2.0))),
        ("b".to_string(), Some(Expression::Number(-1.0))),
    ]);
    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        Some(&user_function),
        true,
        false,
        true,
        &HashMap::new(),
        &parameter_value_bindings,
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");

    for statement in &function.body {
        function_compiler
            .emit_statement(statement)
            .expect("derived constructor statement should emit");
    }

    let this_binding = function_compiler
        .state
        .speculation
        .static_semantics
        .objects
        .local_object_bindings
        .get("this")
        .cloned()
        .expect("expected current this object binding after super()");
    assert_eq!(
        object_binding_lookup_value(&this_binding, &Expression::String("prp".to_string())),
        Some(&Expression::Number(1.0)),
    );
}

#[test]
fn resolves_new_object_binding_for_constructor_returning_replacement_object() {
    let program = frontend::parse(
        r#"
            class Base {
              constructor(a, b) {
                var o = new Object();
                o.prp = a + b;
                return o;
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let object_binding = function_compiler
        .resolve_user_constructor_object_binding_from_new(
            &Expression::Identifier("Base".to_string()),
            &[
                CallArgument::Expression(Expression::Number(2.0)),
                CallArgument::Expression(Expression::Number(-1.0)),
            ],
        )
        .expect("expected replacement object binding");
    assert_eq!(
        object_binding_lookup_value(&object_binding, &Expression::String("prp".to_string())),
        Some(&Expression::Number(1.0)),
    );
}

#[test]
fn resolves_new_object_binding_for_derived_constructor_returning_replacement_object() {
    let program = frontend::parse(
        r#"
            class Base {
              constructor(a, b) {
                var o = new Object();
                o.prp = a + b;
                return o;
              }
            }

            class Subclass extends Base {
              constructor(a, b) {
                super(a, b);
                return this;
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let object_binding = function_compiler
        .resolve_user_constructor_object_binding_from_new(
            &Expression::Identifier("Subclass".to_string()),
            &[
                CallArgument::Expression(Expression::Number(2.0)),
                CallArgument::Expression(Expression::Number(-1.0)),
            ],
        )
        .expect("expected replacement object binding from derived constructor");
    assert_eq!(
        object_binding_lookup_value(&object_binding, &Expression::String("prp".to_string())),
        Some(&Expression::Number(1.0)),
    );
}

#[test]
fn resolves_static_same_value_for_derived_constructor_replacement_object_property() {
    let program = frontend::parse(
        r#"
            class Base {
              constructor(a, b) {
                var o = new Object();
                o.prp = a + b;
                return o;
              }
            }

            class Subclass extends Base {
              constructor(a, b) {
                super(a, b);
                return this;
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| {
            compiler
                .state
                .function_registry
                .catalog
                .registered_function_declarations
                .iter()
                .find(|declaration| declaration.name == function.name)
                .is_some_and(|declaration| {
                    declaration.derived_constructor
                        && declaration.self_binding.as_deref() == Some("Subclass")
                })
        })
        .cloned()
        .expect("expected derived constructor user function");
    let function = compiler
        .state
        .function_registry
        .catalog
        .registered_function_declarations
        .iter()
        .find(|function| function.name == user_function.name)
        .cloned()
        .expect("expected derived constructor declaration");

    let parameter_value_bindings = HashMap::from([
        ("a".to_string(), Some(Expression::Number(2.0))),
        ("b".to_string(), Some(Expression::Number(-1.0))),
    ]);
    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        Some(&user_function),
        true,
        false,
        true,
        &HashMap::new(),
        &parameter_value_bindings,
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");

    for statement in &function.body {
        function_compiler
            .emit_statement(statement)
            .expect("derived constructor statement should emit");
    }

    let actual = Expression::Member {
        object: Box::new(Expression::This),
        property: Box::new(Expression::String("prp".to_string())),
    };
    let expected = Expression::Binary {
        op: crate::ir::hir::BinaryOp::Add,
        left: Box::new(Expression::Identifier("a".to_string())),
        right: Box::new(Expression::Identifier("b".to_string())),
    };
    assert_eq!(
        function_compiler.resolve_static_same_value_result_with_context(
            &actual,
            &expected,
            Some(&user_function.name),
        ),
        Some(true),
    );
}

#[test]
fn preserves_derived_constructor_this_binding_after_pre_super_reference_error() {
    let program = frontend::parse(
        r#"
            class Base {
              constructor(a, b) {
                var o = new Object();
                o.prp = a + b;
                return o;
              }
            }

            class Subclass extends Base {
              constructor(a, b) {
                var exn;
                try {
                  this.prp1 = 3;
                } catch (e) {
                  exn = e;
                }
                super(a, b);
                return this;
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| {
            compiler
                .state
                .function_registry
                .catalog
                .registered_function_declarations
                .iter()
                .find(|declaration| declaration.name == function.name)
                .is_some_and(|declaration| {
                    declaration.derived_constructor
                        && declaration.self_binding.as_deref() == Some("Subclass")
                })
        })
        .cloned()
        .expect("expected derived constructor user function");
    let function = compiler
        .state
        .function_registry
        .catalog
        .registered_function_declarations
        .iter()
        .find(|function| function.name == user_function.name)
        .cloned()
        .expect("expected derived constructor declaration");

    let parameter_value_bindings = HashMap::from([
        ("a".to_string(), Some(Expression::Number(2.0))),
        ("b".to_string(), Some(Expression::Number(-1.0))),
    ]);
    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        Some(&user_function),
        true,
        false,
        true,
        &HashMap::new(),
        &parameter_value_bindings,
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");

    for statement in &function.body {
        function_compiler
            .emit_statement(statement)
            .expect("derived constructor statement should emit");
    }

    let this_binding = function_compiler
        .resolve_object_binding_from_expression(&Expression::This)
        .expect("expected this object binding after caught pre-super access");
    assert_eq!(
        object_binding_lookup_value(&this_binding, &Expression::String("prp".to_string())),
        Some(&Expression::Number(1.0)),
    );
    assert_eq!(
        object_binding_lookup_value(&this_binding, &Expression::String("prp1".to_string())),
        None,
    );
    let actual = Expression::Member {
        object: Box::new(Expression::This),
        property: Box::new(Expression::String("prp".to_string())),
    };
    let expected = Expression::Binary {
        op: crate::ir::hir::BinaryOp::Add,
        left: Box::new(Expression::Identifier("a".to_string())),
        right: Box::new(Expression::Identifier("b".to_string())),
    };
    assert_eq!(
        function_compiler.resolve_static_same_value_result_with_context(
            &actual,
            &expected,
            Some(&user_function.name),
        ),
        Some(true),
    );
}

#[test]
fn resolves_new_object_binding_for_derived_constructor_after_caught_pre_super_reference_error() {
    let program = frontend::parse(
        r#"
            class Base {
              constructor(a, b) {
                var o = new Object();
                o.prp = a + b;
                return o;
              }
            }

            class Subclass extends Base {
              constructor(a, b) {
                var exn;
                try {
                  this.prp1 = 3;
                } catch (e) {
                  exn = e;
                }
                super(a, b);
                return this;
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let object_binding = function_compiler
        .resolve_user_constructor_object_binding_from_new(
            &Expression::Identifier("Subclass".to_string()),
            &[
                CallArgument::Expression(Expression::Number(2.0)),
                CallArgument::Expression(Expression::Number(-1.0)),
            ],
        )
        .expect("expected derived constructor object binding after caught pre-super error");
    assert_eq!(
        object_binding_lookup_value(&object_binding, &Expression::String("prp".to_string())),
        Some(&Expression::Number(1.0)),
    );
    assert_eq!(
        object_binding_lookup_value(&object_binding, &Expression::String("prp1".to_string())),
        None,
    );
}

#[test]
fn resolves_default_arrow_function_name_through_conditional_destructuring_binding() {
    let program = frontend::parse(
        r#"
            var callCount = 0;
            var f;
            f = ([arrow = () => {}]) => {
              console.log("name", arrow.name);
              callCount += 1;
            };

            f([]);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;
    compiler.register_user_function_capture_bindings(&program.functions);
    compiler
        .reserve_function_constructor_implicit_global_bindings(&program)
        .expect("implicit globals should reserve");
    compiler.reserve_global_runtime_prototype_binding_globals();

    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| internal_function_name_hint(&function.name) == Some("f"))
        .cloned()
        .expect("expected lowered arrow function");
    let function_declaration = program
        .functions
        .iter()
        .find(|function| function.name == user_function.name)
        .cloned()
        .expect("expected lowered function declaration");
    let function_parameter_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_value_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_array_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_object_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        Some(&user_function),
        false,
        false,
        user_function.strict,
        &function_parameter_bindings,
        &function_parameter_value_bindings,
        &function_parameter_array_bindings,
        &function_parameter_object_bindings,
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&function_declaration.body)
        .expect("bindings should register");

    for statement in &function_declaration.body[..6] {
        function_compiler
            .emit_statement(statement)
            .expect("destructuring setup should emit");
    }

    let member_expression = Expression::Member {
        object: Box::new(Expression::Identifier("arrow".to_string())),
        property: Box::new(Expression::String("name".to_string())),
    };
    assert_eq!(
        function_compiler.resolve_function_name_value(
            &Expression::Identifier("arrow".to_string()),
            &Expression::String("name".to_string())
        ),
        Some("arrow".to_string())
    );
    assert_eq!(
        function_compiler.resolve_static_string_value(&member_expression),
        Some("arrow".to_string())
    );
}

#[test]
fn simple_generator_analysis_keeps_post_yield_effects_in_completion_only() {
    let program = frontend::parse(
        r#"
            let first = 0;
            let second = 0;
            function* g() {
              first += 1;
              yield;
              second += 1;
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let function = compiler
        .state
        .function_registry
        .catalog
        .user_function_map
        .get("g")
        .expect("expected lowered generator")
        .clone();
    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        true,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    let (steps, completion_effects, completion_value) = function_compiler
        .resolve_simple_generator_source(&Expression::Call {
            callee: Box::new(Expression::Identifier(function.name)),
            arguments: Vec::new(),
        })
        .expect("expected simple generator source");

    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].effects.len(), 1);
    assert!(matches!(
        steps[0].outcome,
        SimpleGeneratorStepOutcome::Yield(Expression::Undefined)
    ));
    assert_eq!(completion_effects.len(), 1);
    assert!(matches!(completion_value, Expression::Undefined));
}

#[test]
fn resolves_async_generator_yield_delegate_non_callable_sync_iterator_as_throw_step() {
    let program = frontend::parse(
        r#"
            var obj = {
              [Symbol.iterator]: {}
            };

            class C {
              static async *gen() {
                yield* obj;
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    let (steps, completion_effects) = function_compiler
        .resolve_simple_yield_delegate_source(&Expression::Identifier("obj".to_string()), true)
        .expect("expected sync iterator delegate source");

    assert_eq!(steps.len(), 1);
    assert!(completion_effects.is_empty());
    assert!(matches!(
        &steps[0].outcome,
        SimpleGeneratorStepOutcome::Throw(Expression::Call { callee, arguments })
            if matches!(callee.as_ref(), Expression::Identifier(name) if name == "TypeError")
                && arguments.is_empty()
    ));
}

#[test]
fn resolves_async_generator_yield_delegate_async_iterator_non_callable_then_as_yield_step() {
    let program = frontend::parse(
        r#"
            var obj = {
              [Symbol.asyncIterator]() {
                return {
                  next() {
                    return {
                      then: true,
                      value: 42,
                      done: false,
                    };
                  }
                };
              }
            };

            class C {
              static async *gen() {
                yield* obj;
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    let (steps, completion_effects) = function_compiler
        .resolve_simple_yield_delegate_source(&Expression::Identifier("obj".to_string()), true)
        .expect("expected async iterator delegate source");

    assert_eq!(steps.len(), 1);
    assert!(completion_effects.is_empty());
    match &steps[0].outcome {
        SimpleGeneratorStepOutcome::Yield(Expression::Number(value))
            if (*value - 42.0).abs() < f64::EPSILON => {}
        SimpleGeneratorStepOutcome::Yield(value) => {
            panic!("expected yield 42, got yield {value:?}");
        }
        SimpleGeneratorStepOutcome::Throw(value) => {
            panic!("expected yield 42, got throw {value:?}");
        }
    }
}

#[test]
fn resolves_async_yield_delegate_generator_plan_for_class_static_method() {
    let program = frontend::parse(
        r#"
            var log = [];
            var obj = {
              get [Symbol.iterator]() {
                log.push("get-iterator");
                return function() {
                  var nextCount = 0;
                  return {
                    get next() {
                      return function() {
                        nextCount++;
                        if (nextCount == 1) {
                          return { value: "first", done: false };
                        }
                        return { value: "second", done: true };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                return null;
              }
            };

            class C {
              static async *gen() {
                log.push("before");
                var v = yield* obj;
                log.push(v);
                return "done";
              }
            }

            var gen = C.gen;
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    let user_function_names = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .map(|user_function| user_function.name.clone())
        .collect::<Vec<_>>();

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    let plan = user_function_names.iter().find_map(|function_name| {
        function_compiler.resolve_async_yield_delegate_generator_plan(
            &Expression::Call {
                callee: Box::new(Expression::Identifier(function_name.clone())),
                arguments: Vec::new(),
            },
            "__ayy_async_delegate_completion",
        )
    });
    assert!(plan.is_some(), "expected async delegate generator plan");
}

#[test]
fn tracks_async_yield_delegate_generator_iterator_binding() {
    let program = frontend::parse(
        r#"
            var obj = {
              [Symbol.iterator]() {
                return {
                  get next() {
                    return function() {
                      return { value: 1, done: false };
                    };
                  }
                };
              }
            };
            class C {
              static async *gen() {
                yield* obj;
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    let user_function_name = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| matches!(function.kind, FunctionKind::AsyncGenerator))
        .expect("expected async generator")
        .name
        .clone();

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    function_compiler.update_local_array_iterator_binding(
        "iter",
        &Expression::Call {
            callee: Box::new(Expression::Identifier(user_function_name)),
            arguments: Vec::new(),
        },
    );
    assert!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .contains_key("iter"),
        "expected iter iterator binding before inlining nested callback"
    );
    assert!(matches!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get("iter")
            .map(|binding| &binding.source),
        Some(IteratorSourceKind::AsyncYieldDelegateGenerator { .. })
    ));
}

#[test]
fn tracks_async_yield_delegate_abrupt_getiterator_fallback_as_throwing_iterator_binding() {
    let program = frontend::parse(
        r#"
            var reason = {};
            var obj = {
              get [Symbol.iterator]() {
                throw reason;
              },
              get [Symbol.asyncIterator]() {
                return null;
              }
            };
            class C {
              static async *gen() {
                yield* obj;
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let user_function_name = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| matches!(function.kind, FunctionKind::AsyncGenerator))
        .expect("expected async generator")
        .name
        .clone();

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .state
        .speculation
        .execution_context
        .current_user_function_name = Some(user_function_name);

    function_compiler.update_local_array_iterator_binding(
        "iter",
        &Expression::GetIterator(Box::new(Expression::Identifier("obj".to_string()))),
    );

    assert!(matches!(
        function_compiler.state.speculation.static_semantics.arrays.local_array_iterator_bindings
            .get("iter")
            .map(|binding| &binding.source),
        Some(IteratorSourceKind::SimpleGenerator {
            steps,
            completion_effects,
            completion_value: Expression::Undefined,
            ..
        }) if completion_effects.is_empty()
            && matches!(steps.as_slice(), [SimpleGeneratorStep {
                outcome: SimpleGeneratorStepOutcome::Throw(Expression::Identifier(name)),
                ..
            }] if name == "reason")
    ));
}

#[test]
fn consumes_async_yield_delegate_abrupt_getiterator_rejection_then_completion() {
    let program = frontend::parse(
        r#"
            var calls = 0;
            var reason = {};
            var obj = {
              get [Symbol.iterator]() {
                throw reason;
              },
              get [Symbol.asyncIterator]() {
                calls += 1;
                return null;
              }
            };
            class C {
              async *gen() {
                yield* obj;
                throw new Error("unreachable");
              }
            }
            var gen = C.prototype.gen;
            var iter = gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let (plan, delegate_iterator_name) = match function_compiler
        .state
        .speculation
        .static_semantics
        .arrays
        .local_array_iterator_bindings
        .get("iter")
        .map(|binding| &binding.source)
    {
        Some(IteratorSourceKind::AsyncYieldDelegateGenerator {
            plan,
            delegate_iterator_name,
            ..
        }) => (plan.clone(), delegate_iterator_name.clone()),
        _ => panic!("expected async yield delegate iterator binding"),
    };
    let async_iterator_property =
        function_compiler.materialize_static_expression(&Expression::Member {
            object: Box::new(Expression::Identifier("Symbol".to_string())),
            property: Box::new(Expression::String("asyncIterator".to_string())),
        });
    let async_iterator_binding = function_compiler
        .resolve_member_function_binding(&plan.delegate_expression, &async_iterator_property)
        .expect("expected async iterator method binding");
    let (delegate_iterator, mut snapshot_bindings) = function_compiler
        .resolve_bound_snapshot_function_result_with_arguments_and_this(
            &async_iterator_binding,
            &HashMap::new(),
            &[],
            &plan.delegate_expression,
        )
        .expect("expected static delegate iterator");
    snapshot_bindings.insert(delegate_iterator_name.clone(), delegate_iterator.clone());
    let next_method = function_compiler
        .evaluate_bound_snapshot_expression(
            &Expression::Member {
                object: Box::new(Expression::Identifier(delegate_iterator_name.clone())),
                property: Box::new(Expression::String("next".to_string())),
            },
            &mut snapshot_bindings,
            Some(&plan.function_name),
        )
        .expect("expected static delegate next method");
    let next_binding = function_compiler
        .resolve_function_binding_from_expression(&next_method)
        .expect("expected static next binding");
    let (next_step_result, mut next_snapshot_bindings) = function_compiler
        .resolve_bound_snapshot_function_result_with_arguments_and_this(
            &next_binding,
            &snapshot_bindings,
            &[Expression::Undefined],
            &delegate_iterator,
        )
        .expect("expected static next step result");
    match function_compiler.resolve_bound_snapshot_await_resolution_outcome(
        &next_step_result,
        &mut next_snapshot_bindings,
        Some(&plan.function_name),
    ) {
        Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(Expression::Identifier(name))))
            if name == "reason" => {}
        Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(name))) => {
            panic!("unexpected awaited named error: {name}");
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(Expression::Call { .. }))) => {
            panic!("unexpected awaited call-expression throw");
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(_))) => {
            panic!("unexpected awaited raw thrown value");
        }
        Some(StaticEvalOutcome::Value(_)) => {
            panic!("unexpected awaited fulfilled outcome");
        }
        None => panic!("expected awaited next-step outcome"),
    }

    let first = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[],
        )
        .expect("first delegate next should compile")
        .expect("first delegate next should exist");
    let second = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[],
        )
        .expect("second delegate next should compile")
        .expect("second delegate next should exist");

    match &first {
        StaticEvalOutcome::Throw(StaticThrowValue::Value(Expression::Identifier(name)))
            if name == "reason" => {}
        StaticEvalOutcome::Throw(StaticThrowValue::NamedError(name)) => {
            panic!("unexpected first named error: {name}");
        }
        StaticEvalOutcome::Throw(StaticThrowValue::Value(Expression::Call { .. })) => {
            panic!("unexpected first call-expression throw");
        }
        StaticEvalOutcome::Throw(StaticThrowValue::Value(_)) => {
            panic!("unexpected first thrown value");
        }
        StaticEvalOutcome::Value(_) => {
            panic!("unexpected first fulfilled outcome");
        }
    }
    assert!(matches!(
        second,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(done_key), value: Expression::Bool(true) },
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(value_key), value: Expression::Undefined },
                ] if done_key == "done" && value_key == "value"
            )
    ));
    assert_eq!(
        function_compiler.resolve_static_number_value(&Expression::Identifier("calls".to_string())),
        Some(1.0)
    );
}

#[test]
fn consumes_async_yield_delegate_undefined_async_getiterator_rejection_then_completion() {
    let program = frontend::parse(
        r#"
            var calls = 0;
            var reason = {};
            var obj = {
              get [Symbol.iterator]() {
                throw reason;
              },
              get [Symbol.asyncIterator]() {
                calls += 1;
                return undefined;
              }
            };
            class C {
              async *gen() {
                yield* obj;
                throw new Error("unreachable");
              }
            }
            var gen = C.prototype.gen;
            var iter = gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let first = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[],
        )
        .expect("first delegate next should compile")
        .expect("first delegate next should exist");
    let second = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[],
        )
        .expect("second delegate next should compile")
        .expect("second delegate next should exist");

    match &first {
        StaticEvalOutcome::Throw(StaticThrowValue::Value(Expression::Identifier(name)))
            if name == "reason" => {}
        StaticEvalOutcome::Throw(StaticThrowValue::NamedError(name)) => {
            panic!("unexpected first named error: {name}");
        }
        StaticEvalOutcome::Throw(StaticThrowValue::Value(Expression::Call { .. })) => {
            panic!("unexpected first call-expression throw");
        }
        StaticEvalOutcome::Throw(StaticThrowValue::Value(_)) => {
            panic!("unexpected first thrown value");
        }
        StaticEvalOutcome::Value(_) => {
            panic!("unexpected first fulfilled outcome");
        }
    }
    assert!(matches!(
        second,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(done_key), value: Expression::Bool(true) },
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(value_key), value: Expression::Undefined },
                ] if done_key == "done" && value_key == "value"
            )
    ));
    assert_eq!(
        function_compiler.resolve_static_number_value(&Expression::Identifier("calls".to_string())),
        Some(1.0)
    );
}

#[test]
fn consumes_async_yield_delegate_next_then_get_rejection_then_completion() {
    let program = frontend::parse(
        r#"
            var callCount = 0;
            var reason = {};
            var obj = {
              get [Symbol.iterator]() {
                throw new Error("should not get Symbol.iterator");
              },
              [Symbol.asyncIterator]() {
                return {
                  next() {
                    return {
                      get then() {
                        throw reason;
                      }
                    };
                  }
                };
              }
            };
            class C {
              async *gen() {
                callCount += 1;
                yield* obj;
                throw new Error("unreachable");
              }
            }
            var gen = C.prototype.gen;
            var iter = gen();
            iter.next().then(function() {
              throw new Error("unexpected fulfill");
            }, function(v) {
              console.log("reject", v);
              iter.next().then(function(v2) {
                console.log("done", v2.done, v2.value);
              });
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let Statement::Expression(then_expression) = program
        .statements
        .last()
        .expect("expected final then expression")
    else {
        panic!("expected final expression statement");
    };
    let outcome = function_compiler
        .consume_immediate_promise_outcome(then_expression)
        .expect("immediate promise outcome should compile");
    assert!(matches!(
        outcome,
        Some(StaticEvalOutcome::Value(Expression::Undefined))
    ));
    assert_eq!(
        function_compiler
            .resolve_static_number_value(&Expression::Identifier("callCount".to_string())),
        Some(1.0)
    );
}

#[test]
fn consumes_async_yield_delegate_generator_next_promise_outcome() {
    let program = frontend::parse(
        r#"
            var obj = {
              [Symbol.iterator]() {
                return {
                  get next() {
                    return function() {
                      return { value: 1, done: false };
                    };
                  }
                };
              }
            };
            class C {
              static async *gen() {
                yield* obj;
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    let user_function_name = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| matches!(function.kind, FunctionKind::AsyncGenerator))
        .expect("expected async generator")
        .name
        .clone();

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    function_compiler.update_local_array_iterator_binding(
        "iter",
        &Expression::Call {
            callee: Box::new(Expression::Identifier(user_function_name)),
            arguments: Vec::new(),
        },
    );
    let outcome = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[CallArgument::Expression(Expression::String(
                "ignored".to_string(),
            ))],
        )
        .expect("promise consumption should compile");
    assert!(matches!(
        outcome,
        Some(StaticEvalOutcome::Value(Expression::Object(_)))
    ));
}

#[test]
fn tracks_async_yield_delegate_generator_iterator_binding_after_emitting_alias_call() {
    let program = frontend::parse(
        r#"
            var obj = {
              [Symbol.iterator]() {
                return {
                  get next() {
                    return function() {
                      return { value: 1, done: false };
                    };
                  }
                };
              }
            };
            class C {
              static async *gen() {
                yield* obj;
              }
            }
            var gen = C.gen;
            var iter = gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    assert!(matches!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get("iter")
            .map(|binding| &binding.source),
        Some(IteratorSourceKind::AsyncYieldDelegateGenerator { .. })
    ));
}

#[test]
fn consumes_async_yield_delegate_generator_next_promise_outcome_after_alias_call() {
    let program = frontend::parse(
        r#"
            var obj = {
              [Symbol.iterator]() {
                return {
                  get next() {
                    return function() {
                      return { value: 1, done: false };
                    };
                  }
                };
              }
            };
            class C {
              static async *gen() {
                yield* obj;
              }
            }
            var gen = C.gen;
            var iter = gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let outcome = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[CallArgument::Expression(Expression::String(
                "ignored".to_string(),
            ))],
        )
        .expect("promise consumption should compile");
    assert!(matches!(
        outcome,
        Some(StaticEvalOutcome::Value(Expression::Object(_)))
    ));
}

#[test]
fn synthesizes_async_yield_delegate_next_capture_slots_from_snapshot_bindings() {
    let program = frontend::parse(
        r#"
            var obj = {
              [Symbol.iterator]() {
                var nextCount = 0;
                return {
                  get next() {
                    return function() {
                      nextCount++;
                      return { value: nextCount, done: false };
                    };
                  }
                };
              }
            };
            class C {
              static async *gen() {
                yield* obj;
              }
            }
            var gen = C.gen;
            var iter = gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let outcome = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[CallArgument::Expression(Expression::String(
                "ignored".to_string(),
            ))],
        )
        .expect("promise consumption should compile")
        .expect("expected promise outcome");
    assert!(matches!(
        outcome,
        StaticEvalOutcome::Value(Expression::Object(_))
    ));

    let delegate_iterator_name = match function_compiler
        .state
        .speculation
        .static_semantics
        .arrays
        .local_array_iterator_bindings
        .get("iter")
        .map(|binding| &binding.source)
    {
        Some(IteratorSourceKind::AsyncYieldDelegateGenerator {
            delegate_iterator_name,
            ..
        }) => delegate_iterator_name.clone(),
        _ => panic!("expected async yield delegate iterator binding"),
    };
    let capture_slots = function_compiler
        .resolve_member_function_capture_slots(
            &Expression::Identifier(delegate_iterator_name),
            &Expression::String("next".to_string()),
        )
        .expect("expected next member capture slots");
    assert!(
        capture_slots.contains_key("nextCount"),
        "expected next capture slots to preserve nextCount"
    );
}

#[test]
fn resolves_async_yield_delegate_next_identifier_capture_slots_from_snapshot_bindings() {
    let program = frontend::parse(
        r#"
            var obj = {
              [Symbol.iterator]() {
                return function() {
                  var log = [];
                  var nextCount = 0;
                  return {
                    get next() {
                      return function() {
                        log.push(nextCount);
                        nextCount++;
                        return { value: nextCount, done: false };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                return null;
              }
            };
            class C {
              static async *gen() {
                yield* obj;
              }
            }
            var iter = C.gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let outcome = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[CallArgument::Expression(Expression::String(
                "ignored".to_string(),
            ))],
        )
        .expect("promise consumption should compile")
        .expect("expected promise outcome");
    assert!(matches!(
        outcome,
        StaticEvalOutcome::Value(Expression::Object(_))
    ));

    assert!(matches!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get("iter")
            .map(|binding| &binding.source),
        Some(IteratorSourceKind::AsyncYieldDelegateGenerator { .. })
    ));
}

#[test]
fn resolves_async_yield_delegate_generator_next_promise_outcome_with_getter_results() {
    let program = frontend::parse(
        r#"
            var log = [];
            var obj = {
              get [Symbol.iterator]() {
                log.push("get-iter");
                return function() {
                  return {
                    get next() {
                      log.push("get-next");
                      return function() {
                        return {
                          get value() {
                            log.push("get-value");
                            return "next-value-1";
                          },
                          get done() {
                            log.push("get-done");
                            return false;
                          }
                        };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                log.push("get-async");
                return null;
              }
            };
            class C {
              static async *gen() {
                yield* obj;
              }
            }
            var gen = C.gen;
            var iter = gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let outcome = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[CallArgument::Expression(Expression::String(
                "ignored".to_string(),
            ))],
        )
        .expect("promise consumption should compile")
        .expect("expected static promise outcome");
    let StaticEvalOutcome::Value(outcome_expression) = outcome else {
        panic!("expected value outcome");
    };
    let outcome_binding = function_compiler
        .resolve_object_binding_from_expression(&outcome_expression)
        .expect("expected object result binding");
    assert_eq!(
        object_binding_lookup_value(&outcome_binding, &Expression::String("value".to_string())),
        Some(&Expression::String("next-value-1".to_string()))
    );
    assert_eq!(
        object_binding_lookup_value(&outcome_binding, &Expression::String("done".to_string())),
        Some(&Expression::Bool(false))
    );
}

fn syncs_async_yield_delegate_generator_snapshot_side_effect_bindings() {
    let program = frontend::parse(
        r#"
            var log = [];
            var obj = {
              get [Symbol.iterator]() {
                log.push("get-iter");
                return function() {
                  return {
                    get next() {
                      log.push("get-next");
                      return function() {
                        return {
                          get value() {
                            log.push("get-value");
                            return "next-value-1";
                          },
                          get done() {
                            log.push("get-done");
                            return false;
                          }
                        };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                log.push("get-async");
                return null;
              }
            };
            class C {
              static async *gen() {
                yield* obj;
              }
            }
            var gen = C.gen;
            var iter = gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[CallArgument::Expression(Expression::String(
                "ignored".to_string(),
            ))],
        )
        .expect("promise consumption should compile")
        .expect("expected static promise outcome");

    let log_binding = function_compiler
        .resolve_array_binding_from_expression(&Expression::Identifier("log".to_string()))
        .expect("expected log array binding");
    let logged_values = log_binding
        .values
        .iter()
        .map(|value| value.clone().unwrap_or(Expression::Undefined))
        .collect::<Vec<_>>();
    let expected_suffix = vec![
        Expression::String("get-async".to_string()),
        Expression::String("get-iter".to_string()),
        Expression::String("get-next".to_string()),
        Expression::String("get-done".to_string()),
        Expression::String("get-value".to_string()),
    ];
    assert!(logged_values.len() >= expected_suffix.len());
    assert_eq!(
        &logged_values[logged_values.len() - expected_suffix.len()..],
        expected_suffix.as_slice()
    );
}

#[test]
fn emits_immediate_then_callback_for_async_yield_delegate_generator_after_alias_call() {
    let program = frontend::parse(
        r#"
            var obj = {
              [Symbol.iterator]() {
                return {
                  get next() {
                    return function() {
                      return { value: 1, done: false };
                    };
                  }
                };
              }
            };
            var callCount = 0;
            class C {
              static async *gen() {
                callCount += 1;
                yield* obj;
              }
            }
            var gen = C.gen;
            var iter = gen();
            iter.next("ignored").then(function() {
              callCount += 10;
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    let setup_statements = &program.statements[..program.statements.len().saturating_sub(1)];
    for statement in setup_statements {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let Statement::Expression(Expression::Call { callee, arguments }) = program
        .statements
        .last()
        .expect("expected trailing then call")
    else {
        panic!("expected trailing then call");
    };
    let Expression::Member { object, property } = callee.as_ref() else {
        panic!("expected member call");
    };
    assert!(
        function_compiler
            .emit_immediate_promise_member_call(object, property, arguments)
            .expect("immediate promise call should emit"),
        "expected immediate promise member call to handle async generator next.then"
    );
}

#[test]
fn consumes_async_yield_delegate_generator_completion_outcome() {
    let program = frontend::parse(
        r#"
            var obj = {
              get [Symbol.iterator]() {
                return function() {
                  var nextCount = 0;
                  return {
                    get next() {
                      return function() {
                        nextCount++;
                        if (nextCount == 1) {
                          return {
                            get value() { return "next-value-1"; },
                            get done() { return false; }
                          };
                        }
                        return {
                          get value() { return "next-value-2"; },
                          get done() { return true; }
                        };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                return null;
              }
            };
            class C {
              static async *gen() {
                var v = yield* obj;
                return "return-value";
              }
            }
            var iter = C.gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let first = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[CallArgument::Expression(Expression::String(
                "arg1".to_string(),
            ))],
        )
        .expect("first async delegate outcome should compile")
        .expect("first async delegate outcome should exist");
    let second = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[CallArgument::Expression(Expression::String(
                "arg2".to_string(),
            ))],
        )
        .expect("second async delegate outcome should compile")
        .expect("second async delegate outcome should exist");
    assert!(matches!(
        first,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(done_key),
                        value: Expression::Bool(false)
                    },
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(value_key),
                        value: Expression::String(value)
                    },
                ] if done_key == "done" && value_key == "value" && value == "next-value-1"
            )
    ));
    assert!(matches!(
        second,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(done_key),
                        value: Expression::Bool(true)
                    },
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(value_key),
                        value: Expression::String(value)
                    },
                ] if done_key == "done" && value_key == "value" && value == "return-value"
            )
    ));
}

#[test]
fn consumes_async_yield_delegate_generator_return_outcomes() {
    let program = frontend::parse(
        r#"
            var obj = {
              [Symbol.iterator]() {
                var returnCount = 0;
                return {
                  get next() {
                    return function() {
                      return {
                        value: "next-value-1",
                        done: false
                      };
                    };
                  },
                  get return() {
                    return function() {
                      returnCount++;
                      if (returnCount == 1) {
                        return {
                          get value() { return "return-value-1"; },
                          get done() { return false; }
                        };
                      }
                      return {
                        get value() { return "return-value-2"; },
                        get done() { return true; }
                      };
                    };
                  }
                };
              }
            };
            class C {
              static async *gen() {
                yield* obj;
              }
            }
            var iter = C.gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let next_outcome = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[],
        )
        .expect("next outcome should compile")
        .expect("expected next outcome");
    let first_return_outcome = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "return",
            &[CallArgument::Expression(Expression::String(
                "return-arg-1".to_string(),
            ))],
        )
        .expect("first return outcome should compile")
        .expect("expected first return outcome");
    let second_return_outcome = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "return",
            &[],
        )
        .expect("second return outcome should compile")
        .expect("expected second return outcome");

    assert!(matches!(
        next_outcome,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(done_key),
                        value: Expression::Bool(false)
                    },
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(value_key),
                        value: Expression::String(value)
                    },
                ] if done_key == "done" && value_key == "value" && value == "next-value-1"
            )
    ));
    assert!(matches!(
        first_return_outcome,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(done_key),
                        value: Expression::Bool(false)
                    },
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(value_key),
                        value: Expression::String(value)
                    },
                ] if done_key == "done" && value_key == "value" && value == "return-value-1"
            )
    ));
    assert!(matches!(
        second_return_outcome,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(done_key),
                        value: Expression::Bool(true)
                    },
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(value_key),
                        value: Expression::String(value)
                    },
                ] if done_key == "done" && value_key == "value" && value == "return-value-2"
            )
    ));
    assert_eq!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get("iter")
            .and_then(|binding| binding.static_index),
        Some(2)
    );
}

#[test]
fn consumes_async_yield_delegate_generator_rejection_then_completion_for_unbound_method() {
    let program = frontend::parse(
        r#"
            let error = new Error();
            async function* readFile() {
              yield Promise.reject(error);
              yield "unreachable";
            }
            class C {
              async *gen() {
                yield * readFile();
              }
            }
            var gen = C.prototype.gen;
            var iter = gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    assert!(matches!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get("iter")
            .map(|binding| &binding.source),
        Some(IteratorSourceKind::SimpleGenerator { is_async: true, .. })
    ));

    let first = function_compiler
        .consume_simple_async_generator_next_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            &[],
        )
        .expect("first async delegate outcome should compile")
        .expect("first async delegate outcome should exist");
    let second = function_compiler
        .consume_simple_async_generator_next_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            &[],
        )
        .expect("second async delegate outcome should compile")
        .expect("second async delegate outcome should exist");

    assert!(matches!(
        first,
        StaticEvalOutcome::Throw(StaticThrowValue::Value(Expression::Identifier(name)))
            if name == "error"
    ));
    assert!(matches!(
        second,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(done_key),
                        value: Expression::Bool(true)
                    },
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(value_key),
                        value: Expression::Undefined
                    },
                ] if done_key == "done" && value_key == "value"
            )
    ));
}

#[test]
fn consumes_async_yield_delegate_generator_throw_outcomes() {
    let program = frontend::parse(
        r#"
            var obj = {
              [Symbol.iterator]() {
                var throwCount = 0;
                return {
                  get next() {
                    return function() {
                      return {
                        value: "next-value-1",
                        done: false
                      };
                    };
                  },
                  get throw() {
                    return function() {
                      throwCount++;
                      if (throwCount == 1) {
                        return {
                          get value() { return "throw-value-1"; },
                          get done() { return false; }
                        };
                      }
                      return {
                        get value() { return "throw-value-2"; },
                        get done() { return true; }
                      };
                    };
                  }
                };
              }
            };
            class C {
              static async *gen() {
                var v = yield* obj;
                return "return-value";
              }
            }
            var iter = C.gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let next_outcome = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[],
        )
        .expect("next outcome should compile")
        .expect("expected next outcome");
    let first_throw_outcome = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "throw",
            &[CallArgument::Expression(Expression::String(
                "throw-arg-1".to_string(),
            ))],
        )
        .expect("first throw outcome should compile")
        .expect("expected first throw outcome");
    let second_throw_outcome = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "throw",
            &[],
        )
        .expect("second throw outcome should compile")
        .expect("expected second throw outcome");

    assert!(matches!(
        next_outcome,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(done_key),
                        value: Expression::Bool(false)
                    },
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(value_key),
                        value: Expression::String(value)
                    },
                ] if done_key == "done" && value_key == "value" && value == "next-value-1"
            )
    ));
    assert!(matches!(
        first_throw_outcome,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(done_key),
                        value: Expression::Bool(false)
                    },
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(value_key),
                        value: Expression::String(value)
                    },
                ] if done_key == "done" && value_key == "value" && value == "throw-value-1"
            )
    ));
    assert!(matches!(
        second_throw_outcome,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(done_key),
                        value: Expression::Bool(true)
                    },
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String(value_key),
                        value: Expression::String(value)
                    },
                ] if done_key == "done" && value_key == "value" && value == "return-value"
            )
    ));
    assert_eq!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get("iter")
            .and_then(|binding| binding.static_index),
        Some(2)
    );
}

#[test]
fn substitutes_async_yield_delegate_var_completion_binding_to_hidden_local() {
    let program = frontend::parse(
        r#"
            var obj = {
              get [Symbol.iterator]() {
                return function() {
                  return {
                    get next() {
                      return function() {
                        return { value: "x", done: true };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                return null;
              }
            };
            class C {
              static async *gen() {
                var v = yield* obj;
                return "return-value";
              }
            }
            var iter = C.gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let plan = match function_compiler
        .state
        .speculation
        .static_semantics
        .arrays
        .local_array_iterator_bindings
        .get("iter")
        .map(|binding| &binding.source)
    {
        Some(IteratorSourceKind::AsyncYieldDelegateGenerator { plan, .. }) => plan,
        _ => panic!("expected async yield delegate iterator binding"),
    };
    match plan.completion_effects.as_slice() {
        [
            Statement::Var {
                name,
                value: Expression::Identifier(value_name),
            },
        ] if name.starts_with("__ayy_async_delegate_scope_v")
            && value_name == "__ayy_async_delegate_completion" => {}
        other => panic!("unexpected completion effects: {other:?}"),
    }
}

#[test]
fn consumes_immediate_promise_outcome_for_sequential_async_yield_delegate_then_calls() {
    let program = frontend::parse(
        r#"
            var obj = {
              get [Symbol.iterator]() {
                return function() {
                  var nextCount = 0;
                  return {
                    get next() {
                      return function() {
                        nextCount++;
                        if (nextCount == 1) {
                          return {
                            get value() { return "next-value-1"; },
                            get done() { return false; }
                          };
                        }
                        return {
                          get value() { return "next-value-2"; },
                          get done() { return true; }
                        };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                return null;
              }
            };
            function handler2(v2) {
              console.log("second", v2.value, v2.done);
            }
            class C {
              static async *gen() {
                var v = yield* obj;
                return "return-value";
              }
            }
            var iter = C.gen();
            iter.next("arg1").then(function(v) { console.log("first", v.value, v.done); });
            iter.next("arg2").then(handler2);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    let setup_len = program
        .statements
        .len()
        .checked_sub(2)
        .expect("expected two then statements");
    let setup_statements = &program.statements[..setup_len];
    let then_statements = &program.statements[setup_len..];
    for statement in setup_statements {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let first_outcome = function_compiler
        .consume_immediate_promise_outcome(match &then_statements[0] {
            Statement::Expression(expression) => expression,
            _ => panic!("expected first then expression statement"),
        })
        .expect("first immediate promise consumption should compile");
    let second_outcome = function_compiler
        .consume_immediate_promise_outcome(match &then_statements[1] {
            Statement::Expression(expression) => expression,
            _ => panic!("expected second then expression statement"),
        })
        .expect("second immediate promise consumption should compile");

    assert!(matches!(
        first_outcome,
        Some(StaticEvalOutcome::Value(Expression::Undefined))
    ));
    assert!(matches!(
        second_outcome,
        Some(StaticEvalOutcome::Value(Expression::Undefined))
    ));
}

#[test]
fn resolves_rejected_yield_async_generator_next_outcome() {
    let program = frontend::parse(
        r#"
            let error = new Error();
            async function* gen() {
              yield Promise.reject(error);
            }
            var iter = gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    let iter_initializer = program
        .statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Var { name, value } if name == "iter" => Some(value.clone()),
            _ => None,
        })
        .expect("expected iter initializer");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    assert!(
        function_compiler
            .resolve_simple_generator_source(&iter_initializer)
            .is_some(),
        "expected iter initializer to resolve as a simple generator source",
    );

    let outcome = function_compiler
        .consume_simple_async_generator_next_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            &[],
        )
        .expect("next outcome should evaluate")
        .expect("next outcome should exist");

    let throws_error_identifier = matches!(
        &outcome,
        StaticEvalOutcome::Throw(StaticThrowValue::Value(Expression::Identifier(name)))
            if name == "error"
    );
    let throws_reject_call = matches!(
        &outcome,
        StaticEvalOutcome::Throw(StaticThrowValue::Value(Expression::Call { callee, .. }))
            if matches!(
                callee.as_ref(),
                Expression::Member { object, property }
                    if matches!(object.as_ref(), Expression::Identifier(name) if name == "Promise")
                        && matches!(property.as_ref(), Expression::String(name) if name == "reject")
            )
    );
    let yields_reject_call = matches!(
        &outcome,
        StaticEvalOutcome::Value(Expression::Object(entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(done_key), value: Expression::Bool(false) },
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(value_key), value: Expression::Call { callee, .. } },
                ] if done_key == "done"
                    && value_key == "value"
                    && matches!(
                        callee.as_ref(),
                        Expression::Member { object, property }
                            if matches!(object.as_ref(), Expression::Identifier(name) if name == "Promise")
                                && matches!(property.as_ref(), Expression::String(name) if name == "reject")
                    )
            )
    );

    assert!(
        throws_error_identifier || throws_reject_call,
        "expected rejected Promise.reject yield outcome to reject, not resolve (throws_error_identifier={throws_error_identifier}, throws_reject_call={throws_reject_call}, yields_reject_call={yields_reject_call})",
    );
}

#[test]
fn consumes_for_await_async_generator_rejection_then_completion() {
    let program = frontend::parse(
        r#"
            let error = new Error();
            async function* readFile() {
              yield Promise.reject(error);
              yield "unreachable";
            }
            class C {
              async *gen() {
                for await (let line of readFile()) {
                  yield line;
                }
              }
            }
            var iter = C.prototype.gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let Some(binding_name) = function_compiler.resolve_local_array_iterator_binding_name("iter")
    else {
        panic!("expected iter iterator binding name");
    };
    assert!(matches!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get(&binding_name)
            .map(|binding| &binding.source),
        Some(IteratorSourceKind::SimpleGenerator { is_async: true, .. })
    ));

    let first = function_compiler
        .consume_simple_async_generator_next_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            &[],
        )
        .expect("first outcome should evaluate")
        .expect("first outcome should exist");
    let second = function_compiler
        .consume_simple_async_generator_next_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            &[],
        )
        .expect("second outcome should evaluate")
        .expect("second outcome should exist");

    assert!(matches!(
        first,
        StaticEvalOutcome::Throw(StaticThrowValue::Value(Expression::Identifier(name)))
            if name == "error"
    ));
    assert!(matches!(
        second,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(done_key), value: Expression::Bool(true) },
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(value_key), value: Expression::Undefined },
                ] if done_key == "done" && value_key == "value"
            )
    ));
}

#[test]
fn consumes_for_await_sync_iterable_rejection_then_completion() {
    let program = frontend::parse(
        r#"
            let error = new Error();
            let iterable = [
              Promise.reject(error),
              "unreachable"
            ];
            class C {
              async *gen() {
                for await (let value of iterable) {
                  yield value;
                }
              }
            }
            var iter = C.prototype.gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }
    let iter_initializer = program
        .statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Var { name, value } if name == "iter" => Some(value.clone()),
            _ => None,
        })
        .expect("expected iter initializer");
    assert!(
        function_compiler
            .resolve_simple_generator_source(&iter_initializer)
            .is_some(),
        "expected iter initializer to resolve as a simple generator source",
    );
    let Some(binding_name) = function_compiler.resolve_local_array_iterator_binding_name("iter")
    else {
        panic!("expected iter iterator binding name");
    };
    assert!(matches!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get(&binding_name)
            .map(|binding| &binding.source),
        Some(IteratorSourceKind::SimpleGenerator { is_async: true, .. })
    ));
}

#[test]
fn consumes_for_await_sync_iterable_rejection_then_completion_for_unbound_method() {
    let program = frontend::parse(
        r#"
            let error = new Error();
            let iterable = [
              Promise.reject(error),
              "unreachable"
            ];
            class C {
              async *gen() {
                for await (let value of iterable) {
                  yield value;
                }
              }
            }
            var gen = C.prototype.gen;
            var iter = gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let iter_initializer = program
        .statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Var { name, value } if name == "iter" => Some(value.clone()),
            _ => None,
        })
        .expect("expected iter initializer");
    assert!(
        function_compiler
            .resolve_simple_generator_source(&iter_initializer)
            .is_some(),
        "expected unbound iter initializer to resolve as a simple generator source",
    );
    let Some(binding_name) = function_compiler.resolve_local_array_iterator_binding_name("iter")
    else {
        panic!("expected iter iterator binding name");
    };
    assert!(matches!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get(&binding_name)
            .map(|binding| &binding.source),
        Some(IteratorSourceKind::SimpleGenerator { is_async: true, .. })
    ));
}

#[test]
fn consumes_immediate_promise_outcome_for_for_await_async_generator_reject_chain() {
    let program = frontend::parse(
        r#"
            let error = new Error();
            async function* readFile() {
              yield Promise.reject(error);
              yield "unreachable";
            }
            class C {
              async *gen() {
                for await (let line of readFile()) {
                  yield line;
                }
              }
            }
            var iter = C.prototype.gen();
            iter.next().then(
              function() {
                throw new Error("resolved");
              },
              function(rejectValue) {
                iter.next()
                  .then(function({done, value}) {
                    console.log(done, value);
                  })
                  .then(
                    function() { console.log("inner-then"); },
                    function(err) { console.log("inner-reject", err && err.message); }
                  );
              }
            ).catch(function(err) {
              console.log("outer-catch", err && err.message);
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let Statement::Expression(expression) = program
        .statements
        .last()
        .expect("expected top-level chained promise expression")
    else {
        panic!("expected chained promise expression statement");
    };

    assert!(matches!(
        function_compiler
            .consume_immediate_promise_outcome(expression)
            .expect("immediate promise outcome should compile"),
        Some(StaticEvalOutcome::Value(Expression::Undefined))
    ));
}

#[test]
fn consumes_immediate_promise_outcome_for_for_await_sync_iterable_reject_chain() {
    let program = frontend::parse(
        r#"
            let error = new Error();
            let iterable = [
              Promise.reject(error),
              "unreachable"
            ];
            class C {
              async *gen() {
                for await (let value of iterable) {
                  yield value;
                }
              }
            }
            var iter = C.prototype.gen();
            iter.next().then(
              function() {
                throw new Error("resolved");
              },
              function(rejectValue) {
                iter.next()
                  .then(function({done, value}) {
                    console.log(done, value);
                  })
                  .then(
                    function() { console.log("inner-then"); },
                    function(err) { console.log("inner-reject", err && err.message); }
                  );
              }
            ).catch(function(err) {
              console.log("outer-catch", err && err.message);
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let Statement::Expression(expression) = program
        .statements
        .last()
        .expect("expected top-level chained promise expression")
    else {
        panic!("expected chained promise expression statement");
    };

    assert!(matches!(
        function_compiler
            .consume_immediate_promise_outcome(expression)
            .expect("immediate promise outcome should compile"),
        Some(StaticEvalOutcome::Value(Expression::Undefined))
    ));
}

#[test]
fn consumes_immediate_promise_outcome_for_unbound_yield_star_async_iterator_reject_chain() {
    let program = frontend::parse(
        r#"
            let error = new Error();
            async function* readFile() {
              yield Promise.reject(error);
              yield "unreachable";
            }
            class C {
              async *gen() {
                yield * readFile();
              }
            }
            var gen = C.prototype.gen;
            var iter = gen();
            iter.next().then(
              function() {
                throw new Error("resolved");
              },
              function(rejectValue) {
                iter.next()
                  .then(function({done, value}) {
                    console.log(done, value);
                  })
                  .then(
                    function() { console.log("inner-then"); },
                    function(err) { console.log("inner-reject", err && err.message); }
                  );
              }
            ).catch(function(err) {
              console.log("outer-catch", err && err.message);
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let Statement::Expression(expression) = program
        .statements
        .last()
        .expect("expected top-level chained promise expression")
    else {
        panic!("expected chained promise expression statement");
    };

    assert!(matches!(
        function_compiler
            .consume_immediate_promise_outcome(expression)
            .expect("immediate promise outcome should compile"),
        Some(StaticEvalOutcome::Value(Expression::Undefined))
    ));
}

#[test]
fn consumes_for_await_sync_iterable_first_next_as_rejection() {
    let program = frontend::parse(
        r#"
            let error = new Error();
            let iterable = [
              Promise.reject(error),
              "unreachable"
            ];
            class C {
              async *gen() {
                for await (let value of iterable) {
                  yield value;
                }
              }
            }
            var iter = C.prototype.gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let first = function_compiler
        .consume_simple_async_generator_next_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            &[],
        )
        .expect("first next outcome should evaluate")
        .expect("first next outcome should exist");
    let second = function_compiler
        .consume_simple_async_generator_next_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            &[],
        )
        .expect("second next outcome should evaluate")
        .expect("second next outcome should exist");

    assert!(matches!(
        first,
        StaticEvalOutcome::Throw(StaticThrowValue::Value(Expression::Identifier(name)))
            if name == "error"
    ));
    assert!(matches!(
        second,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(done_key), value: Expression::Bool(true) },
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(value_key), value: Expression::Undefined },
                ] if done_key == "done" && value_key == "value"
            )
    ));
}

#[test]
fn inlines_immediate_promise_callback_with_lowered_pattern_parameters() {
    let program = frontend::parse(
        r#"
            Promise.resolve({ done: true, value: undefined }).then(function({done, value}) {
              console.log(done, value);
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    let Statement::Expression(Expression::Call { arguments, .. }) = program
        .statements
        .last()
        .expect("expected then expression statement")
    else {
        panic!("expected then expression statement");
    };
    let handler = function_compiler
        .promise_handler_expression(arguments.first())
        .expect("expected callback handler");
    let user_function = function_compiler
        .resolve_user_function_from_expression(&handler)
        .expect("expected handler user function");
    let capture_slots = function_compiler.resolve_function_expression_capture_slots(&handler);

    assert!(user_function.has_lowered_pattern_parameters());
    assert!(
        capture_slots.is_none(),
        "expected lowered destructured promise callback to have no bound capture slots, got {capture_slots:?}"
    );
    assert!(!function_compiler.can_inline_user_function_call(
        user_function,
        &[Expression::Object(vec![
            crate::ir::hir::ObjectEntry::Data {
                key: Expression::String("done".to_string()),
                value: Expression::Bool(true),
            },
            crate::ir::hir::ObjectEntry::Data {
                key: Expression::String("value".to_string()),
                value: Expression::Undefined,
            },
        ])],
    ));
    assert!(
        function_compiler.can_inline_user_function_call_with_explicit_call_frame(
            user_function,
            &[Expression::Object(vec![
                crate::ir::hir::ObjectEntry::Data {
                    key: Expression::String("done".to_string()),
                    value: Expression::Bool(true),
                },
                crate::ir::hir::ObjectEntry::Data {
                    key: Expression::String("value".to_string()),
                    value: Expression::Undefined,
                },
            ])],
            &Expression::Undefined,
        )
    );
}

#[test]
fn defers_rejected_yield_async_generator_call_result() {
    let program = frontend::parse(
        r#"
            let error = new Error();
            async function* gen() {
              yield Promise.reject(error);
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let user_function = function_compiler
        .backend
        .function_registry
        .catalog
        .user_function_map
        .get("gen")
        .cloned()
        .expect("expected async generator function");

    assert!(
        function_compiler
            .emit_deferred_generator_call_result(&user_function, &[])
            .expect("deferred generator call result should evaluate"),
        "expected async generator call to defer to iterator creation",
    );
}

#[test]
fn emits_no_runtime_call_for_rejected_yield_async_generator_creation() {
    let program = frontend::parse(
        r#"
            let error = new Error();
            async function* gen() {
              yield Promise.reject(error);
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let call_expression = Expression::Call {
        callee: Box::new(Expression::Identifier("gen".to_string())),
        arguments: Vec::new(),
    };
    let base_len = function_compiler.state.emission.output.instructions.len();
    function_compiler
        .emit_numeric_expression(&call_expression)
        .expect("call expression should emit");
    let emitted = &function_compiler.state.emission.output.instructions[base_len..];

    assert!(
        !emitted.contains(&0x10),
        "expected deferred async generator creation to avoid direct wasm calls",
    );
}

#[test]
fn materializes_log_entry_properties_after_async_yield_delegate_then_consumption() {
    let program = frontend::parse(
        r#"
            var log = [];
            var obj = {
              get [Symbol.iterator]() {
                log.push({ name: "get [Symbol.iterator]", thisValue: this });
                return function() {
                  log.push({ name: "call [Symbol.iterator]", thisValue: this, args: [...arguments] });
                  var nextCount = 0;
                  return {
                    name: "syncIterator",
                    get next() {
                      log.push({ name: "get next", thisValue: this });
                      return function() {
                        log.push({ name: "call next", thisValue: this, args: [...arguments] });
                        nextCount++;
                        if (nextCount == 1) {
                          return {
                            name: "next-result-1",
                            get value() {
                              log.push({ name: "get next value (1)", thisValue: this });
                              return "next-value-1";
                            },
                            get done() {
                              log.push({ name: "get next done (1)", thisValue: this });
                              return false;
                            }
                          };
                        }
                        return {
                          name: "next-result-2",
                          get value() {
                            log.push({ name: "get next value (2)", thisValue: this });
                            return "next-value-2";
                          },
                          get done() {
                            log.push({ name: "get next done (2)", thisValue: this });
                            return true;
                          }
                        };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                log.push({ name: "get [Symbol.asyncIterator]" });
                return null;
              }
            };
            class C {
              static async *gen() {
                log.push({ name: "before yield*" });
                yield* obj;
              }
            }
            var iter = C.gen();
            iter.next("arg1").then(function(v) {
              console.log(v.value, v.done, log.length, log[0].name, log[1].name, log[2].name);
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    let setup_len = program
        .statements
        .len()
        .checked_sub(1)
        .expect("expected one then statement");
    for statement in &program.statements[..setup_len] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }
    let then_expression = match &program.statements[setup_len] {
        Statement::Expression(expression) => expression,
        _ => panic!("expected then expression statement"),
    };
    assert!(matches!(
        function_compiler
            .consume_immediate_promise_outcome(then_expression)
            .expect("immediate promise outcome should compile"),
        Some(StaticEvalOutcome::Value(Expression::Undefined))
    ));

    for (index, expected_name) in [
        "before yield*",
        "get [Symbol.asyncIterator]",
        "get [Symbol.iterator]",
    ]
    .into_iter()
    .enumerate()
    {
        let name_expression = Expression::Member {
            object: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("log".to_string())),
                property: Box::new(Expression::Number(index as f64)),
            }),
            property: Box::new(Expression::String("name".to_string())),
        };
        assert_eq!(
            function_compiler.materialize_static_expression(&name_expression),
            Expression::String(expected_name.to_string())
        );
    }
    let previous_user_function_name = function_compiler
        .state
        .speculation
        .execution_context
        .current_user_function_name
        .clone();
    function_compiler
        .state
        .speculation
        .execution_context
        .current_user_function_name = program
        .functions
        .iter()
        .find(|function| matches!(function.body.as_slice(), [Statement::Print { .. }]))
        .map(|function| function.name.clone());
    let callback_scoped_expression = Expression::Member {
        object: Box::new(Expression::Member {
            object: Box::new(Expression::Identifier("log".to_string())),
            property: Box::new(Expression::Number(0.0)),
        }),
        property: Box::new(Expression::String("name".to_string())),
    };
    assert_eq!(
        function_compiler.materialize_static_expression(&callback_scoped_expression),
        Expression::String("before yield*".to_string())
    );
    function_compiler
        .state
        .speculation
        .execution_context
        .current_user_function_name = previous_user_function_name;
}

#[test]
fn inlines_top_level_then_handler_for_async_yield_delegate_completion() {
    let program = frontend::parse(
        r#"
            var obj = {
              get [Symbol.iterator]() {
                return function() {
                  var nextCount = 0;
                  return {
                    get next() {
                      return function() {
                        nextCount++;
                        if (nextCount == 1) {
                          return {
                            get value() { return "next-value-1"; },
                            get done() { return false; }
                          };
                        }
                        return {
                          get value() { return "next-value-2"; },
                          get done() { return true; }
                        };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                return null;
              }
            };
            function handler2(v2) {
              console.log("second", v2.value, v2.done);
            }
            class C {
              static async *gen() {
                var v = yield* obj;
                return "return-value";
              }
            }
            var iter = C.gen();
            iter.next("arg2").then(handler2);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let then_statement = match program.statements.last().expect("expected then statement") {
        Statement::Expression(expression) => expression,
        _ => panic!("expected then expression statement"),
    };
    let Expression::Call { arguments, .. } = then_statement else {
        panic!("expected then call expression");
    };
    let handler_expression = function_compiler
        .promise_handler_expression(arguments.first())
        .expect("expected handler expression");
    let user_function = function_compiler
        .resolve_user_function_from_expression(&handler_expression)
        .cloned()
        .expect("expected handler user function");
    let result_local = function_compiler.allocate_temp_local();
    assert!(
        function_compiler
            .emit_inline_user_function_summary_with_explicit_call_frame(
                &user_function,
                &[Expression::Object(vec![
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String("done".to_string()),
                        value: Expression::Bool(true),
                    },
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String("value".to_string()),
                        value: Expression::String("return-value".to_string()),
                    },
                ])],
                &Expression::Undefined,
                result_local,
            )
            .expect("inline handler emission should compile"),
        "expected top-level then handler to inline"
    );
}

#[test]
fn inlines_side_effect_only_then_callback_with_explicit_call_frame() {
    let program = frontend::parse(
        r#"
            Promise.resolve({ value: 1, done: false }).then(function(v) {
              console.log("vals", v.value, v.done);
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    let Statement::Expression(Expression::Call { arguments, .. }) =
        program.statements.first().expect("expected then call")
    else {
        panic!("expected top-level call expression");
    };
    let handler = function_compiler
        .promise_handler_expression(arguments.first())
        .expect("expected handler");
    let user_function = function_compiler
        .resolve_user_function_from_expression(&handler)
        .cloned()
        .expect("expected handler user function");
    let result_local = function_compiler.allocate_temp_local();
    assert!(
        function_compiler
            .emit_inline_user_function_summary_with_explicit_call_frame(
                &user_function,
                &[Expression::Object(vec![
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String("value".to_string()),
                        value: Expression::Number(1.0),
                    },
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String("done".to_string()),
                        value: Expression::Bool(false),
                    },
                ])],
                &Expression::Undefined,
                result_local,
            )
            .expect("inline callback emission should compile"),
        "expected side-effect-only then callback to inline"
    );
}

#[test]
fn materializes_inline_then_arrow_callback_handler_expression() {
    let program = frontend::parse(
        r#"
            Promise.resolve(1).then(v => v);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    let Statement::Expression(Expression::Call { arguments, .. }) = &program.statements[0] else {
        panic!("expected top-level call expression");
    };
    let handler = function_compiler.promise_handler_expression(arguments.first());
    assert!(
        matches!(handler, Some(Expression::Identifier(ref name)) if name.starts_with("__ayy_arrow_")),
        "expected materialized arrow handler identifier, got {handler:?}"
    );
}

#[test]
fn materializes_inline_then_function_callback_handler_expression() {
    let program = frontend::parse(
        r#"
            Promise.resolve(1).then(function(v) { return v; }, function(err) { return err; });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    let Statement::Expression(Expression::Call { arguments, .. }) = &program.statements[0] else {
        panic!("expected top-level call expression");
    };
    let handler = function_compiler.promise_handler_expression(arguments.first());
    assert!(
        matches!(handler, Some(Expression::Identifier(ref name)) if name.starts_with("__ayy_fnexpr_")),
        "expected materialized function handler identifier, got {handler:?}"
    );
}

#[test]
fn substitutes_scoped_arguments_alias_in_explicit_call_frame() {
    let program = frontend::parse(
        r#"
            function outer() {
              return function() {
                return [...arguments];
              };
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let inner_user_function = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| function.name.starts_with("__ayy_fnexpr_"))
        .cloned()
        .expect("expected nested function expression user function");
    let inline_summary = inner_user_function
        .inline_summary
        .as_ref()
        .expect("expected nested function inline summary");
    let mut referenced_names = HashSet::new();
    for effect in &inline_summary.effects {
        match effect {
            crate::backend::direct_wasm::InlineFunctionEffect::Assign { value, .. } => {
                collect_referenced_binding_names_from_expression(value, &mut referenced_names);
            }
            crate::backend::direct_wasm::InlineFunctionEffect::Update { name, .. } => {
                referenced_names.insert(name.clone());
            }
            crate::backend::direct_wasm::InlineFunctionEffect::Expression(expression) => {
                collect_referenced_binding_names_from_expression(expression, &mut referenced_names);
            }
        }
    }
    if let Some(return_value) = inline_summary.return_value.as_ref() {
        collect_referenced_binding_names_from_expression(return_value, &mut referenced_names);
    }
    let scoped_arguments_name = referenced_names
        .into_iter()
        .find(|name| name.starts_with("__ayy_scope$arguments$"))
        .expect("expected scoped arguments alias in nested function body");

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    let arguments_binding = Expression::Array(vec![crate::ir::hir::ArrayElement::Expression(
        Expression::String("x".to_string()),
    )]);

    assert_eq!(
        function_compiler.substitute_call_frame_special_bindings(
            &Expression::Identifier(scoped_arguments_name),
            &inner_user_function,
            &Expression::Undefined,
            &arguments_binding,
        ),
        arguments_binding
    );
}

#[test]
fn resolves_array_binding_from_object_property_member_expression() {
    let program = frontend::parse(
        r#"
            var log = [{ args: [undefined] }];
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    let binding = function_compiler
        .resolve_array_binding_from_expression(&Expression::Member {
            object: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("log".to_string())),
                property: Box::new(Expression::Number(0.0)),
            }),
            property: Box::new(Expression::String("args".to_string())),
        })
        .expect("expected nested array property binding");

    assert_eq!(binding.values, vec![Some(Expression::Undefined)]);
}

#[test]
fn resolves_nested_then_callback_handler_to_inline_user_function() {
    let program = frontend::parse(
        r#"
            class C {
              static async *gen() {
                yield { value: "x", done: false };
              }
            }
            var iter = C.gen();
            iter.next().then(function(v) {
              console.log(v.value.value, v.value.done);
              iter.next().then(function(v2) {
                console.log(v2.value.value, v2.value.done);
              });
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    let arguments = program
        .statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Expression(Expression::Call { arguments, .. }) => Some(arguments),
            _ => None,
        })
        .expect("expected top-level then call expression");
    let handler = function_compiler
        .promise_handler_expression(arguments.first())
        .expect("expected nested then callback handler expression");
    let user_function = function_compiler
        .resolve_user_function_from_expression(&handler)
        .cloned()
        .expect("expected nested then callback user function");
    assert!(
        user_function.inline_summary.is_some(),
        "expected nested then callback inline summary"
    );
}

#[test]
fn inlines_nested_then_callback_handler_with_explicit_call_frame() {
    let program = frontend::parse(
        r#"
            class C {
              static async *gen() {
                yield { value: "x", done: false };
              }
            }
            var iter = C.gen();
            iter.next().then(function(v) {
              console.log(v.value.value, v.value.done);
              iter.next().then(function(v2) {
                console.log(v2.value.value, v2.value.done);
              });
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    let (_then_statement, setup_statements) = program
        .statements
        .split_last()
        .expect("expected top-level then call");
    for statement in setup_statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }
    let user_function_name = function_compiler
        .backend
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| matches!(function.kind, FunctionKind::AsyncGenerator))
        .expect("expected async generator user function")
        .name
        .clone();
    function_compiler.update_local_array_iterator_binding(
        "iter",
        &Expression::Call {
            callee: Box::new(Expression::Identifier(user_function_name)),
            arguments: Vec::new(),
        },
    );

    let arguments = program
        .statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Expression(Expression::Call { arguments, .. }) => Some(arguments),
            _ => None,
        })
        .expect("expected top-level then call expression");
    let handler = function_compiler
        .promise_handler_expression(arguments.first())
        .expect("expected nested then callback handler expression");
    let user_function = function_compiler
        .resolve_user_function_from_expression(&handler)
        .cloned()
        .expect("expected nested then callback user function");
    let result_local = function_compiler.allocate_temp_local();
    assert!(
        function_compiler
            .emit_inline_user_function_summary_with_explicit_call_frame(
                &user_function,
                &[Expression::Object(vec![
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String("value".to_string()),
                        value: Expression::Object(vec![
                            crate::ir::hir::ObjectEntry::Data {
                                key: Expression::String("value".to_string()),
                                value: Expression::String("x".to_string()),
                            },
                            crate::ir::hir::ObjectEntry::Data {
                                key: Expression::String("done".to_string()),
                                value: Expression::Bool(false),
                            },
                        ]),
                    },
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String("done".to_string()),
                        value: Expression::Bool(false),
                    },
                ])],
                &Expression::Undefined,
                result_local,
            )
            .expect("inline callback emission should compile"),
        "expected nested then callback to inline"
    );
}

#[test]
fn tracks_global_async_generator_iterator_binding_after_var_assignment() {
    let program = frontend::parse(
        r#"
            class C {
              static async *gen() {
                yield 1;
              }
            }
            var iter = C.gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    assert!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .contains_key("iter"),
        "expected global async generator assignment to track iter as an iterator binding"
    );
}

#[test]
fn inlines_actual_like_then_callback_with_explicit_call_frame() {
    let program = frontend::parse(
        r#"
            var log = [];
            var obj = {
              get [Symbol.iterator]() {
                log.push({ name: "get [Symbol.iterator]", thisValue: this });
                return function() {
                  log.push({ name: "call [Symbol.iterator]", thisValue: this, args: [...arguments] });
                  var nextCount = 0;
                  return {
                    name: "syncIterator",
                    get next() {
                      log.push({ name: "get next", thisValue: this });
                      return function() {
                        log.push({ name: "call next", thisValue: this, args: [...arguments] });
                        nextCount++;
                        if (nextCount == 1) {
                          return {
                            name: "next-result-1",
                            get value() {
                              log.push({ name: "get next value (1)", thisValue: this });
                              return "next-value-1";
                            },
                            get done() {
                              log.push({ name: "get next done (1)", thisValue: this });
                              return false;
                            }
                          };
                        }
                        return {
                          name: "next-result-2",
                          get value() {
                            log.push({ name: "get next value (2)", thisValue: this });
                            return "next-value-2";
                          },
                          get done() {
                            log.push({ name: "get next done (2)", thisValue: this });
                            return true;
                          }
                        };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                log.push({ name: "get [Symbol.asyncIterator]" });
                return null;
              }
            };
            class C {
              static async *gen() {
                log.push({ name: "before yield*" });
                var v = yield* obj;
                log.push({ name: "after yield*", value: v });
                return "return-value";
              }
            }
            var gen = C.gen;
            var iter = gen();
            iter.next("arg1").then(function(v) {
              console.log("after-first", v.value, v.done, log.length);
              iter.next("arg2").then(function(v2) {
                console.log("after-second", v2.value, v2.done, log.length);
              });
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let then_statement = match program.statements.last().expect("expected then statement") {
        Statement::Expression(expression) => expression,
        _ => panic!("expected then expression statement"),
    };
    let Expression::Call { arguments, .. } = then_statement else {
        panic!("expected then call expression");
    };
    let handler = function_compiler
        .promise_handler_expression(arguments.first())
        .expect("expected handler expression");
    let user_function = function_compiler
        .resolve_user_function_from_expression(&handler)
        .cloned()
        .expect("expected handler user function");
    let result_local = function_compiler.allocate_temp_local();
    assert!(
        function_compiler
            .emit_inline_user_function_summary_with_explicit_call_frame(
                &user_function,
                &[Expression::Object(vec![
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String("done".to_string()),
                        value: Expression::Bool(false),
                    },
                    crate::ir::hir::ObjectEntry::Data {
                        key: Expression::String("value".to_string()),
                        value: Expression::String("next-value-1".to_string()),
                    },
                ])],
                &Expression::Undefined,
                result_local,
            )
            .expect("inline callback emission should compile"),
        "expected actual-like callback to inline"
    );
}

#[test]
fn inlining_actual_like_callback_after_first_yield_preserves_generator_progress() {
    let program = frontend::parse(
        r#"
            var log = [];
            var callCount = 0;
            var obj = {
              get [Symbol.iterator]() {
                log.push({ name: "get [Symbol.iterator]", thisValue: this });
                return function() {
                  log.push({ name: "call [Symbol.iterator]", thisValue: this, args: [...arguments] });
                  var nextCount = 0;
                  return {
                    name: "syncIterator",
                    get next() {
                      log.push({ name: "get next", thisValue: this });
                      return function() {
                        log.push({ name: "call next", thisValue: this, args: [...arguments] });
                        nextCount++;
                        if (nextCount == 1) {
                          return {
                            name: "next-result-1",
                            get value() {
                              log.push({ name: "get next value (1)", thisValue: this });
                              return "next-value-1";
                            },
                            get done() {
                              log.push({ name: "get next done (1)", thisValue: this });
                              return false;
                            }
                          };
                        }
                        return {
                          name: "next-result-2",
                          get value() {
                            log.push({ name: "get next value (2)", thisValue: this });
                            return "next-value-2";
                          },
                          get done() {
                            log.push({ name: "get next done (2)", thisValue: this });
                            return true;
                          }
                        };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                log.push({ name: "get [Symbol.asyncIterator]" });
                return null;
              }
            };
            class C {
              static async *gen() {
                callCount += 1;
                log.push({ name: "before yield*" });
                var v = yield* obj;
                log.push({ name: "after yield*", value: v });
                return "return-value";
              }
            }
            var gen = C.gen;
            var iter = gen();
            iter.next("arg1").then(function(v) {
              console.log("after-first", v.value, v.done, log.length, callCount);
              iter.next("arg2").then(function(v2) {
                console.log("after-second", v2.value, v2.done, log.length, callCount);
              });
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let outer_then_statement = match program.statements.last().expect("expected then statement") {
        Statement::Expression(expression) => expression,
        _ => panic!("expected then expression statement"),
    };
    let outer_handler = match outer_then_statement {
        Expression::Call { arguments, .. } => function_compiler
            .promise_handler_expression(arguments.first())
            .expect("expected outer handler expression"),
        _ => panic!("expected outer then call expression"),
    };
    let outer_user_function = function_compiler
        .resolve_user_function_from_expression(&outer_handler)
        .cloned()
        .expect("expected outer handler user function");

    let first_outcome = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[CallArgument::Expression(Expression::String(
                "arg1".to_string(),
            ))],
        )
        .expect("first delegate next should compile")
        .expect("first delegate next should return static outcome");
    let first_value = match first_outcome {
        StaticEvalOutcome::Value(value) => value,
        _ => panic!("expected first delegate value outcome"),
    };
    assert_eq!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get("iter")
            .and_then(|binding| binding.static_index),
        Some(1)
    );

    let result_local = function_compiler.allocate_temp_local();
    assert!(
        function_compiler
            .emit_inline_user_function_summary_with_explicit_call_frame(
                &outer_user_function,
                &[first_value],
                &Expression::Undefined,
                result_local,
            )
            .expect("outer callback inline emission should compile"),
        "expected outer callback to inline"
    );

    assert_eq!(
        function_compiler
            .resolve_static_number_value(&Expression::Identifier("callCount".to_string())),
        Some(1.0)
    );
    assert_eq!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get("iter")
            .and_then(|binding| binding.static_index),
        Some(2)
    );
}

#[test]
fn consumes_second_delegate_next_inside_callback_context_without_restart() {
    let program = frontend::parse(
        r#"
            var log = [];
            var callCount = 0;
            var obj = {
              get [Symbol.iterator]() {
                log.push({ name: "get [Symbol.iterator]", thisValue: this });
                return function() {
                  log.push({ name: "call [Symbol.iterator]", thisValue: this, args: [...arguments] });
                  var nextCount = 0;
                  return {
                    name: "syncIterator",
                    get next() {
                      log.push({ name: "get next", thisValue: this });
                      return function() {
                        log.push({ name: "call next", thisValue: this, args: [...arguments] });
                        nextCount++;
                        if (nextCount == 1) {
                          return {
                            name: "next-result-1",
                            get value() {
                              log.push({ name: "get next value (1)", thisValue: this });
                              return "next-value-1";
                            },
                            get done() {
                              log.push({ name: "get next done (1)", thisValue: this });
                              return false;
                            }
                          };
                        }
                        return {
                          name: "next-result-2",
                          get value() {
                            log.push({ name: "get next value (2)", thisValue: this });
                            return "next-value-2";
                          },
                          get done() {
                            log.push({ name: "get next done (2)", thisValue: this });
                            return true;
                          }
                        };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                log.push({ name: "get [Symbol.asyncIterator]" });
                return null;
              }
            };
            class C {
              static async *gen() {
                callCount += 1;
                log.push({ name: "before yield*" });
                var v = yield* obj;
                log.push({ name: "after yield*", value: v });
                return "return-value";
              }
            }
            var gen = C.gen;
            var iter = gen();
            iter.next("arg1").then(function(v) {
              console.log("after-first", v.value, v.done, log.length, callCount);
              iter.next("arg2").then(function(v2) {
                console.log("after-second", v2.value, v2.done, log.length, callCount);
              });
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let outer_then_statement = match program.statements.last().expect("expected then statement") {
        Statement::Expression(expression) => expression,
        _ => panic!("expected then expression statement"),
    };
    let outer_handler = match outer_then_statement {
        Expression::Call { arguments, .. } => function_compiler
            .promise_handler_expression(arguments.first())
            .expect("expected outer handler expression"),
        _ => panic!("expected outer then call expression"),
    };
    let outer_user_function = function_compiler
        .resolve_user_function_from_expression(&outer_handler)
        .cloned()
        .expect("expected outer handler user function");

    assert!(matches!(
        function_compiler
            .consume_async_yield_delegate_generator_promise_outcome(
                &Expression::Identifier("iter".to_string()),
                "next",
                &[CallArgument::Expression(Expression::String(
                    "arg1".to_string()
                ))],
            )
            .expect("first delegate next should compile"),
        Some(StaticEvalOutcome::Value(_))
    ));
    assert_eq!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get("iter")
            .and_then(|binding| binding.static_index),
        Some(1)
    );
    let (delegate_next_name, delegate_snapshot_bindings) = match function_compiler
        .state
        .speculation
        .static_semantics
        .arrays
        .local_array_iterator_bindings
        .get("iter")
        .expect("expected iter binding after first next")
        .source
        .clone()
    {
        IteratorSourceKind::AsyncYieldDelegateGenerator {
            delegate_next_name,
            snapshot_bindings: Some(snapshot_bindings),
            ..
        } => (delegate_next_name, snapshot_bindings),
        IteratorSourceKind::SimpleGenerator { .. } => panic!("unexpected simple generator source"),
        IteratorSourceKind::StaticArray { .. } => panic!("unexpected static array source"),
        IteratorSourceKind::TypedArrayView { .. } => panic!("unexpected typed array source"),
        IteratorSourceKind::DirectArguments { .. } => panic!("unexpected direct arguments source"),
        IteratorSourceKind::AsyncYieldDelegateGenerator {
            snapshot_bindings: None,
            ..
        } => {
            panic!("missing async-yield-delegate snapshot bindings")
        }
    };

    let previous_user_function_name = function_compiler
        .state
        .speculation
        .execution_context
        .current_user_function_name
        .clone();
    function_compiler
        .state
        .speculation
        .execution_context
        .current_user_function_name = Some(outer_user_function.name.clone());
    let delegate_next_binding = function_compiler
        .resolve_function_binding_from_expression(&Expression::Identifier(
            delegate_next_name.clone(),
        ))
        .expect("expected delegate next function binding");
    let (snapshot_result, _updated_snapshot_bindings) = function_compiler
        .resolve_bound_snapshot_function_result_with_arguments(
            &delegate_next_binding,
            &delegate_snapshot_bindings,
            &[Expression::String("arg2".to_string())],
        )
        .expect("expected callback-context snapshot delegate next result");
    let snapshot_result_binding = function_compiler
        .resolve_object_binding_from_expression(&snapshot_result)
        .expect("expected snapshot result object binding");
    assert_eq!(
        object_binding_lookup_value(
            &snapshot_result_binding,
            &Expression::String("name".to_string()),
        ),
        Some(&Expression::String("next-result-2".to_string()))
    );
    let second_outcome = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[CallArgument::Expression(Expression::String(
                "arg2".to_string(),
            ))],
        )
        .expect("second delegate next should compile inside callback context");
    function_compiler
        .state
        .speculation
        .execution_context
        .current_user_function_name = previous_user_function_name;

    let second_value = match second_outcome {
        Some(StaticEvalOutcome::Value(value)) => value,
        _ => panic!("expected second delegate value outcome"),
    };
    assert_eq!(
        function_compiler.materialize_static_expression(&Expression::Member {
            object: Box::new(second_value.clone()),
            property: Box::new(Expression::String("done".to_string())),
        }),
        Expression::Bool(true)
    );
    assert_eq!(
        function_compiler.materialize_static_expression(&Expression::Member {
            object: Box::new(second_value),
            property: Box::new(Expression::String("value".to_string())),
        }),
        Expression::String("return-value".to_string())
    );
    assert_eq!(
        function_compiler
            .resolve_static_number_value(&Expression::Identifier("callCount".to_string())),
        Some(1.0)
    );
    assert_eq!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get("iter")
            .and_then(|binding| binding.static_index),
        Some(2)
    );
}

#[test]
fn consuming_actual_like_then_outcome_advances_nested_delegate_state() {
    let program = frontend::parse(
        r#"
            var log = [];
            var callCount = 0;
            var obj = {
              get [Symbol.iterator]() {
                log.push({ name: "get [Symbol.iterator]", thisValue: this });
                return function() {
                  log.push({ name: "call [Symbol.iterator]", thisValue: this, args: [...arguments] });
                  var nextCount = 0;
                  return {
                    name: "syncIterator",
                    get next() {
                      log.push({ name: "get next", thisValue: this });
                      return function() {
                        log.push({ name: "call next", thisValue: this, args: [...arguments] });
                        nextCount++;
                        if (nextCount == 1) {
                          return {
                            name: "next-result-1",
                            get value() {
                              log.push({ name: "get next value (1)", thisValue: this });
                              return "next-value-1";
                            },
                            get done() {
                              log.push({ name: "get next done (1)", thisValue: this });
                              return false;
                            }
                          };
                        }
                        return {
                          name: "next-result-2",
                          get value() {
                            log.push({ name: "get next value (2)", thisValue: this });
                            return "next-value-2";
                          },
                          get done() {
                            log.push({ name: "get next done (2)", thisValue: this });
                            return true;
                          }
                        };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                log.push({ name: "get [Symbol.asyncIterator]" });
                return null;
              }
            };
            class C {
              static async *gen() {
                callCount += 1;
                log.push({ name: "before yield*" });
                var v = yield* obj;
                log.push({ name: "after yield*", value: v });
                return "return-value";
              }
            }
            var gen = C.gen;
            var iter = gen();
            iter.next("arg1").then(function(v) {
              console.log("after-first", v.value, v.done, log.length, callCount);
              iter.next("arg2").then(function(v2) {
                console.log("after-second", v2.value, v2.done, log.length, callCount);
              });
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let then_statement = match program.statements.last().expect("expected then statement") {
        Statement::Expression(expression) => expression,
        _ => panic!("expected then expression statement"),
    };
    assert!(matches!(
        function_compiler
            .consume_immediate_promise_outcome(then_statement)
            .expect("outer immediate promise consumption should compile"),
        Some(StaticEvalOutcome::Value(Expression::Undefined))
    ));
    assert_eq!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get("iter")
            .and_then(|binding| binding.static_index),
        Some(2)
    );

    assert_eq!(
        function_compiler
            .resolve_static_number_value(&Expression::Identifier("callCount".to_string())),
        Some(1.0)
    );
    assert_eq!(
        function_compiler.materialize_static_expression(&Expression::Member {
            object: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("log".to_string())),
                property: Box::new(Expression::Number(11.0)),
            }),
            property: Box::new(Expression::String("name".to_string())),
        }),
        Expression::String("after yield*".to_string())
    );
    assert_eq!(
        function_compiler.materialize_static_expression(&Expression::Member {
            object: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("log".to_string())),
                property: Box::new(Expression::Number(11.0)),
            }),
            property: Box::new(Expression::String("value".to_string())),
        }),
        Expression::String("next-value-2".to_string())
    );
}

#[test]
fn consuming_actual_like_then_outcome_materializes_nested_completion_log_entries() {
    let program = frontend::parse(
        r#"
            var log = [];
            var callCount = 0;
            var obj = {
              get [Symbol.iterator]() {
                log.push({ name: "get [Symbol.iterator]", thisValue: this });
                return function() {
                  log.push({ name: "call [Symbol.iterator]", thisValue: this, args: [...arguments] });
                  var nextCount = 0;
                  return {
                    name: "syncIterator",
                    get next() {
                      log.push({ name: "get next", thisValue: this });
                      return function() {
                        log.push({ name: "call next", thisValue: this, args: [...arguments] });
                        nextCount++;
                        if (nextCount == 1) {
                          return {
                            name: "next-result-1",
                            get value() {
                              log.push({ name: "get next value (1)", thisValue: this });
                              return "next-value-1";
                            },
                            get done() {
                              log.push({ name: "get next done (1)", thisValue: this });
                              return false;
                            }
                          };
                        }
                        return {
                          name: "next-result-2",
                          get value() {
                            log.push({ name: "get next value (2)", thisValue: this });
                            return "next-value-2";
                          },
                          get done() {
                            log.push({ name: "get next done (2)", thisValue: this });
                            return true;
                          }
                        };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                log.push({ name: "get [Symbol.asyncIterator]" });
                return null;
              }
            };
            class C {
              static async *gen() {
                callCount += 1;
                log.push({ name: "before yield*" });
                var v = yield* obj;
                log.push({ name: "after yield*", value: v });
                return "return-value";
              }
            }
            var gen = C.gen;
            var iter = gen();
            iter.next("arg1").then(function(v) {
              console.log("after-first", v.value, v.done, log.length, callCount);
              iter.next("arg2").then(function(v2) {
                console.log("after-second", v2.value, v2.done, log.length, callCount);
              });
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    assert!(matches!(
        function_compiler
            .consume_immediate_promise_outcome(
                match program.statements.last().expect("expected then statement") {
                    Statement::Expression(expression) => expression,
                    _ => panic!("expected then expression statement"),
                }
            )
            .expect("outer immediate promise consumption should compile"),
        Some(StaticEvalOutcome::Value(Expression::Undefined))
    ));
    assert_eq!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get("iter")
            .and_then(|binding| binding.static_index),
        Some(2)
    );

    assert_eq!(
        function_compiler
            .resolve_static_number_value(&Expression::Identifier("callCount".to_string())),
        Some(1.0)
    );
    assert_eq!(
        function_compiler.materialize_static_expression(&Expression::Member {
            object: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("log".to_string())),
                property: Box::new(Expression::Number(11.0)),
            }),
            property: Box::new(Expression::String("name".to_string())),
        }),
        Expression::String("after yield*".to_string())
    );
}

#[test]
fn consumes_simple_async_generator_completion_value_for_tracked_iterator() {
    let program = frontend::parse(
        r#"
            class C {
              static async *gen() {
                yield { value: "x", done: false };
                return { value: "y", done: true };
              }
            }
            var iter = C.gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    let iter_initializer = match program
        .statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Var { name, value } if name == "iter" => Some(value.clone()),
            _ => None,
        }) {
        Some(value) => value,
        None => panic!("expected iter initializer"),
    };
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }
    assert!(
        function_compiler
            .resolve_simple_generator_source(&iter_initializer)
            .is_some(),
        "expected iter initializer to resolve as simple generator source after class setup"
    );
    let Some(binding_name) = function_compiler.resolve_local_array_iterator_binding_name("iter")
    else {
        panic!("expected iter iterator binding name");
    };
    let Some(IteratorSourceKind::SimpleGenerator {
        is_async,
        completion_effects,
        ..
    }) = function_compiler
        .state
        .speculation
        .static_semantics
        .arrays
        .local_array_iterator_bindings
        .get(&binding_name)
        .map(|binding| &binding.source)
    else {
        panic!("expected iter simple generator source");
    };
    assert!(*is_async);
    let Statement::Yield { .. } = &program.functions[0].body[0] else {
        panic!("expected async generator body to begin with yield");
    };
    assert!(
        completion_effects.is_empty(),
        "expected yield-only async generator binding to carry no completion effects",
    );

    let first = function_compiler
        .consume_simple_async_generator_next_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            &[],
        )
        .expect("first outcome should evaluate")
        .expect("first outcome should exist");
    let second = function_compiler
        .consume_simple_async_generator_next_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            &[],
        )
        .expect("second outcome should evaluate")
        .expect("second outcome should exist");

    assert!(matches!(
        first,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(done_key), value: Expression::Bool(false) },
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(value_key), value: Expression::Object(value_entries) },
                ] if done_key == "done"
                    && value_key == "value"
                    && matches!(
                        value_entries.as_slice(),
                        [
                            crate::ir::hir::ObjectEntry::Data { key: Expression::String(inner_value_key), value: Expression::String(inner_value) },
                            crate::ir::hir::ObjectEntry::Data { key: Expression::String(inner_done_key), value: Expression::Bool(false) },
                        ] if inner_value_key == "value"
                            && inner_value == "x"
                            && inner_done_key == "done"
                    )
            )
    ));
    assert!(matches!(
        second,
        StaticEvalOutcome::Value(Expression::Object(ref entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(done_key), value: Expression::Bool(true) },
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(value_key), value: Expression::Object(value_entries) },
                ] if done_key == "done"
                    && value_key == "value"
                    && matches!(
                        value_entries.as_slice(),
                        [
                            crate::ir::hir::ObjectEntry::Data { key: Expression::String(inner_value_key), value: Expression::String(inner_value) },
                            crate::ir::hir::ObjectEntry::Data { key: Expression::String(inner_done_key), value: Expression::Bool(true) },
                        ] if inner_value_key == "value"
                            && inner_value == "y"
                            && inner_done_key == "done"
                    )
            )
    ));
}

#[test]
fn materializes_object_property_from_runtime_array_slot_member_expression() {
    let program = frontend::parse(
        r#"
            var log = [];
            log.push({ name: "a" });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let expression = Expression::Member {
        object: Box::new(Expression::Member {
            object: Box::new(Expression::Identifier("log".to_string())),
            property: Box::new(Expression::Number(0.0)),
        }),
        property: Box::new(Expression::String("name".to_string())),
    };
    assert_eq!(
        function_compiler.materialize_static_expression(&expression),
        Expression::String("a".to_string())
    );
}

#[test]
fn infers_typeof_for_materialized_nested_runtime_array_slot_member_expression() {
    let program = frontend::parse(
        r#"
            var log = [];
            log.push({ args: [Boolean] });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let expression = Expression::Member {
        object: Box::new(Expression::Member {
            object: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("log".to_string())),
                property: Box::new(Expression::Number(0.0)),
            }),
            property: Box::new(Expression::String("args".to_string())),
        }),
        property: Box::new(Expression::Number(0.0)),
    };
    assert!(matches!(
        function_compiler.materialize_static_expression(&expression),
        Expression::Member { .. } | Expression::Identifier(_)
    ));
    assert!(matches!(
        function_compiler.infer_typeof_operand_kind(&expression),
        Some(StaticValueKind::Function)
    ));
}

#[test]
fn materializes_runtime_array_object_properties_after_inline_getter_side_effects() {
    let program = frontend::parse(
        r#"
            var log = [];
            var result = {
                name: "next-result-1",
                get done() {
                    log.push({ name: "get done", thisValue: this });
                    return false;
                }
            };
            result.done;
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let log_name_expression = Expression::Member {
        object: Box::new(Expression::Member {
            object: Box::new(Expression::Identifier("log".to_string())),
            property: Box::new(Expression::Number(0.0)),
        }),
        property: Box::new(Expression::String("name".to_string())),
    };
    assert_eq!(
        function_compiler.materialize_static_expression(&log_name_expression),
        Expression::String("get done".to_string())
    );

    let this_name_expression = Expression::Member {
        object: Box::new(Expression::Member {
            object: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("log".to_string())),
                property: Box::new(Expression::Number(0.0)),
            }),
            property: Box::new(Expression::String("thisValue".to_string())),
        }),
        property: Box::new(Expression::String("name".to_string())),
    };
    assert_eq!(
        function_compiler.materialize_static_expression(&this_name_expression),
        Expression::String("next-result-1".to_string())
    );
}

#[test]
fn resolves_print_arguments_after_inline_getter_side_effects() {
    let program = frontend::parse(
        r#"
            var log = [];
            var result = {
                name: "next-result-1",
                get done() {
                    log.push({ name: "get done", thisValue: this });
                    return false;
                }
            };
            result.done;
            console.log("log-name", log[0].name);
            console.log("log-this-name", log[0].thisValue.name);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..3] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let Statement::Print {
        values: first_print_values,
    } = &program.statements[3]
    else {
        panic!("expected first print statement");
    };
    assert_eq!(
        function_compiler.resolve_static_string_value(&first_print_values[1]),
        Some("get done".to_string())
    );

    let Statement::Print {
        values: second_print_values,
    } = &program.statements[4]
    else {
        panic!("expected second print statement");
    };
    assert_eq!(
        function_compiler.resolve_static_string_value(&second_print_values[1]),
        Some("next-result-1".to_string())
    );
}

#[test]
fn consumes_actual_like_then_outcome_with_registered_capture_bindings() {
    let program = frontend::parse(
        r#"
            var log = [];
            var obj = {
              get [Symbol.iterator]() {
                log.push({ name: "get [Symbol.iterator]", thisValue: this });
                return function() {
                  log.push({ name: "call [Symbol.iterator]", thisValue: this, args: [...arguments] });
                  var nextCount = 0;
                  return {
                    name: "syncIterator",
                    get next() {
                      log.push({ name: "get next", thisValue: this });
                      return function() {
                        log.push({ name: "call next", thisValue: this, args: [...arguments] });
                        nextCount++;
                        if (nextCount == 1) {
                          return {
                            name: "next-result-1",
                            get value() { log.push({ name: "get next value (1)", thisValue: this }); return "next-value-1"; },
                            get done() { log.push({ name: "get next done (1)", thisValue: this }); return false; }
                          };
                        }
                        return {
                          name: "next-result-2",
                          get value() { log.push({ name: "get next value (2)", thisValue: this }); return "next-value-2"; },
                          get done() { log.push({ name: "get next done (2)", thisValue: this }); return true; }
                        };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                log.push({ name: "get [Symbol.asyncIterator]" });
                return null;
              }
            };
            class C { static async *gen() { yield* obj; } }
            var iter = C.gen();
            iter.next("next-arg-1").then(function(v) {});
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let Statement::Expression(then_expression) =
        program.statements.last().expect("expected then statement")
    else {
        panic!("expected then expression statement");
    };
    assert!(matches!(
        function_compiler
            .consume_immediate_promise_outcome(then_expression)
            .expect("actual-like immediate promise outcome should compile"),
        Some(StaticEvalOutcome::Value(Expression::Undefined))
    ));
}

#[test]
fn materializes_then_handler_for_async_generator_next_call_to_callback_function() {
    let program = frontend::parse(
        r#"
            var obj = { [Symbol.iterator]() { return { get next() { return function() { return { value: 1, done: false }; }; } }; } };
            class C { static async *gen() { yield* obj; } }
            var iter = C.gen();
            iter.next('x').then(function() { console.log('hit'); }, function() { console.log('rej'); });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    let Statement::Expression(Expression::Call { arguments, .. }) =
        program.statements.last().expect("expected statements")
    else {
        panic!("expected top-level call expression");
    };
    let handler = function_compiler.promise_handler_expression(arguments.first());
    assert!(
        matches!(handler, Some(Expression::Identifier(ref name)) if name.starts_with("__ayy_fnexpr_")),
        "expected materialized callback function identifier, got {handler:?}"
    );
    let handler_expression = handler.expect("handler should exist");
    assert!(
        function_compiler
            .resolve_user_function_from_expression(&handler_expression)
            .is_some(),
        "expected handler identifier to resolve to a user function binding"
    );
}

#[test]
fn consumes_immediate_promise_outcome_for_yield_star_async_iterator_next_callback() {
    let program = frontend::parse(
        r#"
            var log = [];
            var obj = {
              [Symbol.asyncIterator]() {
                return {
                  get next() {
                    log.push("get next");
                    return function() {
                      log.push("call next");
                      return { value: "next-value-1", done: false };
                    };
                  }
                };
              }
            };
            class C {
              async *gen() {
                log.push("before yield*");
                yield* obj;
              }
            }
            var iter = C.prototype.gen();
            iter.next().then(function(v) {
              console.log("next", v.value, v.done, log.length);
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let Statement::Expression(then_expression) =
        program.statements.last().expect("expected then statement")
    else {
        panic!("expected then expression statement");
    };
    let Expression::Call { arguments, .. } = then_expression else {
        panic!("expected then call expression");
    };
    assert!(
        function_compiler
            .promise_handler_expression(arguments.first())
            .is_some(),
        "expected then fulfillment handler to materialize",
    );
    let outcome = function_compiler
        .consume_immediate_promise_outcome(then_expression)
        .expect("immediate promise outcome should compile");
    match outcome {
        Some(StaticEvalOutcome::Value(Expression::Undefined)) => {}
        Some(StaticEvalOutcome::Value(_)) => {
            panic!("expected callback execution instead of value passthrough")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(name))) => {
            panic!("expected fulfilled promise outcome, got named error {name}")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(_))) => {
            panic!("expected fulfilled promise outcome, got thrown value")
        }
        None => panic!("expected immediate promise outcome"),
    }
}

#[test]
fn consumes_direct_async_yield_delegate_next_outcome_for_async_iterator() {
    let program = frontend::parse(
        r#"
            var log = [];
            var obj = {
              [Symbol.asyncIterator]() {
                return {
                  get next() {
                    log.push("get next");
                    return function() {
                      log.push("call next");
                      return { value: "next-value-1", done: false };
                    };
                  }
                };
              }
            };
            class C {
              async *gen() {
                log.push("before yield*");
                yield* obj;
              }
            }
            var iter = C.prototype.gen();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    assert!(
        matches!(
            function_compiler
                .state
                .speculation
                .static_semantics
                .arrays
                .local_array_iterator_bindings
                .get("iter")
                .map(|binding| &binding.source),
            Some(IteratorSourceKind::AsyncYieldDelegateGenerator { .. })
        ),
        "expected iter binding to remain async-yield-delegate based",
    );
    let outcome = function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "next",
            &[],
        )
        .expect("delegate promise outcome should compile");
    match outcome {
        Some(StaticEvalOutcome::Value(Expression::Object(_))) => {}
        Some(StaticEvalOutcome::Value(_)) => {
            panic!("expected iterator result object")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(name))) => {
            panic!("expected fulfilled delegate outcome, got named error {name}")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(Expression::Identifier(name)))) => {
            panic!("expected fulfilled delegate outcome, got identifier throw {name}")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(Expression::Call { .. }))) => {
            panic!("expected fulfilled delegate outcome, got call-expression throw")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(_))) => {
            panic!("expected fulfilled delegate outcome, got raw thrown value")
        }
        None => panic!("expected async delegate outcome"),
    }
}

#[test]
fn consumes_immediate_promise_outcome_for_async_iterator_next_with_delegate_return() {
    let program = frontend::parse(
        r#"
            var obj = {
              [Symbol.asyncIterator]() {
                var returnCount = 0;
                return {
                  get next() {
                    return function() {
                      return { value: "next-value-1", done: false };
                    };
                  },
                  get return() {
                    return function(arg) {
                      returnCount++;
                      if (returnCount === 1) {
                        return {
                          get then() {
                            return function(resolve) {
                              resolve({
                                get value() { return "return-value-1"; },
                                get done() { return false; }
                              });
                            };
                          }
                        };
                      }
                      return {
                        get then() {
                          return function(resolve) {
                            resolve({
                              get value() { return "return-value-2"; },
                              get done() { return true; }
                            });
                          };
                        }
                      };
                    };
                  }
                };
              }
            };
            class C {
              async *gen() {
                yield* obj;
              }
            }
            var iter = C.prototype.gen();
            iter.next().then(function(v) {
              console.log("next", v.value, v.done);
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let Statement::Expression(then_expression) =
        program.statements.last().expect("expected then statement")
    else {
        panic!("expected then expression statement");
    };
    let outcome = function_compiler
        .consume_immediate_promise_outcome(then_expression)
        .expect("immediate promise outcome should compile");
    match outcome {
        Some(StaticEvalOutcome::Value(Expression::Undefined)) => {}
        Some(StaticEvalOutcome::Value(_)) => {
            panic!("expected callback execution instead of value passthrough")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(name))) => {
            panic!("expected fulfilled next outcome, got named error {name}")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(_))) => {
            panic!("expected fulfilled next outcome, got thrown value")
        }
        None => panic!("expected immediate promise outcome"),
    }
}

#[test]
fn consumes_immediate_promise_outcome_for_async_iterator_return_callback_chain() {
    let program = frontend::parse(
        r#"
            var obj = {
              [Symbol.asyncIterator]() {
                var returnCount = 0;
                return {
                  get next() {
                    return function() {
                      return { value: "next-value-1", done: false };
                    };
                  },
                  get return() {
                    return function(arg) {
                      returnCount++;
                      if (returnCount === 1) {
                        return {
                          get then() {
                            return function(resolve) {
                              resolve({
                                get value() { return "return-value-1"; },
                                get done() { return false; }
                              });
                            };
                          }
                        };
                      }
                      return {
                        get then() {
                          return function(resolve) {
                            resolve({
                              get value() { return "return-value-2"; },
                              get done() { return true; }
                            });
                          };
                        }
                      };
                    };
                  }
                };
              }
            };
            class C {
              async *gen() {
                yield* obj;
              }
            }
            var iter = C.prototype.gen();
            iter.next().then(function(v) {
              iter.return("return-arg-1").then(function(v2) {
                iter.return("return-arg-2").then(function(v3) {
                  console.log("return2", v3.value, v3.done);
                });
                console.log("return1", v2.value, v2.done);
              });
              console.log("next", v.value, v.done);
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let Statement::Expression(then_expression) =
        program.statements.last().expect("expected then statement")
    else {
        panic!("expected then expression statement");
    };
    let outcome = function_compiler
        .consume_immediate_promise_outcome(then_expression)
        .expect("immediate promise outcome should compile");
    match outcome {
        Some(StaticEvalOutcome::Value(Expression::Undefined)) => {}
        Some(StaticEvalOutcome::Value(_)) => {
            panic!("expected callback-chain execution instead of value passthrough")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(name))) => {
            panic!("expected fulfilled callback-chain outcome, got named error {name}")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(_))) => {
            panic!("expected fulfilled callback-chain outcome, got thrown value")
        }
        None => panic!("expected immediate promise outcome"),
    }
}

#[test]
fn consumes_immediate_promise_outcome_for_sync_iterator_return_callback_chain() {
    let program = frontend::parse(
        r#"
            var log = [];
            var obj = {
              [Symbol.iterator]() {
                var returnCount = 0;
                return {
                  get next() {
                    log.push("get next");
                    return function() {
                      return { value: "next-value-1", done: false };
                    };
                  },
                  get return() {
                    log.push("get return");
                    return function(arg) {
                      log.push("call return:" + arg);
                      returnCount++;
                      if (returnCount === 1) {
                        return {
                          get value() {
                            log.push("get return value (1)");
                            return "return-value-1";
                          },
                          get done() {
                            log.push("get return done (1)");
                            return false;
                          }
                        };
                      }
                      return {
                        get value() {
                          log.push("get return value (2)");
                          return "return-value-2";
                        },
                        get done() {
                          log.push("get return done (2)");
                          return true;
                        }
                      };
                    };
                  }
                };
              }
            };
            class C {
              async *gen() {
                log.push("before yield*");
                yield* obj;
              }
            }
            var iter = C.prototype.gen();
            iter.next().then(function(v) {
              iter.return("return-arg-1").then(function(v2) {
                iter.return("return-arg-2").then(function(v3) {
                  console.log("return2", v3.value, v3.done, log.length);
                });
                console.log("return1", v2.value, v2.done, log.length);
              });
              console.log("next", v.value, v.done, log.length);
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let Statement::Expression(then_expression) =
        program.statements.last().expect("expected then statement")
    else {
        panic!("expected then expression statement");
    };
    let outcome = function_compiler
        .consume_immediate_promise_outcome(then_expression)
        .expect("immediate promise outcome should compile");
    match outcome {
        Some(StaticEvalOutcome::Value(Expression::Undefined)) => {}
        Some(StaticEvalOutcome::Value(_)) => {
            panic!("expected callback-chain execution instead of value passthrough")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(name))) => {
            panic!("expected fulfilled callback-chain outcome, got named error {name}")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(_))) => {
            panic!("expected fulfilled callback-chain outcome, got thrown value")
        }
        None => panic!("expected immediate promise outcome"),
    }
}

#[test]
fn consumes_immediate_promise_outcome_for_async_iterator_return_then_after_next() {
    let program = frontend::parse(
        r#"
            var obj = {
              [Symbol.asyncIterator]() {
                var returnCount = 0;
                return {
                  get next() {
                    return function() {
                      return { value: "next-value-1", done: false };
                    };
                  },
                  get return() {
                    return function(arg) {
                      returnCount++;
                      if (returnCount === 1) {
                        return {
                          get then() {
                            return function(resolve) {
                              resolve({
                                get value() { return "return-value-1"; },
                                get done() { return false; }
                              });
                            };
                          }
                        };
                      }
                      return {
                        get then() {
                          return function(resolve) {
                            resolve({
                              get value() { return "return-value-2"; },
                              get done() { return true; }
                            });
                          };
                        }
                      };
                    };
                  }
                };
              }
            };
            class C {
              async *gen() {
                yield* obj;
              }
            }
            var iter = C.prototype.gen();
            iter.return("return-arg-1").then(function(v) {
              console.log("return1", v.value, v.done);
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    assert!(matches!(
        function_compiler
            .consume_async_yield_delegate_generator_promise_outcome(
                &Expression::Identifier("iter".to_string()),
                "next",
                &[],
            )
            .expect("next outcome should compile"),
        Some(StaticEvalOutcome::Value(_))
    ));
    assert!(
        matches!(
            function_compiler
                .state
                .speculation
                .static_semantics
                .arrays
                .local_array_iterator_bindings
                .get("iter")
                .map(|binding| &binding.source),
            Some(IteratorSourceKind::AsyncYieldDelegateGenerator { .. })
        ),
        "expected iter binding to remain async-yield-delegate based after first next",
    );
    let (delegate_iterator_name, delegate_snapshot_bindings, current_function_name) =
        match function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get("iter")
            .expect("expected iter binding after first next")
            .source
            .clone()
        {
            IteratorSourceKind::AsyncYieldDelegateGenerator {
                plan,
                delegate_iterator_name,
                snapshot_bindings: Some(snapshot_bindings),
                ..
            } => (
                delegate_iterator_name,
                snapshot_bindings,
                Some(plan.function_name),
            ),
            IteratorSourceKind::AsyncYieldDelegateGenerator {
                snapshot_bindings: None,
                ..
            } => panic!("missing async-yield-delegate snapshot bindings after next"),
            _ => panic!("expected async-yield-delegate iterator source after next"),
        };
    let mut return_snapshot_bindings = delegate_snapshot_bindings.clone();
    let return_method = function_compiler
        .evaluate_bound_snapshot_expression(
            &Expression::Member {
                object: Box::new(Expression::Identifier(delegate_iterator_name.clone())),
                property: Box::new(Expression::String("return".to_string())),
            },
            &mut return_snapshot_bindings,
            current_function_name.as_deref(),
        )
        .expect("expected snapshot return method after next");
    let return_binding = function_compiler
        .resolve_function_binding_from_expression(&return_method)
        .expect("expected snapshot return binding after next");
    let (return_step_result, _) = function_compiler
        .resolve_bound_snapshot_function_result_with_arguments_and_this(
            &return_binding,
            &delegate_snapshot_bindings,
            &[Expression::String("return-arg-1".to_string())],
            &Expression::Identifier(delegate_iterator_name.clone()),
        )
        .expect("expected snapshot return step result after next");
    let Expression::Object(return_step_entries) = &return_step_result else {
        panic!(
            "expected snapshot return step result object after next: {:?}",
            return_step_result
        );
    };
    let mut then_snapshot_bindings = HashMap::new();
    let then_value = function_compiler
        .resolve_bound_snapshot_object_member_value(
            return_step_entries,
            &Expression::String("then".to_string()),
            &mut then_snapshot_bindings,
            current_function_name.as_deref(),
        )
        .expect("expected snapshot then getter result after next");
    let then_binding = function_compiler
        .resolve_function_binding_from_expression(&then_value)
        .expect("expected snapshot then binding after next");
    assert!(
        function_compiler
            .resolve_bound_snapshot_thenable_outcome(
                &then_binding,
                &return_step_result,
                &mut then_snapshot_bindings,
                current_function_name.as_deref(),
            )
            .is_some(),
        "expected snapshot thenable outcome after next: then={:?} bindings={:?}",
        then_value,
        then_snapshot_bindings,
    );
    assert!(
        function_compiler
            .resolve_static_await_resolution_outcome(&return_step_result)
            .is_some(),
        "expected raw snapshot return step result to settle after next: {:?}",
        return_step_result,
    );
    match function_compiler
        .consume_async_yield_delegate_generator_promise_outcome(
            &Expression::Identifier("iter".to_string()),
            "return",
            &[CallArgument::Expression(Expression::String(
                "return-arg-1".to_string(),
            ))],
        )
        .expect("direct return outcome should compile")
    {
        Some(StaticEvalOutcome::Value(_)) => {}
        Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(name))) => {
            panic!("expected direct return outcome, got named error {name}")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(_))) => {
            panic!("expected direct return outcome, got thrown value")
        }
        None => panic!("expected direct return outcome"),
    }

    let Statement::Expression(then_expression) =
        program.statements.last().expect("expected then statement")
    else {
        panic!("expected then expression statement");
    };
    let outcome = function_compiler
        .consume_immediate_promise_outcome(then_expression)
        .expect("immediate promise outcome should compile");
    match outcome {
        Some(StaticEvalOutcome::Value(Expression::Undefined)) => {}
        Some(StaticEvalOutcome::Value(_)) => {
            panic!("expected callback execution instead of value passthrough")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(name))) => {
            panic!("expected fulfilled return outcome, got named error {name}")
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(_))) => {
            panic!("expected fulfilled return outcome, got thrown value")
        }
        None => panic!("expected immediate promise outcome"),
    }
}

#[test]
fn detects_nested_then_callback_that_captures_outer_parameter() {
    let program = frontend::parse(
        r#"
            var obj = {
              get [Symbol.iterator]() {
                return function() {
                  var nextCount = 0;
                  return {
                    get next() {
                      return function() {
                        nextCount++;
                        if (nextCount === 1) {
                          return { value: "next-value-1", done: false };
                        }
                        return { value: "next-value-2", done: true };
                      };
                    }
                  };
                };
              },
              get [Symbol.asyncIterator]() {
                return null;
              }
            };
            class C {
              static async *gen() {
                var value = yield* obj;
                return value;
              }
            }
            var iter = C.gen();
            iter.next("first").then(first => {
              iter.next("second").then(second => {
                console.log(first.value, second.value);
              });
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let outer_callback = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| function.params == ["first"])
        .cloned()
        .expect("expected outer then callback");
    let nested_callback = compiler
        .state
        .function_registry
        .catalog
        .user_functions
        .iter()
        .find(|function| function.params == ["second"])
        .cloned()
        .expect("expected nested then callback");

    let nested_captures = compiler
        .state
        .function_registry
        .analysis
        .user_function_capture_bindings
        .get(&nested_callback.name)
        .expect("expected nested callback capture bindings");
    assert!(
        nested_captures.contains_key("first"),
        "expected nested callback to capture first, got {nested_captures:#?}"
    );

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    assert!(
        function_compiler.user_function_references_captured_user_function(&outer_callback),
        "expected outer callback to be treated as referencing a captured user function"
    );
}

#[test]
fn resolves_simple_generator_source_for_class_expression_prototype_method_call() {
    let program = frontend::parse(
        r#"
            var callCount = 0;
            var C = class {
              *method() {
                callCount = arguments.length === 2 && arguments[0] === 42 && arguments[1] === "TC39" ? 1 : -1;
              }
            };
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");

    let method_callee = Expression::Member {
        object: Box::new(Expression::Member {
            object: Box::new(Expression::Identifier("C".to_string())),
            property: Box::new(Expression::String("prototype".to_string())),
        }),
        property: Box::new(Expression::String("method".to_string())),
    };
    let method_call = Expression::Call {
        callee: Box::new(method_callee.clone()),
        arguments: vec![
            CallArgument::Expression(Expression::Number(42.0)),
            CallArgument::Expression(Expression::String("TC39".to_string())),
        ],
    };

    let Some(LocalFunctionBinding::User(function_name)) =
        function_compiler.resolve_function_binding_from_expression(&method_callee)
    else {
        panic!("expected class expression prototype method binding");
    };
    let user_function = function_compiler
        .backend
        .function_registry
        .catalog
        .user_function_map
        .get(&function_name)
        .cloned()
        .expect("expected method user function");
    let function_declaration = function_compiler
        .backend
        .function_registry
        .catalog
        .registered_function_declarations
        .iter()
        .find(|function| function.name == function_name)
        .cloned()
        .expect("expected method declaration");
    let substituted_body = function_compiler
        .substitute_simple_generator_statements_with_call_frame_bindings(
            &function_declaration.body,
            &user_function,
            false,
            &mut vec![
                Expression::Number(42.0),
                Expression::String("TC39".to_string()),
            ],
            &mut vec![
                Expression::Number(42.0),
                Expression::String("TC39".to_string()),
            ],
            &Expression::Undefined,
        )
        .expect("expected substituted method body");
    let mut steps = Vec::new();
    let mut effects = Vec::new();
    function_compiler
        .analyze_simple_generator_statements(&substituted_body, false, &mut steps, &mut effects)
        .expect("expected class expression method body to analyze as simple generator");
    assert!(
        function_compiler
            .resolve_simple_generator_source(&method_call)
            .is_some(),
        "expected class expression prototype method call to resolve as simple generator source"
    );
}

#[test]
fn preserves_class_expression_generator_source_after_emitting_assignment() {
    let program = frontend::parse(
        r#"
            var callCount = 0;
            var C = class {
              *method() {
                callCount = arguments.length === 2 && arguments[0] === 42 && arguments[1] === "TC39" ? 1 : -1;
              }
            };
            C.prototype.method(42, "TC39").next();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..2] {
        function_compiler
            .emit_statement(statement)
            .expect("leading statements should emit");
    }

    let Statement::Expression(Expression::Call { callee, arguments }) = &program.statements[2]
    else {
        panic!("expected trailing next call");
    };
    assert!(arguments.is_empty(), "expected zero-argument next call");
    let Expression::Member {
        object: method_call,
        property,
    } = callee.as_ref()
    else {
        panic!("expected next call member callee");
    };
    assert!(
        matches!(property.as_ref(), Expression::String(name) if name == "next"),
        "expected next call property"
    );
    let method_callee = match method_call.as_ref() {
        Expression::Call { callee, .. } => callee.as_ref(),
        _ => unreachable!("constructed above"),
    };
    let method_binding = function_compiler.resolve_function_binding_from_expression(method_callee);

    assert!(
        method_binding.is_some(),
        "expected emitted class expression assignment to preserve method binding"
    );
    assert!(
        function_compiler
            .resolve_simple_generator_source(method_call.as_ref())
            .is_some(),
        "expected emitted class expression assignment to preserve simple generator source"
    );
}

#[test]
fn resolves_returned_class_expression_static_field_binding_from_init_call() {
    let program = frontend::parse(
        r#"
            let C = class {
              static [1 + 1] = 2;
            };
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    let Statement::Let {
        value: Expression::Call { callee, arguments },
        ..
    } = &program.statements[0]
    else {
        panic!("expected class expression initializer call");
    };
    let object_binding = function_compiler
        .resolve_returned_object_binding_from_call(callee, arguments)
        .expect("expected returned object binding from class init call");
    assert_eq!(
        object_binding_lookup_value(&object_binding, &Expression::String("2".to_string())),
        Some(&Expression::Number(2.0)),
    );
}

#[test]
fn resolves_new_object_binding_for_class_expression_instance_field() {
    let program = frontend::parse(
        r#"
            let C = class {
              x = 2;
            };
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    function_compiler
        .emit_statement(&program.statements[0])
        .expect("class expression binding should emit");

    let object_binding = function_compiler
        .resolve_user_constructor_object_binding_from_new(
            &Expression::Identifier("C".to_string()),
            &[],
        )
        .expect("expected new C object binding");
    assert_eq!(
        object_binding_lookup_value(&object_binding, &Expression::String("x".to_string())),
        Some(&Expression::Number(2.0)),
    );
}

#[test]
fn resolves_new_object_binding_for_class_expression_computed_instance_field() {
    let program = frontend::parse(
        r#"
            let C = class {
              [1 + 1] = 2;
            };
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    function_compiler
        .emit_statement(&program.statements[0])
        .expect("class expression binding should emit");

    let object_binding = function_compiler
        .resolve_user_constructor_object_binding_from_new(
            &Expression::Identifier("C".to_string()),
            &[],
        )
        .expect("expected new C object binding");
    assert_eq!(
        object_binding_lookup_value(&object_binding, &Expression::String("2".to_string())),
        Some(&Expression::Number(2.0)),
    );
}

#[test]
fn resolves_new_object_binding_for_derived_class_super_constructor() {
    let program = frontend::parse(
        r#"
            class Base {
              constructor(x) {
                this.foobar = x;
              }
            }

            class Subclass extends Base {
              constructor(x) {
                super(x);
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let object_binding = function_compiler
        .resolve_user_constructor_object_binding_from_new(
            &Expression::Identifier("Subclass".to_string()),
            &[CallArgument::Expression(Expression::Number(1.0))],
        )
        .expect("expected derived constructor object binding");
    let foobar =
        object_binding_lookup_value(&object_binding, &Expression::String("foobar".to_string()))
            .cloned();
    assert_eq!(foobar, Some(Expression::Number(1.0)));
}

#[test]
fn resolves_new_object_binding_for_derived_constructor_with_second_super_try_path() {
    let program = frontend::parse(
        r#"
            class Base {
              constructor(a, b) {
                var o = new Object();
                o.prp = a + b;
                return o;
              }
            }

            class Subclass2 extends Base {
              constructor(x) {
                super(1, 2);

                if (x < 0) return;

                var called = false;
                function tmp() { called = true; return 3; }
                var exn = null;
                try {
                  super(tmp(), 4);
                } catch (e) {
                  exn = e;
                }
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let positive_binding = function_compiler
        .resolve_user_constructor_object_binding_from_new(
            &Expression::Identifier("Subclass2".to_string()),
            &[CallArgument::Expression(Expression::Number(1.0))],
        )
        .expect("expected derived constructor object binding through second super try path");
    assert_eq!(
        object_binding_lookup_value(&positive_binding, &Expression::String("prp".to_string())),
        Some(&Expression::Number(3.0)),
    );

    let negative_binding = function_compiler
        .resolve_user_constructor_object_binding_from_new(
            &Expression::Identifier("Subclass2".to_string()),
            &[CallArgument::Expression(Expression::Number(-1.0))],
        )
        .expect("expected derived constructor object binding through early return");
    assert_eq!(
        object_binding_lookup_value(&negative_binding, &Expression::String("prp".to_string())),
        Some(&Expression::Number(3.0)),
    );
}

#[test]
fn resolves_function_binding_and_object_prototype_for_class_expression_binding() {
    let program = frontend::parse(
        r#"
            let C = class C {};
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    function_compiler
        .emit_statement(&program.statements[0])
        .expect("class expression binding should emit");

    assert!(
        function_compiler
            .resolve_function_binding_from_expression(&Expression::Identifier("C".to_string()))
            .is_some(),
        "expected class expression binding to resolve as function"
    );
    assert_eq!(
        function_compiler
            .resolve_static_object_prototype_expression(&Expression::Identifier("C".to_string())),
        Some(Expression::Member {
            object: Box::new(Expression::Identifier("Function".to_string())),
            property: Box::new(Expression::String("prototype".to_string())),
        }),
    );
}

#[test]
fn resolves_static_object_prototype_expression_for_class_extends_constructor_binding() {
    let program = frontend::parse(
        r#"
            class C {}
            class D extends (0, C) {}
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("class declaration should emit");
    }

    assert!(function_compiler.binding_name_is_global("D"));
    assert_eq!(
        function_compiler
            .backend
            .global_semantics
            .values
            .object_prototype_bindings
            .get("D")
            .cloned(),
        Some(Expression::Identifier("C".to_string())),
    );
    assert_eq!(
        function_compiler
            .resolve_static_object_prototype_expression(&Expression::Identifier("D".to_string())),
        Some(Expression::Identifier("C".to_string())),
    );
    assert_eq!(
        function_compiler.resolve_static_object_prototype_expression(&Expression::Member {
            object: Box::new(Expression::Identifier("D".to_string())),
            property: Box::new(Expression::String("prototype".to_string())),
        }),
        Some(Expression::Member {
            object: Box::new(Expression::Identifier("C".to_string())),
            property: Box::new(Expression::String("prototype".to_string())),
        }),
    );
}

#[test]
fn resolves_new_object_binding_for_class_expression_identifier_computed_instance_field() {
    let program = frontend::parse(
        r#"
            let x = 1;
            let C = class {
              [x] = 2;
            };
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let object_binding = function_compiler
        .resolve_user_constructor_object_binding_from_new(
            &Expression::Identifier("C".to_string()),
            &[],
        )
        .expect("expected new C object binding");
    assert_eq!(
        object_binding_lookup_value(&object_binding, &Expression::String("1".to_string())),
        Some(&Expression::Number(2.0)),
    );
}

#[test]
fn resolves_identifier_bound_class_expression_computed_stringified_field_reads() {
    let program = frontend::parse(
        r#"
            let x = 1;
            let C = class {
              [x] = '2';
              static [x] = '2';
            };
            let c = new C();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let c_binding = function_compiler
        .resolve_object_binding_from_expression(&Expression::Identifier("c".to_string()))
        .expect("expected c object binding");
    assert_eq!(
        function_compiler.resolve_object_binding_property_value(
            &c_binding,
            &Expression::Call {
                callee: Box::new(Expression::Identifier("String".to_string())),
                arguments: vec![CallArgument::Expression(Expression::Identifier(
                    "x".to_string()
                ))],
            },
        ),
        Some(Expression::String("2".to_string())),
    );
}

#[test]
fn emits_fresh_class_expression_generator_next_call_effects() {
    let program = frontend::parse(
        r#"
            var callCount = 0;
            var C = class {
              *method() {
                callCount = arguments.length === 2 && arguments[0] === 42 && arguments[1] === "TC39" ? 1 : -1;
              }
            };
            C.prototype.method(42, "TC39").next();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..2] {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }
    let Statement::Expression(Expression::Call { callee, arguments }) = &program.statements[2]
    else {
        panic!("expected trailing next call");
    };
    assert!(arguments.is_empty(), "expected zero-argument next call");
    let Expression::Member {
        object: method_call,
        property,
    } = callee.as_ref()
    else {
        panic!("expected next call member callee");
    };
    assert!(
        matches!(property.as_ref(), Expression::String(name) if name == "next"),
        "expected next call property"
    );
    assert!(
        function_compiler
            .resolve_simple_generator_source(method_call.as_ref())
            .is_some(),
        "expected class expression method call to resolve as simple generator source"
    );
    assert!(
        function_compiler
            .emit_fresh_simple_generator_next_call(method_call.as_ref(), &[])
            .expect("fresh next helper should emit"),
        "expected fresh next helper to handle class expression method call"
    );
}

#[test]
fn preserves_global_value_and_async_generator_method_binding_after_class_definition() {
    let program = frontend::parse(
        r#"
            var callCount = 0;
            class C {
              async *method() {
                callCount++;
              }
            }
            console.log("after-class", callCount);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("class setup statement should emit");
    }

    assert_eq!(
        function_compiler
            .backend
            .global_semantics
            .values
            .value_bindings
            .get("callCount"),
        Some(&Expression::Number(0.0)),
        "expected callCount static binding to remain 0 after class definition",
    );

    let method_expression = Expression::Member {
        object: Box::new(Expression::Member {
            object: Box::new(Expression::Identifier("C".to_string())),
            property: Box::new(Expression::String("prototype".to_string())),
        }),
        property: Box::new(Expression::String("method".to_string())),
    };
    assert_eq!(
        function_compiler.materialize_static_expression(&method_expression),
        Expression::Identifier("__ayy_class_method_1".to_string()),
        "expected prototype method binding to stay attached to the async generator method",
    );
}

#[test]
fn snapshots_fresh_simple_generator_completion_result_object() {
    let program = frontend::parse(
        r#"
            class A {
              *foo(a) {}
            }
            var result = A.prototype.foo(3).next();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        true,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let snapshot = function_compiler
        .state
        .speculation
        .static_semantics
        .last_bound_user_function_call
        .as_ref()
        .expect("expected next-call snapshot");
    assert_eq!(snapshot.function_name, "__ayy_simple_generator_next");
    assert!(matches!(
        snapshot.result_expression.as_ref(),
        Some(Expression::Object(entries))
            if matches!(
                entries.as_slice(),
                [
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(done_key), value: Expression::Bool(true) },
                    crate::ir::hir::ObjectEntry::Data { key: Expression::String(value_key), value: Expression::Undefined },
                ] if done_key == "done" && value_key == "value"
            )
    ));
    assert!(matches!(
        function_compiler.resolve_object_binding_property_value(
            function_compiler
                .state
                .speculation
                .static_semantics
                .objects
                .local_object_bindings
                .get("result")
                .expect("expected result object binding"),
            &Expression::String("done".to_string()),
        ),
        Some(Expression::Bool(true))
    ));
}

#[test]
fn resolves_async_generator_method_completion_effects_with_bound_receiver() {
    let program = frontend::parse(
        r#"
            var callCount = 0;
            class C {
              async *method() {
                console.log("own", this.method.hasOwnProperty("arguments"));
                callCount++;
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("class setup statement should emit");
    }

    let method_call = Expression::Call {
        callee: Box::new(Expression::Member {
            object: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("C".to_string())),
                property: Box::new(Expression::String("prototype".to_string())),
            }),
            property: Box::new(Expression::String("method".to_string())),
        }),
        arguments: Vec::new(),
    };
    let (steps, completion_effects, completion_value) = function_compiler
        .resolve_simple_generator_source(&method_call)
        .expect("expected simple async generator source");

    assert!(steps.is_empty());
    assert!(matches!(completion_value, Expression::Undefined));
    assert_eq!(completion_effects.len(), 2);
    let Statement::Print { values } = &completion_effects[0] else {
        panic!("expected print completion effect");
    };
    assert_eq!(values[0], Expression::String("own".to_string()));
    assert_eq!(
        function_compiler.materialize_static_expression(&values[1]),
        Expression::Bool(false),
        "expected bound receiver method lookup to resolve to the async generator function object",
    );
}

#[test]
fn preserves_generator_class_expression_static_computed_field_binding_after_resumption() {
    let program = frontend::parse(
        r#"
            let captured;
            function* g() {
              let C = class {
                [yield 9] = 9;
                static [yield 9] = 9;
              };
              captured = C;
            }
            let iter = g();
            iter.next();
            iter.next(9);
            iter.next(9);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let captured_binding = function_compiler
        .resolve_object_binding_from_expression(&Expression::Identifier("captured".to_string()))
        .expect("expected captured class object binding after generator resumption");
    assert_eq!(
        object_binding_lookup_value(&captured_binding, &Expression::String("9".to_string())),
        Some(&Expression::Number(9.0)),
    );
}

#[test]
fn resolves_async_generator_method_completion_effects_with_bound_receiver_for_caller() {
    let program = frontend::parse(
        r#"
            var callCount = 0;
            class C {
              async *method() {
                console.log("own", this.method.hasOwnProperty("caller"));
                callCount++;
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("class setup statement should emit");
    }

    let method_call = Expression::Call {
        callee: Box::new(Expression::Member {
            object: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("C".to_string())),
                property: Box::new(Expression::String("prototype".to_string())),
            }),
            property: Box::new(Expression::String("method".to_string())),
        }),
        arguments: Vec::new(),
    };
    let (steps, completion_effects, completion_value) = function_compiler
        .resolve_simple_generator_source(&method_call)
        .expect("expected simple async generator source");

    assert!(steps.is_empty());
    assert!(matches!(completion_value, Expression::Undefined));
    assert_eq!(completion_effects.len(), 2);
    let Statement::Print { values } = &completion_effects[0] else {
        panic!("expected print completion effect");
    };
    assert_eq!(values[0], Expression::String("own".to_string()));
    assert_eq!(
        function_compiler.materialize_static_expression(&values[1]),
        Expression::Bool(false),
        "expected bound receiver caller lookup to resolve to the async generator function object",
    );
}

#[test]
fn stores_async_generator_method_call_as_local_iterator_binding() {
    let program = frontend::parse(
        r#"
            class C {
              async *method() {
                yield 1;
              }
            }
            var iter = C.prototype.method();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let Some(binding_name) = function_compiler.resolve_local_array_iterator_binding_name("iter")
    else {
        panic!("expected iter iterator binding name");
    };
    assert!(matches!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_iterator_bindings
            .get(&binding_name)
            .map(|binding| &binding.source),
        Some(IteratorSourceKind::SimpleGenerator { is_async: true, .. })
    ));
}

#[test]
fn stores_async_generator_method_completion_effects_in_iterator_binding_for_caller() {
    let program = frontend::parse(
        r#"
            var callCount = 0;
            class C {
              async *method() {
                console.log("own", this.method.hasOwnProperty("caller"));
                callCount++;
              }
            }
            var iter = C.prototype.method();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let Some(binding_name) = function_compiler.resolve_local_array_iterator_binding_name("iter")
    else {
        panic!("expected iter iterator binding name");
    };
    let Some(IteratorSourceKind::SimpleGenerator {
        is_async,
        completion_effects,
        completion_value,
        ..
    }) = function_compiler
        .state
        .speculation
        .static_semantics
        .arrays
        .local_array_iterator_bindings
        .get(&binding_name)
        .map(|binding| &binding.source)
    else {
        panic!("expected iter simple generator source");
    };
    assert!(*is_async);
    assert!(matches!(completion_value, Expression::Undefined));
    let Statement::Print { values } = &completion_effects[0] else {
        panic!("expected print completion effect");
    };
    assert_eq!(
        function_compiler.materialize_static_expression(&values[1]),
        Expression::Bool(false),
        "expected stored iterator completion effect to preserve caller=false",
    );
}

#[test]
fn preserves_async_generator_method_receiver_resolution_after_call_emission_for_caller() {
    let program = frontend::parse(
        r#"
            var callCount = 0;
            class C {
              async *method() {
                console.log("own", this.method.hasOwnProperty("caller"));
                callCount++;
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("class setup statement should emit");
    }

    let method_call = Expression::Call {
        callee: Box::new(Expression::Member {
            object: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("C".to_string())),
                property: Box::new(Expression::String("prototype".to_string())),
            }),
            property: Box::new(Expression::String("method".to_string())),
        }),
        arguments: Vec::new(),
    };
    let (_, before_effects, _) = function_compiler
        .resolve_simple_generator_source(&method_call)
        .expect("expected simple async generator source before emission");
    let Statement::Print {
        values: before_values,
    } = &before_effects[0]
    else {
        panic!("expected print completion effect before emission");
    };
    assert_eq!(
        function_compiler.materialize_static_expression(&before_values[1]),
        Expression::Bool(false),
        "expected caller lookup to resolve before call emission",
    );

    function_compiler
        .emit_numeric_expression(&method_call)
        .expect("method call should emit");
    function_compiler
        .state
        .emission
        .output
        .instructions
        .push(0x1a);

    let (_, after_effects, _) = function_compiler
        .resolve_simple_generator_source(&method_call)
        .expect("expected simple async generator source after emission");
    let Statement::Print {
        values: after_values,
    } = &after_effects[0]
    else {
        panic!("expected print completion effect after emission");
    };
    assert_eq!(
        function_compiler.materialize_static_expression(&after_values[1]),
        Expression::Bool(false),
        "expected caller lookup to stay resolved after call emission",
    );
}

#[test]
fn consumes_chained_immediate_promise_outcome_for_async_generator_method_next() {
    let program = frontend::parse(
        r#"
            function done(value) {
              console.log("done", value);
            }
            class C {
              async *method() {
                console.log("own", this.method.hasOwnProperty("caller"));
              }
            }
            C.prototype.method().next()
              .then(function() {
                console.log("then1");
              }, done)
              .then(done, done);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let Statement::Expression(expression) = program
        .statements
        .last()
        .expect("expected chained then expression")
    else {
        panic!("expected chained then expression statement");
    };
    assert!(matches!(
        function_compiler
            .consume_immediate_promise_outcome(expression)
            .expect("chained immediate promise outcome should compile"),
        Some(StaticEvalOutcome::Value(Expression::Undefined))
    ));
}

#[test]
fn named_generator_inner_closures_capture_outer_internal_self_binding() {
    let program = frontend::parse(
        r#"
            let probeParams;
            let probeBody;

            let fnExpr = function* g(
              _ = (probeParams = function() { return g; })
            ) {
              probeBody = function() { return g; };
            };
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler.register_user_function_capture_bindings(&program.functions);

    let outer_function_name = compiler
        .state
        .function_registry
        .catalog
        .registered_function_declarations
        .iter()
        .find(|function| function.self_binding.as_deref() == Some("g"))
        .map(|function| function.name.clone())
        .expect("expected named generator expression function");

    let matching_capture_maps = compiler
        .state
        .function_registry
        .analysis
        .user_function_capture_bindings
        .values()
        .filter(|bindings| {
            bindings.contains_key(&outer_function_name) || bindings.contains_key("g")
        })
        .collect::<Vec<_>>();

    assert!(
        !matching_capture_maps.is_empty(),
        "expected nested closures to capture named generator self binding"
    );
    assert!(
        matching_capture_maps
            .iter()
            .all(|bindings| bindings.contains_key(&outer_function_name)),
        "expected nested closures to capture outer internal function name instead of bare self binding: {:?}",
        matching_capture_maps
    );
}

#[test]
fn folds_bigint_right_shift_beyond_i128_to_static_primitive() {
    let program = frontend::parse(
        r#"
            let value = 99022168773993092867842010762644549533710n;
            console.log("huge", value >> 5n);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        program.strict,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    function_compiler
        .emit_statement(&program.statements[0])
        .expect("bigint binding init should emit");

    let Statement::Print { values } = &program.statements[1] else {
        panic!("expected trailing print");
    };
    assert_eq!(
        function_compiler.resolve_static_primitive_expression_with_context(&values[1], None),
        Some(Expression::BigInt(
            "3094442774187284152120062836332642172928".to_string()
        )),
    );
}

#[test]
fn resolves_function_name_through_lowered_nullish_assignment() {
    let program = frontend::parse(
        r#"
            let missing = undefined;
            let named = missing ??= function() {};
            console.log("name", named.name);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..2] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let member_expression = Expression::Member {
        object: Box::new(Expression::Identifier("named".to_string())),
        property: Box::new(Expression::String("name".to_string())),
    };
    assert_eq!(
        function_compiler.resolve_function_name_value(
            &Expression::Identifier("named".to_string()),
            &Expression::String("name".to_string())
        ),
        Some("missing".to_string())
    );
    assert_eq!(
        function_compiler.resolve_static_string_value(&member_expression),
        Some("missing".to_string())
    );
}

#[test]
fn resolves_class_definition_function_property_precedence_bindings() {
    let program = crate::ir::pipeline::prepare(
        frontend::parse(
            r#"
            var namedSym = Symbol('test262');
            var anonSym = Symbol();
            var isDefined = false;
            class A {
              get id() {}
              get [anonSym]() {}
              get [namedSym]() {}
              set id(_) {}
              *gen() {}
              static get length() {
                if (isDefined) return 'pass';
                throw new Error('getter executed during definition');
              }
              static *name() {}
            }
            isDefined = true;
            var getter = Object.getOwnPropertyDescriptor(A.prototype, 'id').get;
            var anonGetter = Object.getOwnPropertyDescriptor(A.prototype, anonSym).get;
            var namedGetter = Object.getOwnPropertyDescriptor(A.prototype, namedSym).get;
            var setter = Object.getOwnPropertyDescriptor(A.prototype, 'id').set;
        "#,
        )
        .expect("program should parse"),
    )
    .expect("program should prepare");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let static_length_property = Expression::String("length".to_string());
    let static_name_property = Expression::String("name".to_string());
    let class_identifier = Expression::Identifier("A".to_string());
    let anon_symbol_identifier = Expression::Identifier("anonSym".to_string());

    assert!(
        function_compiler
            .resolve_member_getter_binding(&class_identifier, &static_length_property)
            .is_some(),
        "expected static length getter binding to resolve",
    );
    assert!(
        function_compiler
            .resolve_member_function_binding(&class_identifier, &static_length_property)
            .is_none(),
        "static getter-backed length should not also resolve as a value function binding",
    );
    assert_eq!(
        function_compiler.resolve_user_function_length(&class_identifier, &static_length_property),
        None,
        "static getter should suppress intrinsic function length",
    );
    assert_eq!(
        function_compiler.resolve_function_name_value(&class_identifier, &static_name_property),
        None,
        "static method should suppress intrinsic function name",
    );
    let static_length_member = Expression::Member {
        object: Box::new(class_identifier.clone()),
        property: Box::new(static_length_property.clone()),
    };
    assert_eq!(
        function_compiler.materialize_static_expression(&static_length_member),
        Expression::String("pass".to_string()),
        "static getter-backed length should materialize through the accessor body once definition-time side effects are complete",
    );
    assert_eq!(
        function_compiler.resolve_static_number_value(&static_length_member),
        None,
        "static getter should suppress intrinsic length number folding",
    );
    assert_eq!(
        function_compiler
            .resolve_static_primitive_expression_with_context(&static_length_member, None,),
        Some(Expression::String("pass".to_string())),
        "static getter-backed length should fold through primitive print resolution once its guard becomes true",
    );

    let anon_getter_binding = function_compiler
        .resolve_member_getter_binding(
            &Expression::Member {
                object: Box::new(class_identifier.clone()),
                property: Box::new(Expression::String("prototype".to_string())),
            },
            &anon_symbol_identifier,
        )
        .expect("anonymous symbol getter should resolve");
    let LocalFunctionBinding::User(anon_function_name) = anon_getter_binding else {
        panic!("anonymous symbol getter should be a user function binding");
    };
    let anon_getter_name_member = Expression::Member {
        object: Box::new(Expression::Identifier("anonGetter".to_string())),
        property: Box::new(Expression::String("name".to_string())),
    };
    assert!(
        matches!(
            function_compiler.resolve_function_binding_from_expression(&Expression::Identifier(
                "anonGetter".to_string()
            )),
            Some(LocalFunctionBinding::User(_))
        ),
        "descriptor getter local should preserve the user function binding",
    );
    assert_eq!(
        function_compiler.materialize_static_expression(&anon_getter_name_member),
        Expression::String("get ".to_string()),
        "anonymous symbol getter name should materialize through descriptor get binding",
    );
    assert_eq!(
        function_compiler.resolve_static_string_value(&anon_getter_name_member),
        Some("get ".to_string()),
        "anonymous symbol getter name should resolve through the direct string inference path",
    );
    assert_eq!(
        function_compiler.resolve_user_function_display_name(&anon_function_name),
        Some("get ".to_string()),
        "anonymous symbol getter should resolve to the accessor prefix with an empty symbol description",
    );
}

#[test]
fn does_not_fold_branched_function_call_to_static_number() {
    let program = frontend::parse(
        r#"
            function f() {
              if (true) return 'pass';
              return 'no';
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");
    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    assert_eq!(
        function_compiler.resolve_static_number_value(&Expression::Call {
            callee: Box::new(Expression::Identifier("f".to_string())),
            arguments: Vec::new(),
        }),
        None,
        "branched function call should not fold to a numeric constant",
    );
}

#[test]
fn resolves_static_symbol_getter_descriptor_binding_on_class_constructor() {
    let program = frontend::parse(
        r#"
            var anonSym = Symbol();
            class A {
              static get [anonSym]() {}
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let summarize_keys = |keys: Vec<&crate::backend::direct_wasm::MemberFunctionBindingKey>| {
        keys.into_iter()
            .map(|key| {
                let target = match &key.target {
                    crate::backend::direct_wasm::MemberFunctionBindingTarget::Identifier(name) => {
                        format!("id:{name}")
                    }
                    crate::backend::direct_wasm::MemberFunctionBindingTarget::Prototype(name) => {
                        format!("proto:{name}")
                    }
                };
                let property = match &key.property {
                    crate::backend::direct_wasm::MemberFunctionBindingProperty::String(name) => {
                        format!("str:{name}")
                    }
                    crate::backend::direct_wasm::MemberFunctionBindingProperty::Symbol(name) => {
                        format!("sym:{name}")
                    }
                    crate::backend::direct_wasm::MemberFunctionBindingProperty::SymbolExpression(name) => {
                        format!("symexpr:{name}")
                    }
                };
                format!("{target}/{property}")
            })
            .collect::<Vec<_>>()
    };

    let property = Expression::Identifier("anonSym".to_string());
    let computed_property_temp = function_compiler
        .state
        .runtime
        .locals
        .keys()
        .chain(
            function_compiler
                .backend
                .global_semantics
                .names
                .bindings
                .keys(),
        )
        .find(|name| name.starts_with("__ayy_class_prop_"))
        .cloned()
        .expect("expected lowered static computed property temp");
    assert!(matches!(
        function_compiler.resolve_property_key_expression(&Expression::Identifier(
            computed_property_temp.clone()
        )),
        Some(Expression::Identifier(name)) if name == "anonSym"
    ));
    assert!(
        function_compiler
            .resolve_member_getter_binding(&Expression::Identifier("A".to_string()), &property)
            .is_some(),
        "expected static symbol getter binding; local getters: {:?}; global getters: {:?}",
        summarize_keys(
            function_compiler
                .state
                .speculation
                .static_semantics
                .objects
                .member_getter_bindings
                .keys()
                .collect::<Vec<_>>(),
        ),
        summarize_keys(
            function_compiler
                .backend
                .global_semantics
                .members
                .member_getter_bindings
                .keys()
                .collect::<Vec<_>>(),
        ),
    );

    let descriptor = function_compiler
        .resolve_descriptor_binding_from_expression(&Expression::Call {
            callee: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("Object".to_string())),
                property: Box::new(Expression::String("getOwnPropertyDescriptor".to_string())),
            }),
            arguments: vec![
                CallArgument::Expression(Expression::Identifier("A".to_string())),
                CallArgument::Expression(property),
            ],
        })
        .expect("expected static symbol descriptor binding");
    assert!(descriptor.has_get, "expected getter descriptor");
}

#[test]
fn resolves_primitive_member_getter_from_object_prototype_define_property() {
    let program = frontend::parse(
        r#"
            "use strict";
            Object.defineProperty(Object.prototype, "x", {
              get: function() {
                return typeof this;
              }
            });
            console.log("primitive", (5).x);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        true,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..2] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    assert_eq!(
        function_compiler.resolve_member_getter_binding(
            &Expression::Number(5.0),
            &Expression::String("x".to_string())
        ),
        Some(super::LocalFunctionBinding::User(
            "__ayy_fnexpr_1".to_string()
        ))
    );
}

#[test]
fn resolves_static_string_replace_callback_result() {
    let program = frontend::parse(
        r#"
            "use strict";
            function replacer() {
              "use strict";
              return "a";
            }
            console.log("replace", "ab".replace("b", replacer));
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        true,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    let replace_expression = Expression::Call {
        callee: Box::new(Expression::Member {
            object: Box::new(Expression::String("ab".to_string())),
            property: Box::new(Expression::String("replace".to_string())),
        }),
        arguments: vec![
            CallArgument::Expression(Expression::String("b".to_string())),
            CallArgument::Expression(Expression::Identifier("replacer".to_string())),
        ],
    };

    assert_eq!(
        function_compiler
            .resolve_static_primitive_expression_with_context(&replace_expression, None,),
        Some(Expression::String("aa".to_string()))
    );
}

#[test]
fn resolves_global_object_literal_getter_binding_for_identifier() {
    let program = frontend::parse(
        r#"
            var obj = {
              get value() {
                return "x";
              }
            };
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    assert_eq!(
        function_compiler.resolve_member_getter_binding(
            &Expression::Identifier("obj".to_string()),
            &Expression::String("value".to_string())
        ),
        Some(super::LocalFunctionBinding::User(
            "__ayy_getter_1".to_string()
        ))
    );
}

#[test]
fn resolves_class_computed_numeric_accessor_bindings() {
    let program = frontend::parse(
        r#"
            class C {
              get [1 + 1]() { return 2; }
              set [1 + 1](v) { return 2; }
              static get [1 + 1]() { return 2; }
              static set [1 + 1](v) { return 2; }
            }
            let c = new C();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let summarize_keys = |keys: Vec<&crate::backend::direct_wasm::MemberFunctionBindingKey>| {
        keys.into_iter()
            .map(|key| {
                let target = match &key.target {
                    crate::backend::direct_wasm::MemberFunctionBindingTarget::Identifier(name) => {
                        format!("id:{name}")
                    }
                    crate::backend::direct_wasm::MemberFunctionBindingTarget::Prototype(name) => {
                        format!("proto:{name}")
                    }
                };
                let property = match &key.property {
                    crate::backend::direct_wasm::MemberFunctionBindingProperty::String(name) => {
                        format!("str:{name}")
                    }
                    crate::backend::direct_wasm::MemberFunctionBindingProperty::Symbol(name) => {
                        format!("sym:{name}")
                    }
                    crate::backend::direct_wasm::MemberFunctionBindingProperty::SymbolExpression(name) => {
                        format!("symexpr:{name}")
                    }
                };
                format!("{target}/{property}")
            })
            .collect::<Vec<_>>()
    };

    assert!(
        function_compiler
            .resolve_member_getter_binding(
                &Expression::Identifier("C".to_string()),
                &Expression::Number(2.0),
            )
            .is_some(),
        "expected static computed getter binding; local getters: {:?}; global getters: {:?}; local setters: {:?}; global setters: {:?}; statements: {:?}",
        summarize_keys(
            function_compiler
                .state
                .speculation
                .static_semantics
                .objects
                .member_getter_bindings
                .keys()
                .collect::<Vec<_>>(),
        ),
        summarize_keys(
            function_compiler
                .backend
                .global_semantics
                .members
                .member_getter_bindings
                .keys()
                .collect::<Vec<_>>(),
        ),
        summarize_keys(
            function_compiler
                .state
                .speculation
                .static_semantics
                .objects
                .member_setter_bindings
                .keys()
                .collect::<Vec<_>>(),
        ),
        summarize_keys(
            function_compiler
                .backend
                .global_semantics
                .members
                .member_setter_bindings
                .keys()
                .collect::<Vec<_>>(),
        ),
        program.statements,
    );
    assert!(
        function_compiler
            .resolve_member_setter_binding(
                &Expression::Identifier("C".to_string()),
                &Expression::Number(2.0),
            )
            .is_some(),
        "expected static computed setter binding",
    );
    assert!(
        function_compiler
            .resolve_member_getter_binding(
                &Expression::Identifier("c".to_string()),
                &Expression::Number(2.0),
            )
            .is_some(),
        "expected identifier-bound instance computed getter binding",
    );
    assert!(
        function_compiler
            .resolve_member_setter_binding(
                &Expression::Identifier("c".to_string()),
                &Expression::Number(2.0),
            )
            .is_some(),
        "expected identifier-bound instance computed setter binding",
    );
    let stringified_property = Expression::Call {
        callee: Box::new(Expression::Identifier("String".to_string())),
        arguments: vec![CallArgument::Expression(Expression::Binary {
            op: crate::ir::hir::BinaryOp::Add,
            left: Box::new(Expression::Number(1.0)),
            right: Box::new(Expression::Number(1.0)),
        })],
    };
    assert_eq!(
        function_compiler.resolve_property_key_expression(&stringified_property),
        Some(Expression::String("2".to_string())),
        "expected String(1 + 1) computed property key to resolve to \"2\"",
    );
    assert!(
        function_compiler
            .resolve_member_getter_binding(
                &Expression::Identifier("c".to_string()),
                &stringified_property,
            )
            .is_some(),
        "expected identifier-bound instance stringified computed getter binding",
    );
    assert!(
        function_compiler
            .resolve_member_getter_binding(
                &Expression::Identifier("C".to_string()),
                &stringified_property,
            )
            .is_some(),
        "expected static stringified computed getter binding",
    );
    let stringified_zero_property = Expression::Call {
        callee: Box::new(Expression::Identifier("String".to_string())),
        arguments: vec![CallArgument::Expression(Expression::Binary {
            op: crate::ir::hir::BinaryOp::Subtract,
            left: Box::new(Expression::Number(1.0)),
            right: Box::new(Expression::Number(1.0)),
        })],
    };
    assert_eq!(
        function_compiler.resolve_property_key_expression(&stringified_zero_property),
        Some(Expression::String("0".to_string())),
        "expected String(1 - 1) computed property key to resolve to \"0\"",
    );
    assert!(
        function_compiler
            .resolve_member_getter_binding(
                &Expression::New {
                    callee: Box::new(Expression::Identifier("C".to_string())),
                    arguments: Vec::new(),
                },
                &Expression::Number(2.0),
            )
            .is_some(),
        "expected instance computed getter binding",
    );
    assert!(
        function_compiler
            .resolve_member_setter_binding(
                &Expression::New {
                    callee: Box::new(Expression::Identifier("C".to_string())),
                    arguments: Vec::new(),
                },
                &Expression::Number(2.0),
            )
            .is_some(),
        "expected instance computed setter binding",
    );
}

#[test]
fn resolves_class_computed_arrow_accessor_bindings() {
    let program = frontend::parse(
        r#"
            class C {
              get [() => { }]() { return 1; }
              set [() => { }](v) { return 1; }
              static get [() => { }]() { return 1; }
              static set [() => { }](v) { return 1; }
            }
            let c = new C();
            console.log(c[() => { }], C[() => { }], C[String(() => { })]);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..program.statements.len() - 1] {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let summarize_keys = |keys: Vec<&crate::backend::direct_wasm::MemberFunctionBindingKey>| {
        keys.into_iter()
            .map(|key| {
                let target = match &key.target {
                    crate::backend::direct_wasm::MemberFunctionBindingTarget::Identifier(name) => {
                        format!("id:{name}")
                    }
                    crate::backend::direct_wasm::MemberFunctionBindingTarget::Prototype(name) => {
                        format!("proto:{name}")
                    }
                };
                let property = match &key.property {
                    crate::backend::direct_wasm::MemberFunctionBindingProperty::String(name) => {
                        format!("str:{name}")
                    }
                    crate::backend::direct_wasm::MemberFunctionBindingProperty::Symbol(name) => {
                        format!("sym:{name}")
                    }
                    crate::backend::direct_wasm::MemberFunctionBindingProperty::SymbolExpression(name) => {
                        format!("symexpr:{name}")
                    }
                };
                format!("{target}/{property}")
            })
            .collect::<Vec<_>>()
    };

    let static_arrow_access = Expression::Identifier("__ayy_arrow_11".to_string());
    assert_eq!(
        function_compiler.resolve_property_key_expression(&static_arrow_access),
        Some(Expression::String("function() {}".to_string())),
        "expected lowered static raw arrow key to coerce to synthesized function source",
    );
    assert!(
        function_compiler
            .resolve_member_getter_binding(
                &Expression::Identifier("C".to_string()),
                &static_arrow_access,
            )
            .is_some(),
        "expected static raw arrow getter binding to resolve; local getters: {:?}; global getters: {:?}",
        summarize_keys(
            function_compiler
                .state
                .speculation
                .static_semantics
                .objects
                .member_getter_bindings
                .keys()
                .collect::<Vec<_>>(),
        ),
        summarize_keys(
            function_compiler
                .backend
                .global_semantics
                .members
                .member_getter_bindings
                .keys()
                .collect::<Vec<_>>(),
        ),
    );
}

#[test]
fn resolves_class_computed_assignment_accessor_bindings() {
    let program = frontend::parse(
        r#"
            let x = 0;
            class C {
              get [x = 1]() { return 2; }
              set [x = 1](v) { return 2; }
              static get [x = 1]() { return 2; }
              static set [x = 1](v) { return 2; }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let assigned_property = Expression::Assign {
        name: "x".to_string(),
        value: Box::new(Expression::Number(1.0)),
    };
    assert_eq!(
        function_compiler.resolve_property_key_expression(&assigned_property),
        Some(Expression::String("1".to_string())),
        "expected computed assignment property key to resolve to \"1\"",
    );
    assert!(
        function_compiler
            .resolve_member_getter_binding(
                &Expression::New {
                    callee: Box::new(Expression::Identifier("C".to_string())),
                    arguments: Vec::new(),
                },
                &assigned_property,
            )
            .is_some(),
        "expected instance computed assignment getter binding",
    );
    assert!(
        function_compiler
            .resolve_member_getter_binding(
                &Expression::Identifier("C".to_string()),
                &assigned_property,
            )
            .is_some(),
        "expected static computed assignment getter binding",
    );

    let bitwise_or_property = Expression::Assign {
        name: "x".to_string(),
        value: Box::new(Expression::Binary {
            op: crate::ir::hir::BinaryOp::BitwiseOr,
            left: Box::new(Expression::Identifier("x".to_string())),
            right: Box::new(Expression::Number(1.0)),
        }),
    };
    assert_eq!(
        function_compiler.resolve_property_key_expression(&bitwise_or_property),
        Some(Expression::String("1".to_string())),
        "expected computed bitwise-or assignment property key to resolve to \"1\"",
    );
    assert!(
        function_compiler
            .resolve_member_getter_binding(
                &Expression::New {
                    callee: Box::new(Expression::Identifier("C".to_string())),
                    arguments: Vec::new(),
                },
                &bitwise_or_property,
            )
            .is_some(),
        "expected instance computed bitwise-or getter binding",
    );
    assert!(
        function_compiler
            .resolve_member_getter_binding(
                &Expression::Identifier("C".to_string()),
                &bitwise_or_property,
            )
            .is_some(),
        "expected static computed bitwise-or getter binding",
    );

    let coalesce_property = Expression::Conditional {
        condition: Box::new(Expression::Binary {
            op: crate::ir::hir::BinaryOp::LogicalAnd,
            left: Box::new(Expression::Binary {
                op: crate::ir::hir::BinaryOp::NotEqual,
                left: Box::new(Expression::Identifier("x".to_string())),
                right: Box::new(Expression::Undefined),
            }),
            right: Box::new(Expression::Binary {
                op: crate::ir::hir::BinaryOp::NotEqual,
                left: Box::new(Expression::Identifier("x".to_string())),
                right: Box::new(Expression::Null),
            }),
        }),
        then_expression: Box::new(Expression::Identifier("x".to_string())),
        else_expression: Box::new(Expression::Assign {
            name: "x".to_string(),
            value: Box::new(Expression::Number(1.0)),
        }),
    };
    assert_eq!(
        function_compiler.resolve_property_key_expression(&coalesce_property),
        Some(Expression::String("1".to_string())),
        "expected computed coalesce assignment property key to resolve to \"1\"",
    );

    let awaited_property = Expression::Await(Box::new(Expression::Number(9.0)));
    assert_eq!(
        function_compiler.resolve_property_key_expression(&awaited_property),
        Some(Expression::String("9".to_string())),
        "expected awaited property key to resolve to \"9\"",
    );
}

#[test]
fn materializes_class_computed_function_declaration_accessor_reads() {
    let program = frontend::parse(
        r#"
            function f() {}
            class C {
              get [f()]() { return 1; }
              set [f()](v) { return 1; }
              static get [f()]() { return 1; }
              static set [f()](v) { return 1; }
            }
            let c = new C();
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let summarize_keys = |keys: Vec<&crate::backend::direct_wasm::MemberFunctionBindingKey>| {
        keys.into_iter()
            .map(|key| {
                let target = match &key.target {
                    crate::backend::direct_wasm::MemberFunctionBindingTarget::Identifier(name) => {
                        format!("id:{name}")
                    }
                    crate::backend::direct_wasm::MemberFunctionBindingTarget::Prototype(name) => {
                        format!("proto:{name}")
                    }
                };
                let property = match &key.property {
                    crate::backend::direct_wasm::MemberFunctionBindingProperty::String(name) => {
                        format!("str:{name}")
                    }
                    crate::backend::direct_wasm::MemberFunctionBindingProperty::Symbol(name) => {
                        format!("sym:{name}")
                    }
                    crate::backend::direct_wasm::MemberFunctionBindingProperty::SymbolExpression(name) => {
                        format!("symexpr:{name}")
                    }
                };
                format!("{target}/{property}")
            })
            .collect::<Vec<_>>()
    };

    let computed_property = Expression::Call {
        callee: Box::new(Expression::Identifier("f".to_string())),
        arguments: Vec::new(),
    };
    assert_eq!(
        function_compiler.resolve_property_key_expression(&computed_property),
        Some(Expression::String("undefined".to_string())),
        "expected function-declaration computed property key to resolve to \"undefined\"",
    );
    assert!(
        function_compiler
            .resolve_member_getter_binding(
                &Expression::Member {
                    object: Box::new(Expression::Identifier("C".to_string())),
                    property: Box::new(Expression::String("prototype".to_string())),
                },
                &computed_property,
            )
            .is_some(),
        "expected explicit prototype computed getter binding from function declaration key; local getters: {:?}; global getters: {:?}",
        summarize_keys(
            function_compiler
                .state
                .speculation
                .static_semantics
                .objects
                .member_getter_bindings
                .keys()
                .collect::<Vec<_>>(),
        ),
        summarize_keys(
            function_compiler
                .backend
                .global_semantics
                .members
                .member_getter_bindings
                .keys()
                .collect::<Vec<_>>(),
        ),
    );
    assert_eq!(
        function_compiler
            .resolve_static_object_prototype_expression(&Expression::Identifier("c".to_string())),
        Some(Expression::Member {
            object: Box::new(Expression::Identifier("C".to_string())),
            property: Box::new(Expression::String("prototype".to_string())),
        }),
        "expected c to retain C.prototype as its static prototype; alias: {:?}; local value: {:?}; global value: {:?}",
        function_compiler.resolve_bound_alias_expression(&Expression::Identifier("c".to_string())),
        function_compiler
            .state
            .speculation
            .static_semantics
            .values
            .local_value_bindings
            .get("c")
            .cloned(),
        function_compiler
            .backend
            .global_semantics
            .values
            .value_bindings
            .get("c")
            .cloned(),
    );
    assert!(
        function_compiler
            .resolve_member_getter_binding(
                &Expression::Identifier("c".to_string()),
                &computed_property,
            )
            .is_some(),
        "expected instance computed getter binding from function declaration key; local getters: {:?}; global getters: {:?}",
        summarize_keys(
            function_compiler
                .state
                .speculation
                .static_semantics
                .objects
                .member_getter_bindings
                .keys()
                .collect::<Vec<_>>(),
        ),
        summarize_keys(
            function_compiler
                .backend
                .global_semantics
                .members
                .member_getter_bindings
                .keys()
                .collect::<Vec<_>>(),
        ),
    );
    assert!(
        function_compiler
            .resolve_member_getter_binding(
                &Expression::Identifier("C".to_string()),
                &computed_property,
            )
            .is_some(),
        "expected static computed getter binding from function declaration key; local getters: {:?}; global getters: {:?}",
        summarize_keys(
            function_compiler
                .state
                .speculation
                .static_semantics
                .objects
                .member_getter_bindings
                .keys()
                .collect::<Vec<_>>(),
        ),
        summarize_keys(
            function_compiler
                .backend
                .global_semantics
                .members
                .member_getter_bindings
                .keys()
                .collect::<Vec<_>>(),
        ),
    );
    assert_eq!(
        function_compiler.materialize_static_expression(&Expression::Member {
            object: Box::new(Expression::Identifier("c".to_string())),
            property: Box::new(computed_property.clone()),
        }),
        Expression::Number(1.0),
        "expected instance accessor read to materialize getter result",
    );
    assert_eq!(
        function_compiler.materialize_static_expression(&Expression::Member {
            object: Box::new(Expression::Identifier("C".to_string())),
            property: Box::new(computed_property),
        }),
        Expression::Number(1.0),
        "expected static accessor read to materialize getter result",
    );
}

#[test]
fn lowers_non_generator_async_functions_with_direct_await_expressions() {
    let program = frontend::parse(
        r#"
            async function f() {
              let x = await Promise.resolve("rhs");
              return x;
            }
        "#,
    )
    .expect("program should parse");

    let function = program
        .functions
        .iter()
        .find(|function| function.name == "f")
        .expect("expected async function");

    assert!(
        function
            .body
            .iter()
            .all(|statement| !matches!(statement, Statement::Yield { .. })),
        "expected non-generator async function body to avoid generator yields: {:?}",
        function.body,
    );
    assert!(
        function.body.iter().any(|statement| matches!(
            statement,
            Statement::Let {
                value: Expression::Await(_),
                ..
            }
        )),
        "expected lowered async function body to retain await expressions: {:?}",
        function.body,
    );
}

#[test]
fn resolves_static_await_outcome_for_promise_resolve_call() {
    let program = frontend::parse(
        r#"
            async function f() {
              return await Promise.resolve("rhs");
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_function_map
        .get("f")
        .cloned()
        .expect("expected registered user function");

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        Some(&user_function),
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should construct");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    let function = program
        .functions
        .iter()
        .find(|function| function.name == "f")
        .expect("expected function");
    let Statement::Return(Expression::Await(expression)) = function
        .body
        .last()
        .expect("expected return await statement")
    else {
        panic!("expected return await statement");
    };

    match function_compiler.resolve_static_await_resolution_outcome(expression) {
        Some(StaticEvalOutcome::Value(Expression::String(value))) => {
            assert_eq!(value, "rhs");
        }
        Some(StaticEvalOutcome::Value(value)) => {
            panic!("expected resolved rhs string, got {value:?}");
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(name))) => {
            panic!("expected resolved rhs string, got named error {name}");
        }
        Some(StaticEvalOutcome::Throw(StaticThrowValue::Value(value))) => {
            panic!("expected resolved rhs string, got thrown value {value:?}");
        }
        None => panic!("expected resolved rhs string"),
    }
}

#[test]
fn registers_effectful_getter_functions_without_recursing() {
    let program = frontend::parse(
        r#"
            var log = [];
            var obj = {
              get value() {
                log.push("hit");
                return "x";
              }
            };
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
}

#[test]
fn registers_effectful_getter_globals_without_recursing() {
    let program = frontend::parse(
        r#"
            var log = [];
            var obj = {
              get value() {
                log.push("hit");
                return "x";
              }
            };
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
}

#[test]
fn emits_effectful_getter_member_print_without_recursing() {
    let program = frontend::parse(
        r#"
            var log = [];
            var obj = {
              get value() {
                log.push("hit");
                return "x";
              }
            };
            console.log("side", obj.value, log.length);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }
}

#[test]
fn emits_effectful_getter_call_without_recursing() {
    let program = frontend::parse(
        r#"
            var log = [];
            var obj = {
              get value() {
                log.push("hit");
                return "x";
              }
            };
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    let getter_binding = function_compiler
        .resolve_member_getter_binding(
            &Expression::Identifier("obj".to_string()),
            &Expression::String("value".to_string()),
        )
        .expect("getter binding should resolve");
    let super::LocalFunctionBinding::User(function_name) = getter_binding else {
        panic!("expected user getter binding");
    };
    let user_function = function_compiler
        .backend
        .function_registry
        .catalog
        .user_function_map
        .get(&function_name)
        .cloned()
        .expect("getter user function should exist");
    function_compiler
        .emit_user_function_call_with_function_this_binding(
            &user_function,
            &[],
            &Expression::Identifier("obj".to_string()),
            None,
        )
        .expect("getter call should emit");
}

#[test]
fn resolves_effectful_iife_ordinary_to_primitive_throw_plan() {
    let program = frontend::parse(
        r#"
            var trace = "";
            (function() {
              trace += "1";
              return {
                valueOf: function() {
                  trace += "3";
                  throw 1;
                }
              };
            })() + 0;
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    let Statement::Expression(Expression::Binary { left, .. }) = &program.statements[1] else {
        panic!("expected binary addition expression");
    };
    let plan = function_compiler
        .resolve_ordinary_to_primitive_plan(left)
        .expect("expected ordinary-to-primitive plan for effectful IIFE");

    assert!(matches!(
        function_compiler.analyze_ordinary_to_primitive_plan(&plan),
        OrdinaryToPrimitiveAnalysis::Throw
    ));

    let Statement::Expression(expression) = &program.statements[1] else {
        panic!("expected expression statement");
    };
    let folded =
        function_compiler.resolve_static_primitive_expression_with_context(expression, None);
    assert!(
        folded.is_none(),
        "effectful ordinary-to-primitive addition should not fold statically: {folded:?}",
    );
}

#[test]
fn resets_global_string_binding_after_effectful_addition_try() {
    let program = frontend::parse(
        r#"
            function MyError() {}
            var trace = "";

            try {
              (function() {
                trace += "1";
                return { valueOf: function() { trace += "3"; throw new MyError(); } };
              })() + (function() {
                trace += "2";
                return { valueOf: function() { trace += "4"; throw new Error("should not run"); } };
              })();
            } catch (e) {}

            trace = "";
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    assert_eq!(
        function_compiler.resolve_static_string_value(&Expression::Identifier("trace".to_string())),
        Some(String::new())
    );
}

#[test]
fn resets_global_string_binding_after_effectful_addition_try_with_catch_print() {
    let program = frontend::parse(
        r#"
            function MyError() {}
            var trace = "";

            try {
              (function() {
                trace += "1";
                return { valueOf: function() { trace += "3"; throw new MyError(); } };
              })() + (function() {
                trace += "2";
                return { valueOf: function() { trace += "4"; throw new Error("should not run"); } };
              })();
            } catch (e) {
              console.log("case1", trace);
            }

            trace = "";
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    assert_eq!(
        function_compiler.resolve_static_string_value(&Expression::Identifier("trace".to_string())),
        Some(String::new())
    );
}

#[test]
fn resets_global_number_binding_after_effectful_addition_try_with_catch_print() {
    let program = frontend::parse(
        r#"
            function MyError() {}
            var trace = "";

            try {
              (function() {
                trace += "1";
                return { valueOf: function() { trace += "3"; throw new MyError(); } };
              })() + (function() {
                trace += "2";
                return { valueOf: function() { trace += "4"; throw new Error("should not run"); } };
              })();
            } catch (e) {
              console.log("case1", trace);
            }

            trace = 99;
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    assert_eq!(
        function_compiler.resolve_static_number_value(&Expression::Identifier("trace".to_string())),
        Some(99.0)
    );
    assert_eq!(
        function_compiler.resolve_static_primitive_expression_with_context(
            &Expression::Identifier("trace".to_string()),
            None
        ),
        Some(Expression::Number(99.0))
    );
    assert_eq!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .values
            .local_value_bindings
            .get("trace"),
        None,
        "unexpected stale local trace binding: {:?}",
        function_compiler
            .state
            .speculation
            .static_semantics
            .values
            .local_value_bindings
            .get("trace")
    );

    function_compiler
        .emit_print_value(&Expression::Identifier("trace".to_string()))
        .expect("print should emit");
    assert!(
        function_compiler
            .backend
            .module_artifacts
            .string_data
            .iter()
            .any(|(_, bytes)| bytes == b"99"),
        "expected print emission to intern 99, found {:?}",
        function_compiler
            .backend
            .module_artifacts
            .string_data
            .iter()
            .map(|(_, bytes)| String::from_utf8_lossy(bytes).to_string())
            .collect::<Vec<_>>()
    );
}

#[test]
fn keeps_post_try_assignment_state_with_following_print_present() {
    let program = frontend::parse(
        r#"
            function MyError() {}
            var trace = "";

            try {
              (function() {
                trace += "1";
                return { valueOf: function() { trace += "3"; throw new MyError(); } };
              })() + (function() {
                trace += "2";
                return { valueOf: function() { trace += "4"; throw new Error("should not run"); } };
              })();
            } catch (e) {
              console.log("case1", trace);
            }

            trace = 99;
            console.log("mid", trace);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements[..3] {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    assert_eq!(
        function_compiler.resolve_static_primitive_expression_with_context(
            &Expression::Identifier("trace".to_string()),
            None
        ),
        Some(Expression::Number(99.0))
    );
}

#[test]
fn compile_path_preserves_post_try_assignment_print_value() {
    let program = frontend::parse(
        r#"
            function MyError() {}
            var trace = "";

            try {
              (function() {
                trace += "1";
                return { valueOf: function() { trace += "3"; throw new MyError(); } };
              })() + (function() {
                trace += "2";
                return { valueOf: function() { trace += "4"; throw new Error("should not run"); } };
              })();
            } catch (e) {
              console.log("case1", trace);
            }

            trace = 99;
            console.log("mid", trace);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut start_statements = program
        .functions
        .iter()
        .filter(|function| function.register_global)
        .map(|function| Statement::Assign {
            name: function.name.clone(),
            value: Expression::Identifier(function.name.clone()),
        })
        .collect::<Vec<_>>();
    start_statements.extend_from_slice(&program.statements);

    FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize")
    .compile(&start_statements)
    .expect("compile should succeed");

    assert!(
        compiler
            .state
            .module_artifacts
            .string_data
            .iter()
            .any(|(_, bytes)| bytes == b"99"),
        "expected compile path to intern 99, found {:?}",
        compiler
            .state
            .module_artifacts
            .string_data
            .iter()
            .map(|(_, bytes)| String::from_utf8_lossy(bytes).to_string())
            .collect::<Vec<_>>()
    );
}

#[test]
fn second_effectful_addition_try_starts_from_reset_trace_binding() {
    let program = frontend::parse(
        r#"
            function MyError() {}
            var trace = "";

            try {
              (function() {
                trace += "1";
                return { valueOf: function() { trace += "3"; throw new MyError(); } };
              })() + (function() {
                trace += "2";
                return { valueOf: function() { trace += "4"; throw new Error("should not run"); } };
              })();
            } catch (e) {
              console.log("addition-order-case1", trace, !!e);
            }

            trace = "";

            try {
              (function() {
                trace += "1";
                return { valueOf: function() { trace += "3"; return 1; } };
              })() + (function() {
                trace += "2";
                return { valueOf: function() { trace += "4"; throw new MyError(); } };
              })();
            } catch (e) {
              console.log("addition-order-case2", trace, !!e);
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    function_compiler
        .emit_statement(&program.statements[0])
        .expect("trace initializer should emit");
    function_compiler
        .emit_statement(&program.statements[1])
        .expect("first try should emit");
    function_compiler
        .emit_statement(&program.statements[2])
        .expect("trace reset should emit");

    assert_eq!(
        function_compiler.resolve_static_string_value(&Expression::Identifier("trace".to_string())),
        Some(String::new())
    );

    function_compiler
        .emit_statement(&program.statements[3])
        .expect("second try should emit");

    assert_eq!(
        function_compiler.resolve_static_string_value(&Expression::Identifier("trace".to_string())),
        Some("1234".to_string())
    );
}

#[test]
fn full_program_compile_does_not_bake_stale_trace_into_second_addition_case() {
    let program = frontend::parse(
        r#"
            function MyError() {}
            var trace = "";

            try {
              (function() {
                trace += "1";
                return { valueOf: function() { trace += "3"; throw new MyError(); } };
              })() + (function() {
                trace += "2";
                return { valueOf: function() { trace += "4"; throw new Error("should not run"); } };
              })();
            } catch (e) {
              console.log("addition-order-case1", trace, !!e);
            }

            trace = "";
            try {
              (function() {
                trace += "1";
                return { valueOf: function() { trace += "3"; return 1; } };
              })() + (function() {
                trace += "2";
                return { valueOf: function() { trace += "4"; throw new MyError(); } };
              })();
            } catch (e) {
              console.log("addition-order-case2", trace, !!e);
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler.compile(&program).expect("compile should succeed");

    let interned_strings = compiler
        .state
        .module_artifacts
        .string_data
        .iter()
        .map(|(_, bytes)| String::from_utf8_lossy(bytes).to_string())
        .collect::<Vec<_>>();
    assert!(
        !interned_strings.iter().any(|value| value == "1231234"),
        "unexpected stale trace string in full compile: {interned_strings:?}"
    );
    assert!(
        interned_strings.iter().any(|value| value == "1234"),
        "expected fresh trace string in full compile: {interned_strings:?}"
    );
}

#[test]
fn full_registration_keeps_trace_reset_before_second_captured_try() {
    let program = frontend::parse(
        r#"
            function MyError() {}
            var trace = "";

            try {
              (function() {
                trace += "1";
                return { valueOf: function() { trace += "3"; throw new MyError(); } };
              })() + (function() {
                trace += "2";
                return { valueOf: function() { trace += "4"; throw new Error("should not run"); } };
              })();
            } catch (e) {
              console.log("addition-order-case1", trace, !!e);
            }

            trace = "";
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;
    compiler.register_user_function_capture_bindings(&program.functions);
    compiler
        .reserve_function_constructor_implicit_global_bindings(&program)
        .expect("implicit globals should reserve");
    compiler.reserve_global_runtime_prototype_binding_globals();

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        program.strict,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    function_compiler
        .emit_statement(&program.statements[0])
        .expect("trace init should emit");
    function_compiler
        .emit_statement(&program.statements[1])
        .expect("first try should emit");
    function_compiler
        .emit_statement(&program.statements[2])
        .expect("trace reset should emit");

    assert_eq!(
        function_compiler
            .state
            .speculation
            .execution_context
            .current_user_function_name,
        None
    );
    assert_eq!(
        function_compiler
            .backend
            .global_semantics
            .values
            .value_bindings
            .get("trace")
            .cloned(),
        Some(Expression::String(String::new()))
    );
    assert_eq!(
        function_compiler.resolve_static_string_value(&Expression::Identifier("trace".to_string())),
        Some(String::new())
    );
}

#[test]
fn registers_top_level_global_capture_for_function_apply_callback() {
    let program = frontend::parse(
        r#"
            var callCount = 0;
            (function() {
              callCount += 1;
            }.apply(null, [1, 2, 3]));
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let function_name = compiler
        .state
        .function_registry
        .analysis
        .user_function_capture_bindings
        .iter()
        .find_map(|(function_name, bindings)| {
            bindings
                .contains_key("callCount")
                .then_some(function_name.clone())
        })
        .expect("expected anonymous apply callback capture binding");

    assert_eq!(
        compiler
            .state
            .function_registry
            .analysis
            .user_function_capture_bindings
            .get(&function_name)
            .and_then(|bindings| bindings.get("callCount")),
        Some(&format!(
            "__ayy_capture_binding__{}__callCount",
            function_name
        ))
    );
}

#[test]
fn apply_call_updates_static_global_capture_binding_after_emit() {
    let program = frontend::parse(
        r#"
            var callCount = 0;
            (function() {
              callCount += 1;
            }.apply(null, [1, 2, 3]));
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;
    compiler.register_user_function_capture_bindings(&program.functions);
    compiler
        .reserve_function_constructor_implicit_global_bindings(&program)
        .expect("implicit globals should reserve");
    compiler.reserve_global_runtime_prototype_binding_globals();

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        program.strict,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    function_compiler
        .emit_statement(&program.statements[0])
        .expect("callCount init should emit");
    function_compiler
        .emit_statement(&program.statements[1])
        .expect("apply call should emit");

    assert_eq!(
        function_compiler
            .backend
            .global_semantics
            .values
            .value_bindings
            .get("callCount")
            .cloned(),
        Some(Expression::Number(1.0))
    );
}

#[test]
fn apply_call_with_print_updates_static_global_capture_binding_after_emit() {
    let program = frontend::parse(
        r#"
            var callCount = 0;
            (function() {
              console.log("apply-spread", arguments.length, arguments[0], arguments[1], arguments[2]);
              callCount += 1;
            }.apply(null, [1, 2, 3]));
            console.log("apply-count", callCount);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;
    compiler.register_user_function_capture_bindings(&program.functions);
    compiler
        .reserve_function_constructor_implicit_global_bindings(&program)
        .expect("implicit globals should reserve");
    compiler.reserve_global_runtime_prototype_binding_globals();

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        program.strict,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    let apply_function_name = program.functions[0].name.clone();
    let snapshot_result = function_compiler.resolve_bound_snapshot_user_function_result(
        &apply_function_name,
        &HashMap::from([("callCount".to_string(), Expression::Number(0.0))]),
    );
    assert_eq!(
        snapshot_result,
        Some((
            Expression::Undefined,
            HashMap::from([
                ("arguments".to_string(), Expression::Array(Vec::new())),
                ("callCount".to_string(), Expression::Number(1.0)),
            ]),
        ))
    );
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    function_compiler
        .emit_statement(&program.statements[0])
        .expect("callCount init should emit");
    function_compiler
        .emit_statement(&program.statements[1])
        .expect("apply call should emit");

    let Statement::Print { values } = &program.statements[2] else {
        panic!("expected trailing print statement");
    };

    assert_eq!(
        function_compiler.resolve_static_number_value(&values[1]),
        Some(1.0)
    );
}

#[test]
fn with_scope_identifiers_do_not_resolve_to_static_local_strings() {
    let program = frontend::parse(
        r#"
            function withCase() {
              var env = new Object();
              env.outer = "with";
              var outer = "local";
              with (env) {
                console.log(outer);
              }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;
    compiler.register_user_function_capture_bindings(&program.functions);
    compiler
        .reserve_function_constructor_implicit_global_bindings(&program)
        .expect("implicit globals should reserve");
    compiler.reserve_global_runtime_prototype_binding_globals();

    let function_declaration = program
        .functions
        .iter()
        .find(|function| {
            function.self_binding.as_deref() == Some("withCase")
                || function.top_level_binding.as_deref() == Some("withCase")
                || function.name == "withCase"
                || internal_function_name_hint(&function.name) == Some("withCase")
        })
        .cloned()
        .expect("expected withCase declaration");
    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_function_map
        .get(&function_declaration.name)
        .cloned()
        .expect("expected withCase function");
    let function_parameter_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_value_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_array_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_object_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        Some(&user_function),
        false,
        false,
        user_function.strict,
        &function_parameter_bindings,
        &function_parameter_value_bindings,
        &function_parameter_array_bindings,
        &function_parameter_object_bindings,
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&function_declaration.body)
        .expect("bindings should register");

    for statement in &function_declaration.body[..3] {
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    let Statement::With { object, .. } = &function_declaration.body[3] else {
        panic!("expected with statement");
    };
    let with_scope = function_compiler.canonicalize_with_scope_expression(object);
    function_compiler
        .state
        .emission
        .lexical_scopes
        .with_scopes
        .push(with_scope);

    assert_eq!(
        function_compiler
            .resolve_with_scope_binding("outer")
            .expect("with scope resolution should succeed"),
        Some(Expression::Identifier("env".to_string())),
    );
    assert_eq!(
        function_compiler.resolve_static_string_value(&Expression::Identifier("outer".to_string())),
        None,
    );
}

#[test]
fn snapshots_hidden_setter_receiver_updates_into_returned_bindings() {
    let program = frontend::parse(
        r#"
            let c = { _x: 1 };
            Object.defineProperty(c, "x", {
              set(v) { this._x = v; },
              get() { return this._x; },
              configurable: true,
            });
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;
    compiler.register_user_function_capture_bindings(&program.functions);
    compiler
        .reserve_function_constructor_implicit_global_bindings(&program)
        .expect("function constructor implicit globals should reserve");
    compiler.reserve_global_runtime_prototype_binding_globals();

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        program.strict,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let setter_binding = function_compiler
        .resolve_member_setter_binding(
            &Expression::Identifier("c".to_string()),
            &Expression::String("x".to_string()),
        )
        .expect("expected c.x setter binding");
    let hidden_receiver_name = "__snapshot_setter_receiver".to_string();
    let hidden_receiver = Expression::Identifier(hidden_receiver_name.clone());
    function_compiler.update_local_value_binding(
        &hidden_receiver_name,
        &Expression::Identifier("c".to_string()),
    );
    function_compiler.update_local_object_binding(
        &hidden_receiver_name,
        &Expression::Identifier("c".to_string()),
    );

    let (_, updated_bindings) = function_compiler
        .resolve_bound_snapshot_function_result_with_arguments_and_this(
            &setter_binding,
            &HashMap::new(),
            &[Expression::Number(2.0)],
            &hidden_receiver,
        )
        .expect("expected setter snapshot result");

    let updated_receiver = updated_bindings
        .get(&hidden_receiver_name)
        .expect("expected hidden receiver update");
    let object_binding = function_compiler
        .resolve_object_binding_from_expression(updated_receiver)
        .expect("expected hidden receiver object binding");
    assert_eq!(
        object_binding_lookup_value(&object_binding, &Expression::String("_x".to_string())),
        Some(&Expression::Number(2.0)),
    );
}

#[test]
fn collects_object_binding_for_lowered_object_method_destructuring_parameter() {
    let program = frontend::parse(
        r#"
            const o = { method([x, y, z]) { console.log(x, y, z); } };
            o.method([1, 2, 3]);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let (_, parameter_value_bindings, parameter_array_bindings, parameter_object_bindings) =
        compiler.collect_user_function_parameter_bindings(&program);

    let method_function = program
        .functions
        .iter()
        .find(|function| function.name.starts_with("__ayy_method_"))
        .expect("expected lowered object method");
    let method_object_bindings = parameter_object_bindings
        .get(&method_function.name)
        .expect("expected method object bindings");
    let method_value_bindings = parameter_value_bindings
        .get(&method_function.name)
        .expect("expected method value bindings");
    let method_array_bindings = parameter_array_bindings
        .get(&method_function.name)
        .expect("expected method array bindings");
    let param_name = method_function
        .params
        .first()
        .map(|param| param.name.as_str())
        .expect("expected lowered temp parameter");

    assert!(
        method_object_bindings
            .get(param_name)
            .and_then(|binding| binding.as_ref())
            .is_some(),
        "expected object binding for {param_name}; has_value_binding={}; has_array_binding={}; has_object_binding={}",
        method_value_bindings
            .get(param_name)
            .and_then(|binding| binding.as_ref())
            .is_some(),
        method_array_bindings
            .get(param_name)
            .and_then(|binding| binding.as_ref())
            .is_some(),
        method_object_bindings
            .get(param_name)
            .and_then(|binding| binding.as_ref())
            .is_some(),
    );
}

#[test]
fn initializes_local_object_binding_for_lowered_object_method_destructuring_parameter() {
    let program = frontend::parse(
        r#"
            const o = { method([x, y, z]) { console.log(x, y, z); } };
            o.method([1, 2, 3]);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;

    let method_function = program
        .functions
        .iter()
        .find(|function| function.name.starts_with("__ayy_method_"))
        .cloned()
        .expect("expected lowered object method");
    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_function_map
        .get(&method_function.name)
        .cloned()
        .expect("expected user function");
    let function_parameter_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_value_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_array_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_object_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let param_name = user_function
        .params
        .first()
        .cloned()
        .expect("expected lowered temp parameter");

    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        Some(&user_function),
        true,
        method_function.mapped_arguments,
        method_function.strict,
        &function_parameter_bindings,
        &function_parameter_value_bindings,
        &function_parameter_array_bindings,
        &function_parameter_object_bindings,
    )
    .expect("function compiler should initialize");

    assert!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .arrays
            .local_array_bindings
            .contains_key(&param_name),
        "expected local array binding for {param_name}"
    );
    assert!(
        function_compiler
            .state
            .speculation
            .static_semantics
            .objects
            .local_object_bindings
            .contains_key(&param_name),
        "expected local object binding for {param_name}"
    );
}

#[test]
fn resolves_lowered_object_method_destructuring_setup_bindings() {
    let program = frontend::parse(
        r#"
            const o = { method([x, y, z]) { console.log(x, y, z); } };
            o.method([1, 2, 3]);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;

    let method_function = program
        .functions
        .iter()
        .find(|function| function.name.starts_with("__ayy_method_"))
        .cloned()
        .expect("expected lowered object method");
    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_function_map
        .get(&method_function.name)
        .cloned()
        .expect("expected user function");
    let function_parameter_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_value_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_array_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_object_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        Some(&user_function),
        true,
        method_function.mapped_arguments,
        method_function.strict,
        &function_parameter_bindings,
        &function_parameter_value_bindings,
        &function_parameter_array_bindings,
        &function_parameter_object_bindings,
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&method_function.body)
        .expect("bindings should register");

    for statement in &method_function.body {
        if matches!(statement, Statement::Print { .. }) {
            break;
        }
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    assert_eq!(
        function_compiler.resolve_static_number_value(&Expression::Identifier("x".to_string())),
        Some(1.0)
    );
    assert_eq!(
        function_compiler.resolve_static_number_value(&Expression::Identifier("y".to_string())),
        Some(2.0)
    );
    assert_eq!(
        function_compiler.resolve_static_number_value(&Expression::Identifier("z".to_string())),
        Some(3.0)
    );
}

#[test]
fn resolves_lowered_class_method_destructuring_setup_bindings() {
    let program = frontend::parse(
        r#"
            class C {
              method([x, y, z]) { console.log(x, y, z); }
            }
            new C().method([1, 2, 3]);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;

    let method_function = program
        .functions
        .iter()
        .find(|function| function.name.starts_with("__ayy_class_method_"))
        .cloned()
        .expect("expected lowered class method");
    let user_function = compiler
        .state
        .function_registry
        .catalog
        .user_function_map
        .get(&method_function.name)
        .cloned()
        .expect("expected user function");
    let function_parameter_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_value_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_array_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_object_bindings = compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        Some(&user_function),
        true,
        method_function.mapped_arguments,
        method_function.strict,
        &function_parameter_bindings,
        &function_parameter_value_bindings,
        &function_parameter_array_bindings,
        &function_parameter_object_bindings,
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&method_function.body)
        .expect("bindings should register");

    for statement in &method_function.body {
        if matches!(statement, Statement::Print { .. }) {
            break;
        }
        function_compiler
            .emit_statement(statement)
            .expect("setup statement should emit");
    }

    assert_eq!(
        function_compiler.resolve_static_number_value(&Expression::Identifier("x".to_string())),
        Some(1.0)
    );
    assert_eq!(
        function_compiler.resolve_static_number_value(&Expression::Identifier("y".to_string())),
        Some(2.0)
    );
    assert_eq!(
        function_compiler.resolve_static_number_value(&Expression::Identifier("z".to_string())),
        Some(3.0)
    );
}

#[test]
fn emitting_accessor_member_assignment_updates_tracked_receiver_object() {
    let program = frontend::parse(
        r#"
            let c = { _x: 1 };
            Object.defineProperty(c, "x", {
              set(v) { this._x = v; },
              get() { return this._x; },
              configurable: true,
            });
            c.x = 2;
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    let (
        parameter_bindings,
        parameter_value_bindings,
        parameter_array_bindings,
        parameter_object_bindings,
    ) = compiler.collect_user_function_parameter_bindings(&program);
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .function_bindings_by_function = parameter_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .value_bindings_by_function = parameter_value_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .array_bindings_by_function = parameter_array_bindings;
    compiler
        .state
        .function_registry
        .analysis
        .user_function_parameter_analysis
        .object_bindings_by_function = parameter_object_bindings;
    compiler.register_user_function_capture_bindings(&program.functions);
    compiler
        .reserve_function_constructor_implicit_global_bindings(&program)
        .expect("function constructor implicit globals should reserve");
    compiler.reserve_global_runtime_prototype_binding_globals();

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        program.strict,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .register_bindings(&program.statements)
        .expect("bindings should register");

    for statement in &program.statements {
        function_compiler
            .emit_statement(statement)
            .expect("statement should emit");
    }

    let object_binding = function_compiler
        .resolve_object_binding_from_expression(&Expression::Identifier("c".to_string()))
        .expect("expected tracked c object binding");
    assert_eq!(
        object_binding_lookup_value(&object_binding, &Expression::String("_x".to_string())),
        Some(&Expression::Number(2.0)),
    );
}

#[test]
fn direct_eval_iife_is_not_inlineable() {
    let program = frontend::parse(
        r#"
            var initialNew, postAssignment, outerNewReadThrows;
            (function() {
              eval("initialNew = f; f = 5; postAssignment = f; function f() { return 33; }");
            }());
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);
    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        program.strict,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    let iife = function_compiler
        .backend
        .function_registry
        .catalog
        .user_function_map
        .get("__ayy_fnexpr_1")
        .cloned()
        .expect("expected iife");

    assert!(
        !function_compiler.can_inline_user_function_call(&iife, &[]),
        "direct-eval IIFE should not be inlineable; summary_present={}",
        iife.inline_summary.is_some()
    );
}

#[test]
fn direct_eval_outer_assignments_normalize_to_plain_global_names() {
    let eval_source = "initialNew = f; f = 5; postAssignment = f; function f() { return 33; }";
    let program = frontend::parse(&format!(
        r#"
            var initialNew, postAssignment;
            (function() {{
              eval({eval_source:?});
            }}());
        "#
    ))
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let iife = compiler
        .state
        .function_registry
        .catalog
        .user_function_map
        .get("__ayy_fnexpr_1")
        .cloned()
        .expect("expected iife");
    let function_compiler = FunctionCompiler::new(
        &mut compiler,
        Some(&iife),
        true,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");

    let mut eval_program = function_compiler
        .parse_eval_program_in_current_function_context(eval_source)
        .expect("eval program should parse in current function context");
    namespace_eval_program_internal_function_names(
        &mut eval_program,
        function_compiler
            .state
            .speculation
            .execution_context
            .current_user_function_name
            .as_deref(),
        eval_source,
    );
    function_compiler.normalize_eval_scoped_bindings_to_source_names(&mut eval_program);

    let assignment_names = eval_program
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Assign { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        assignment_names,
        vec![
            "initialNew".to_string(),
            "f".to_string(),
            "postAssignment".to_string(),
        ]
    );
}

#[test]
fn direct_eval_iife_compile_updates_global_metadata_for_outer_assignments() {
    let program = frontend::parse(
        r#"
            var initialNew, postAssignment;
            (function() {
              eval("initialNew = f; f = 5; postAssignment = f; function f() { return 33; }");
            }());
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let iife_declaration = program
        .functions
        .iter()
        .find(|function| function.name == "__ayy_fnexpr_1")
        .cloned()
        .expect("expected iife declaration");
    let iife = compiler
        .state
        .function_registry
        .catalog
        .user_function_map
        .get("__ayy_fnexpr_1")
        .cloned()
        .expect("expected iife user function");

    let _compiled = FunctionCompiler::new(
        &mut compiler,
        Some(&iife),
        true,
        false,
        false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize")
    .compile(&iife_declaration.body)
    .expect("iife should compile");

    let Some(Expression::Identifier(initial_new_binding)) = compiler
        .state
        .global_semantics
        .values
        .value_bindings
        .get("initialNew")
    else {
        panic!(
            "expected initialNew to resolve to an internal function identifier, got {:#?}",
            compiler
                .state
                .global_semantics
                .values
                .value_bindings
                .get("initialNew")
        );
    };
    assert_eq!(internal_function_name_hint(initial_new_binding), None);
    assert!(
        initial_new_binding.starts_with("__ayy_fnstmt_")
            && initial_new_binding.contains("__evalctx_"),
        "unexpected initialNew binding: {initial_new_binding}"
    );
    assert_eq!(
        compiler
            .state
            .global_semantics
            .values
            .value_bindings
            .get("postAssignment"),
        Some(&Expression::Number(5.0))
    );
}

#[test]
fn resolves_class_super_method_binding_from_home_object_metadata() {
    let program = frontend::parse(
        r#"
            class A {
              method() { return "sup"; }
            }
            class B extends A {
              method() { return super.method(); }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let base_binding = compiler
        .state
        .global_semantics
        .members
        .member_function_bindings
        .iter()
        .find_map(|(key, binding)| {
            let target_name = match &key.target {
                crate::backend::direct_wasm::MemberFunctionBindingTarget::Prototype(name) => name,
                _ => return None,
            };
            let property_name = match &key.property {
                crate::backend::direct_wasm::MemberFunctionBindingProperty::String(name) => name,
                _ => return None,
            };
            (target_name == "A" && property_name == "method").then_some(binding.clone())
        })
        .expect("expected A.prototype.method binding");

    let LocalFunctionBinding::User(derived_method_name) = compiler
        .state
        .global_semantics
        .members
        .member_function_bindings
        .iter()
        .find_map(|(key, binding)| {
            let target_name = match &key.target {
                crate::backend::direct_wasm::MemberFunctionBindingTarget::Prototype(name) => name,
                _ => return None,
            };
            let property_name = match &key.property {
                crate::backend::direct_wasm::MemberFunctionBindingProperty::String(name) => name,
                _ => return None,
            };
            (target_name == "B" && property_name == "method").then_some(binding.clone())
        })
        .expect("expected B.prototype.method binding")
    else {
        panic!("expected derived prototype method to use a user function");
    };

    assert_eq!(
        compiler
            .state
            .function_registry
            .catalog
            .user_function_map
            .get(&derived_method_name)
            .and_then(|function| function.home_object_binding.as_deref()),
        Some("B.prototype")
    );

    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        program.strict,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");
    function_compiler
        .state
        .speculation
        .execution_context
        .current_user_function_name = Some(derived_method_name);

    assert!(
        matches!(
            function_compiler.resolve_super_base_expression_with_context(
                function_compiler.state.speculation.execution_context.current_user_function_name.as_deref(),
            ),
            Some(Expression::Member { property, .. })
                if matches!(property.as_ref(), Expression::String(property_name) if property_name == "prototype")
        ),
        "expected derived method to resolve a prototype-based super home object"
    );

    assert_eq!(
        function_compiler.resolve_super_function_binding(&Expression::String("method".to_string())),
        Some(base_binding)
    );
}

#[test]
fn resolves_static_numeric_class_super_bindings_from_class_home_object_metadata() {
    let program = frontend::parse(
        r#"
            class B {
              static 4() { return 4; }
              static get 5() { return 5; }
            }
            class C extends B {
              static 4() { return super[4](); }
              static get 5() { return super[5]; }
            }
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);
    compiler.register_user_function_capture_bindings(&program.functions);

    let base_method_binding = compiler
        .state
        .global_semantics
        .members
        .member_function_bindings
        .iter()
        .find_map(|(key, binding)| {
            let target_name = match &key.target {
                crate::backend::direct_wasm::MemberFunctionBindingTarget::Identifier(name) => name,
                _ => return None,
            };
            let property_name = match &key.property {
                crate::backend::direct_wasm::MemberFunctionBindingProperty::String(name) => name,
                _ => return None,
            };
            (target_name == "B" && property_name == "4").then_some(binding.clone())
        })
        .expect("expected B static numeric method binding");
    let base_getter_binding = compiler
        .state
        .global_semantics
        .members
        .member_getter_bindings
        .iter()
        .find_map(|(key, binding)| {
            let target_name = match &key.target {
                crate::backend::direct_wasm::MemberFunctionBindingTarget::Identifier(name) => name,
                _ => return None,
            };
            let property_name = match &key.property {
                crate::backend::direct_wasm::MemberFunctionBindingProperty::String(name) => name,
                _ => return None,
            };
            (target_name == "B" && property_name == "5").then_some(binding.clone())
        })
        .expect("expected B static numeric getter binding");

    let derived_static_method_name = compiler
        .state
        .global_semantics
        .members
        .member_function_bindings
        .iter()
        .find_map(|(key, binding)| {
            let target_name = match &key.target {
                crate::backend::direct_wasm::MemberFunctionBindingTarget::Identifier(name) => name,
                _ => return None,
            };
            let property_name = match &key.property {
                crate::backend::direct_wasm::MemberFunctionBindingProperty::String(name) => name,
                _ => return None,
            };
            let LocalFunctionBinding::User(function_name) = binding else {
                return None;
            };
            (target_name == "C" && property_name == "4").then_some(function_name.clone())
        })
        .expect("expected C static numeric method binding");

    let derived_static_getter_name = compiler
        .state
        .global_semantics
        .members
        .member_getter_bindings
        .iter()
        .find_map(|(key, binding)| {
            let target_name = match &key.target {
                crate::backend::direct_wasm::MemberFunctionBindingTarget::Identifier(name) => name,
                _ => return None,
            };
            let property_name = match &key.property {
                crate::backend::direct_wasm::MemberFunctionBindingProperty::String(name) => name,
                _ => return None,
            };
            let LocalFunctionBinding::User(function_name) = binding else {
                return None;
            };
            (target_name == "C" && property_name == "5").then_some(function_name.clone())
        })
        .expect("expected C static numeric getter binding");

    assert_eq!(
        compiler
            .state
            .function_registry
            .catalog
            .user_function_map
            .get(&derived_static_method_name)
            .and_then(|function| function.home_object_binding.as_deref()),
        Some("C")
    );
    let mut function_compiler = FunctionCompiler::new(
        &mut compiler,
        None,
        false,
        false,
        program.strict,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
    .expect("function compiler should initialize");

    function_compiler
        .state
        .speculation
        .execution_context
        .current_user_function_name = Some(derived_static_method_name);
    assert!(
        function_compiler
            .resolve_super_base_expression_with_context(
                function_compiler
                    .state
                    .speculation
                    .execution_context
                    .current_user_function_name
                    .as_deref()
            )
            .is_some(),
        "expected static method super base"
    );
    assert_eq!(
        function_compiler.resolve_super_function_binding(&Expression::Number(4.0)),
        Some(base_method_binding)
    );

    function_compiler
        .state
        .speculation
        .execution_context
        .current_user_function_name = Some(derived_static_getter_name);
    assert!(
        function_compiler
            .resolve_super_base_expression_with_context(
                function_compiler
                    .state
                    .speculation
                    .execution_context
                    .current_user_function_name
                    .as_deref()
            )
            .is_some(),
        "expected static getter super base"
    );
    assert_eq!(
        function_compiler.resolve_super_getter_binding(&Expression::Number(5.0)),
        Some(base_getter_binding)
    );
}

#[test]
fn full_compile_keeps_eval_local_global_metadata_after_iife_call() {
    let program = frontend::parse(
        r#"
            var initialNew, postAssignment;
            (function() {
              eval("initialNew = f; f = 5; postAssignment = f; function f() { return 33; }");
            }());
            console.log(typeof initialNew, postAssignment);
        "#,
    )
    .expect("program should parse");

    let mut compiler = DirectWasmCompiler::default();
    compiler.compile(&program).expect("program should compile");

    assert!(
        matches!(
            compiler.state.global_semantics.values.value_bindings.get("initialNew"),
            Some(Expression::Identifier(name))
                if name.starts_with("__ayy_fnstmt_") && name.contains("__evalctx_")
        ),
        "unexpected initialNew binding after full compile: {:#?}",
        compiler
            .state
            .global_semantics
            .values
            .value_bindings
            .get("initialNew")
    );
    assert_eq!(
        compiler
            .state
            .global_semantics
            .values
            .value_bindings
            .get("postAssignment"),
        Some(&Expression::Number(5.0))
    );
    assert_eq!(
        compiler
            .state
            .global_semantics
            .functions
            .function_bindings
            .get("initialNew"),
        Some(&LocalFunctionBinding::User(
            compiler
                .state
                .global_semantics
                .values
                .value_bindings
                .get("initialNew")
                .and_then(|value| match value {
                    Expression::Identifier(name) => Some(name.clone()),
                    _ => None,
                })
                .expect("expected function identifier for initialNew")
        ))
    );
}
