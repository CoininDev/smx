use smx::value::Value;
use ordered_float::NotNan;
use im::HashMap;

fn main() {
    println!("=== Exemplos de Serialização e Desserialização de Value para JSON ===\n");

    // Número
    let num = Value::Num(NotNan::new(42.5).unwrap());
    let json_num = serde_json::to_string_pretty(&num).unwrap();
    println!("Número (42.5):");
    println!("{}\n", json_num);
    
    // Desserializar número
    let deserialized_num: Value = serde_json::from_str(&json_num).unwrap();
    println!("Desserializado: {:?}\n", deserialized_num);

    // String
    let string = Value::Str("Olá, mundo!".to_string());
    let json_string = serde_json::to_string_pretty(&string).unwrap();
    println!("String:");
    println!("{}\n", json_string);

    // Booleano
    let boolean = Value::Bool(true);
    let json_bool = serde_json::to_string_pretty(&boolean).unwrap();
    println!("Booleano:");
    println!("{}\n", json_bool);

    // Nil
    let nil = Value::Nil;
    let json_nil = serde_json::to_string_pretty(&nil).unwrap();
    println!("Nil:");
    println!("{}\n", json_nil);

    // Pair (lista cons)
    let pair = Value::Pair(
        Box::new(Value::Num(NotNan::new(1.0).unwrap())),
        Box::new(Value::Pair(
            Box::new(Value::Num(NotNan::new(2.0).unwrap())),
            Box::new(Value::Nil),
        )),
    );
    let json_pair = serde_json::to_string_pretty(&pair).unwrap();
    println!("Pair (1, (2, nil)):");
    println!("{}\n", json_pair);
    
    // Desserializar Pair
    let deserialized_pair: Value = serde_json::from_str(&json_pair).unwrap();
    println!("Desserializado: {:?}\n", deserialized_pair);

    // Builtin
    let builtin = Value::Builtin("map".to_string());
    let json_builtin = serde_json::to_string_pretty(&builtin).unwrap();
    println!("Builtin (map):");
    println!("{}\n", json_builtin);
    
    // Desserializar Builtin
    let deserialized_builtin: Value = serde_json::from_str(&json_builtin).unwrap();
    println!("Desserializado: {:?}\n", deserialized_builtin);

    // Environment (HashMap)
    println!("\n=== Environment (HashMap) ===\n");
    let mut env_map = HashMap::new();
    env_map.insert("x".to_string(), Value::Num(NotNan::new(10.0).unwrap()));
    env_map.insert("y".to_string(), Value::Str("hello".to_string()));
    env_map.insert("z".to_string(), Value::Bool(false));
    
    let environment = Value::Environment(env_map);
    let json_env = serde_json::to_string_pretty(&environment).unwrap();
    println!("Environment:");
    println!("{}\n", json_env);
    
    // Desserializar Environment
    let deserialized_env: Value = serde_json::from_str(&json_env).unwrap();
    println!("Desserializado:");
    println!("{:?}\n", deserialized_env);

    // Nested Environment
    println!("\n=== Nested Environment ===\n");
    let mut inner_env = HashMap::new();
    inner_env.insert("a".to_string(), Value::Num(NotNan::new(1.0).unwrap()));
    inner_env.insert("b".to_string(), Value::Num(NotNan::new(2.0).unwrap()));
    
    let mut outer_env = HashMap::new();
    outer_env.insert("inner".to_string(), Value::Environment(inner_env));
    outer_env.insert("outer_value".to_string(), Value::Str("test".to_string()));
    
    let nested_env = Value::Environment(outer_env);
    let json_nested = serde_json::to_string_pretty(&nested_env).unwrap();
    println!("Nested Environment:");
    println!("{}\n", json_nested);
    
    // Desserializar Nested Environment
    let deserialized_nested: Value = serde_json::from_str(&json_nested).unwrap();
    println!("Desserializado:");
    println!("{:?}\n", deserialized_nested);

    // Lambda (não é completamente serializável)
    let lambda = Value::Lambda(
        smx::value::Pattern::Wildcard,
        smx::ast::Expression::Var(vec!["x".to_string()]),
        im::HashMap::new(),
        vec![],
    );
    let json_lambda = serde_json::to_string_pretty(&lambda).unwrap();
    println!("Lambda (não serializado completamente):");
    println!("{}\n", json_lambda);

    // Testar round-trip com dados simples
    println!("\n=== Round-Trip Test ===\n");
    let original = Value::Pair(
        Box::new(Value::Str("hello".to_string())),
        Box::new(Value::Pair(
            Box::new(Value::Bool(true)),
            Box::new(Value::Nil),
        )),
    );
    
    let json = serde_json::to_string_pretty(&original).unwrap();
    println!("JSON original:\n{}\n", json);
    
    let restored: Value = serde_json::from_str(&json).unwrap();
    println!("Restaurado: {:?}\n", restored);
    
    if format!("{:?}", original) == format!("{:?}", restored) {
        println!("✓ Round-trip bem-sucedido!");
    } else {
        println!("✗ Round-trip falhou!");
    }
}

