use std::collections::{HashMap, HashSet};

use crate::frontend;
use crate::ir::hir::CallArgument;

use super::{
    DirectWasmCompiler, Expression, FunctionCompiler, LocalFunctionBinding,
    OrdinaryToPrimitiveAnalysis, SimpleGeneratorStepOutcome, Statement,
    collect_eval_local_function_declarations, internal_function_name_hint,
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
    compiler
        .register_functions(&program.functions)
        .expect("functions should register");
    compiler
        .register_static_eval_functions(&program)
        .expect("static eval functions should register");
    compiler.register_global_bindings(&program.statements);
    compiler.register_global_function_bindings(&program.functions);

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
    compiler.user_function_parameter_bindings = parameter_bindings;
    compiler.user_function_parameter_value_bindings = parameter_value_bindings;
    compiler.user_function_parameter_array_bindings = parameter_array_bindings;
    compiler.user_function_parameter_object_bindings = parameter_object_bindings;
    compiler.register_user_function_capture_bindings(&program.functions);
    compiler
        .reserve_function_constructor_implicit_global_bindings(&program)
        .expect("implicit globals should reserve");
    compiler.reserve_global_runtime_prototype_binding_globals();

    let user_function = compiler
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
        .user_function_parameter_bindings
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_value_bindings = compiler
        .user_function_parameter_value_bindings
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_array_bindings = compiler
        .user_function_parameter_array_bindings
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_object_bindings = compiler
        .user_function_parameter_object_bindings
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

    let (steps, completion_effects) = function_compiler
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
        .module
        .user_function_map
        .get(&function_name)
        .cloned()
        .expect("expected method user function");
    let function_declaration = function_compiler
        .module
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
        .analyze_simple_generator_statements(&substituted_body, &mut steps, &mut effects)
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
            .emit_fresh_simple_generator_next_call(method_call.as_ref())
            .expect("fresh next helper should emit"),
        "expected fresh next helper to handle class expression method call"
    );
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
        .registered_function_declarations
        .iter()
        .find(|function| function.self_binding.as_deref() == Some("g"))
        .map(|function| function.name.clone())
        .expect("expected named generator expression function");

    let matching_capture_maps = compiler
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
        function_compiler.local_value_bindings.get("trace"),
        None,
        "unexpected stale local trace binding: {:?}",
        function_compiler.local_value_bindings.get("trace")
    );

    function_compiler
        .emit_print_value(&Expression::Identifier("trace".to_string()))
        .expect("print should emit");
    assert!(
        function_compiler
            .module
            .string_data
            .iter()
            .any(|(_, bytes)| bytes == b"99"),
        "expected print emission to intern 99, found {:?}",
        function_compiler
            .module
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
        compiler.string_data.iter().any(|(_, bytes)| bytes == b"99"),
        "expected compile path to intern 99, found {:?}",
        compiler
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
    compiler.user_function_parameter_bindings = parameter_bindings;
    compiler.user_function_parameter_value_bindings = parameter_value_bindings;
    compiler.user_function_parameter_array_bindings = parameter_array_bindings;
    compiler.user_function_parameter_object_bindings = parameter_object_bindings;
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

    assert_eq!(function_compiler.current_user_function_name, None);
    assert_eq!(
        function_compiler
            .module
            .global_value_bindings
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
    compiler.user_function_parameter_bindings = parameter_bindings;
    compiler.user_function_parameter_value_bindings = parameter_value_bindings;
    compiler.user_function_parameter_array_bindings = parameter_array_bindings;
    compiler.user_function_parameter_object_bindings = parameter_object_bindings;
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
            .module
            .global_value_bindings
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
    compiler.user_function_parameter_bindings = parameter_bindings;
    compiler.user_function_parameter_value_bindings = parameter_value_bindings;
    compiler.user_function_parameter_array_bindings = parameter_array_bindings;
    compiler.user_function_parameter_object_bindings = parameter_object_bindings;
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
    compiler.user_function_parameter_bindings = parameter_bindings;
    compiler.user_function_parameter_value_bindings = parameter_value_bindings;
    compiler.user_function_parameter_array_bindings = parameter_array_bindings;
    compiler.user_function_parameter_object_bindings = parameter_object_bindings;
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
        .user_function_map
        .get(&function_declaration.name)
        .cloned()
        .expect("expected withCase function");
    let function_parameter_bindings = compiler
        .user_function_parameter_bindings
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_value_bindings = compiler
        .user_function_parameter_value_bindings
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_array_bindings = compiler
        .user_function_parameter_array_bindings
        .get(&user_function.name)
        .cloned()
        .unwrap_or_default();
    let function_parameter_object_bindings = compiler
        .user_function_parameter_object_bindings
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
    function_compiler.with_scopes.push(with_scope);

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
        .module
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
        function_compiler.current_user_function_name.as_deref(),
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

    let Some(Expression::Identifier(initial_new_binding)) =
        compiler.global_value_bindings.get("initialNew")
    else {
        panic!(
            "expected initialNew to resolve to an internal function identifier, got {:#?}",
            compiler.global_value_bindings.get("initialNew")
        );
    };
    assert_eq!(internal_function_name_hint(initial_new_binding), None);
    assert!(
        initial_new_binding.starts_with("__ayy_fnstmt_")
            && initial_new_binding.contains("__evalctx_"),
        "unexpected initialNew binding: {initial_new_binding}"
    );
    assert_eq!(
        compiler.global_value_bindings.get("postAssignment"),
        Some(&Expression::Number(5.0))
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
            compiler.global_value_bindings.get("initialNew"),
            Some(Expression::Identifier(name))
                if name.starts_with("__ayy_fnstmt_") && name.contains("__evalctx_")
        ),
        "unexpected initialNew binding after full compile: {:#?}",
        compiler.global_value_bindings.get("initialNew")
    );
    assert_eq!(
        compiler.global_value_bindings.get("postAssignment"),
        Some(&Expression::Number(5.0))
    );
    assert_eq!(
        compiler.global_function_bindings.get("initialNew"),
        Some(&LocalFunctionBinding::User(
            compiler
                .global_value_bindings
                .get("initialNew")
                .and_then(|value| match value {
                    Expression::Identifier(name) => Some(name.clone()),
                    _ => None,
                })
                .expect("expected function identifier for initialNew")
        ))
    );
}
