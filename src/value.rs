use crate::{ast::*, eval::*};
use std::{any::Any, rc::Rc, fmt::Debug};
use std::cmp::Ordering;
use ordered_float::NotNan;
use num_bigint::{BigInt, BigUint};
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde_json;

#[derive(Default, Clone)]
pub struct Ambient {
    pub vars: Environment,
    pub rsrcs: Environment,
    pub natives: Vec<Rc<dyn Any>>,
    pub custom_resources: Vec<Rc<dyn IoObject>>,
}

impl Ambient {
    pub fn extend(&mut self, other: &Ambient) {
        self.vars.extend(other.vars.clone());
        self.rsrcs.extend(other.rsrcs.clone());
        self.natives.extend(other.natives.clone());
        self.custom_resources.extend(other.custom_resources.clone());
    }

    pub fn eject(&mut self, other: &Ambient) {
        for k in other.vars.keys() {
            self.vars.remove(k);
        }
        for k in other.rsrcs.keys() {
            self.rsrcs.remove(k);
        }
        // Note: not ejecting natives or custom_resources, as they might be shared
    }

    pub fn eject_vars(&mut self, vars: &Environment) {
        for k in vars.keys() {
            self.vars.remove(k);
        }
    }

    pub fn add_custom_resource(&mut self, res: std::rc::Rc<dyn IoObject>) {
        let name = res.name().to_string();
        self.custom_resources.push(res.clone());
        self.rsrcs.insert(name.clone(), Value::Builtin(name.clone())); 
    }
}

pub trait IoObject {
    fn redirect(&mut self, function: Vec<String>, value: Value, amb: &mut Ambient) -> EvalResult<Value>;
    fn name(&self) -> &str;
}


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NumericValue {
    Float(NotNan<f64>),
    Int(BigInt),
    Uint(BigUint),
}

impl std::fmt::Display for NumericValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Float(x) => write!(f, "{x}"),
            Self::Int(x)   => write!(f, "{x}"),
            Self::Uint(x)  => write!(f, "{x}"),
        }
    }
}

impl Serialize for NumericValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            NumericValue::Float(f) => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "float")?;
                map.serialize_entry("value", &f.into_inner())?;
                map.end()
            }
            NumericValue::Int(i) => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "int")?;
                map.serialize_entry("value", &i.to_string())?;
                map.end()
            }
            NumericValue::Uint(u) => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "uint")?;
                map.serialize_entry("value", &u.to_string())?;
                map.end()
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    Num(NotNan<f64>),
    StrictNum(NumericType, NumericValue),
    Str(String),
    Lambda(Pattern, Expression, Environment, Vec<String>),
    Environment(Environment),
    Type(PatternType),
    Frozen(Expression),
    Pattern(Pattern),
    Builtin(String),
    Bool(bool),
    Pair(Box<Value>, Box<Value>),
    Native(usize),
    Nil,
}

impl Value {
    pub fn pair_to_vec(&self) -> Vec<Value> {
        let mut result = Vec::new();
        let mut current = self;

        if let Value::Nil = current {
            return result;
        }

        loop {
            match current {
                Value::Pair(car, cdr) => {
                    result.push((**car).clone());
                    current = cdr;
                }
                Value::Nil => break,
                other => {
                    result.push(other.clone());
                    break;
                }
            }
        }

        result
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum Pattern {
    Name(String),
    TypedName(String, PatternType),
    Value(Box<Value>),
    Pair(Box<Pattern>, Box<Pattern>),
    Environment(Vec<(String, Pattern)>),
    Wildcard,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum PatternType {
    Any,
    Nil,
    Pattern,
    Number,
    StrictNumber(NumericType),
    String,
    Lambda,
    Bool,
    Environment,
    EnvironmentWithSchema(Vec<(String, PatternType)>),
    Frozen,
    List(Vec<PatternType>),
}

impl std::fmt::Display for PatternType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatternType::Any => write!(f, "any"),
            PatternType::Nil => write!(f, "nil"),
            PatternType::Number => write!(f, "number"),
            PatternType::StrictNumber(t) => write!(f, "{}", t),
            PatternType::String => write!(f, "string"),
            PatternType::Bool => write!(f, "bool"),
            PatternType::Lambda => write!(f, "fn"),
            PatternType::Pattern => write!(f, "pattern"),
            PatternType::Environment => write!(f, "env"),
            PatternType::EnvironmentWithSchema(schema) => {
                write!(f, "{{")?;
                for (i, (k, t)) in schema.iter().enumerate() {
                    if i > 0 { write!(f, "; ")?; }
                    write!(f, "{} ~ {}", k, t)?;
                }
                write!(f, "}}")
            }
            PatternType::Frozen => write!(f, "frozen"),
            PatternType::List(items) => {
                let joined = items
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(" | ");
                write!(f, "[{}]", joined)
            }
        }
    }
}
impl std::fmt::Display for Pattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(x)         => write!(f, "{x}"),
            Self::TypedName(x, t) => write!(f, "{x} ~ {t}"),
            Self::Value(x)        => write!(f, "{}", *x),
            Self::Pair(a, b)      => write!(f, "({}, {})", *a, *b),
            Self::Environment(e)  => {
                write!(f, "{{")?;
                for (i, (k, p)) in e.iter().enumerate() {
                    if i > 0 { write!(f, "; ")?; }
                    write!(f, "{} = {}", k, p)?;
                }
                write!(f, "}}")
            }
            Self::Wildcard        => write!(f, "_"),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Num(x) => write!(f, "{x}"),
            Self::StrictNum(t, v) => write!(f, "{}{}", v, t),
            Self::Str(s) => write!(f, "\"{s}\""),
            Self::Lambda(arg, body, _, res) => {
                if res.is_empty() {
                    write!(f, "(\\{arg}. {body})")
                } else {
                    write!(f, "(\\{arg} @{{")?;
                    for (i, r) in res.iter().enumerate() {
                        if i > 0 { write!(f, ", ")?; }
                        write!(f, "{}", r)?;
                    }
                    write!(f, "}}. {body})")
                }
            },
            Self::Builtin(b) => write!(f, "{b}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Pattern(p) => write!(f, "#{p}"),
            Self::Pair(a, b) => write!(f, "({}, {})", *a, *b),
            Self::Type(p) => write!(f, "type {p}"),
            Self::Frozen(e) => write!(f, "'{e}"),
            Self::Native(a) => write!(f, "<#{a:02}>"),
            Self::Environment(e) => {
                write!(f, "{{")?;
                for (k, v) in e {
                    write!(f, " {k} =")?;
                    write!(f, " {v}; ")?;
                }
                write!(f, "}}")
            }
            Self::Nil => write!(f, "nil")
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Value::Num(a), Value::Num(b)) => a.partial_cmp(b),
            (Value::StrictNum(t1, v1), Value::StrictNum(t2, v2)) if t1 == t2 => {
                match (v1, v2) {
                    (NumericValue::Float(a), NumericValue::Float(b)) => a.partial_cmp(b),
                    (NumericValue::Int(a), NumericValue::Int(b)) => a.partial_cmp(b),
                    (NumericValue::Uint(a), NumericValue::Uint(b)) => a.partial_cmp(b),
                    _ => None,
                }
            }
            (Value::Bool(a), Value::Bool(b)) => a.partial_cmp(b),
            (Value::Nil, Value::Nil) => Some(Ordering::Equal),
            _ => None,
        }
    }
}

impl std::ops::Add for Value {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Num(a), Value::Num(b)) => Value::Num(a + b),
            (Value::StrictNum(t1, v1), Value::StrictNum(t2, v2)) if t1 == t2 => {
                match (v1, v2) {
                    (NumericValue::Float(a), NumericValue::Float(b)) => Value::StrictNum(t1, NumericValue::Float(a + b)),
                    (NumericValue::Int(a), NumericValue::Int(b)) => Value::StrictNum(t1, NumericValue::Int(a + b)),
                    (NumericValue::Uint(a), NumericValue::Uint(b)) => Value::StrictNum(t1, NumericValue::Uint(a + b)),
                    _ => Value::Nil,
                }
            }
            (Value::Str(a), Value::Str(b)) => Value::Str(format!("{a}{b}")),
            (Value::Str(s), other) => Value::Str(format!("{s}{other}")),
            (other, Value::Str(s)) => Value::Str(format!("{other}{s}")),
            (Value::Environment(mut a), Value::Environment(b)) => {
                a.extend(b);
                Value::Environment(a)
            }
            _ => Value::Nil,
        }
    }
}
impl std::ops::Neg for Value {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Value::Num(a) => Value::Num(-a),
            Value::StrictNum(t, NumericValue::Float(a)) => Value::StrictNum(t, NumericValue::Float(-a)),
            Value::StrictNum(t, NumericValue::Int(a)) => Value::StrictNum(t, NumericValue::Int(-a)),
            _ => Value::Nil,
        }
    }
}

impl std::ops::Not for Value {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Value::Bool(b) => Value::Bool(!b),
            _ => Value::Nil,
        }
    }
}

impl std::ops::Sub for Value {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Num(a), Value::Num(b)) => Value::Num(a - b),
            (Value::StrictNum(t1, v1), Value::StrictNum(t2, v2)) if t1 == t2 => {
                match (v1, v2) {
                    (NumericValue::Float(a), NumericValue::Float(b)) => Value::StrictNum(t1, NumericValue::Float(a - b)),
                    (NumericValue::Int(a), NumericValue::Int(b)) => Value::StrictNum(t1, NumericValue::Int(a - b)),
                    (NumericValue::Uint(a), NumericValue::Uint(b)) => Value::StrictNum(t1, NumericValue::Uint(a - b)),
                    _ => Value::Nil,
                }
            }
            (Value::Environment(mut a), rhs) => {
                let keys = rhs.pair_to_vec();
                for k in keys {
                    match k {
                        Value::Str(s) => { a.remove(&s); }
                        Value::Frozen(Expression::Var(v)) if v.len() == 1 => { a.remove(&v[0]); }
                        _ => {}
                    }
                }
                Value::Environment(a)
            }
            _ => Value::Nil,
        }
    }
}

impl std::ops::Mul for Value {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Num(a), Value::Num(b)) => Value::Num(a * b),
            (Value::StrictNum(t1, v1), Value::StrictNum(t2, v2)) if t1 == t2 => {
                match (v1, v2) {
                    (NumericValue::Float(a), NumericValue::Float(b)) => Value::StrictNum(t1, NumericValue::Float(a * b)),
                    (NumericValue::Int(a), NumericValue::Int(b)) => Value::StrictNum(t1, NumericValue::Int(a * b)),
                    (NumericValue::Uint(a), NumericValue::Uint(b)) => Value::StrictNum(t1, NumericValue::Uint(a * b)),
                    _ => Value::Nil,
                }
            }
            _ => Value::Nil,
        }
    }
}


impl std::ops::Div for Value {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Num(a), Value::Num(b)) => Value::Num(a / b),
            (Value::StrictNum(t1, v1), Value::StrictNum(t2, v2)) if t1 == t2 => {
                match (v1, v2) {
                    (NumericValue::Float(a), NumericValue::Float(b)) => Value::StrictNum(t1, NumericValue::Float(a / b)),
                    (NumericValue::Int(a), NumericValue::Int(b)) => Value::StrictNum(t1, NumericValue::Int(a / b)),
                    (NumericValue::Uint(a), NumericValue::Uint(b)) => Value::StrictNum(t1, NumericValue::Uint(a / b)),
                    _ => Value::Nil,
                }
            }
            _ => Value::Nil,
        }
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Num(NotNan::new(f).expect("NaN is not allowed in SMX"))
    }
}

impl From<i32> for Value {
    fn from(i: i32) -> Self {
        Value::Num(NotNan::new(i as f64).unwrap())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_string())
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;

        match self {
            Value::Num(n) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "number")?;
                map.serialize_entry("value", &n.into_inner())?;
                map.end()
            }
            Value::StrictNum(t, v) => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("type", "strict_number")?;
                map.serialize_entry("num_type", &t.to_string())?;
                map.serialize_entry("value", v)?;
                map.end()
            }
            Value::Str(s) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "string")?;
                map.serialize_entry("value", s)?;
                map.end()
            }
            Value::Bool(b) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "bool")?;
                map.serialize_entry("value", b)?;
                map.end()
            }
            Value::Nil => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("type", "nil")?;
                map.end()
            }
            Value::Pair(a, b) => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("type", "pair")?;
                map.serialize_entry("car", &**a)?;
                map.serialize_entry("cdr", &**b)?;
                map.end()
            }
            Value::Builtin(name) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "builtin")?;
                map.serialize_entry("name", name)?;
                map.end()
            }
            Value::Lambda(_pat, _expr, _env, _res) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "lambda")?;
                map.serialize_entry("value", "<lambda: not serializable>")?;
                map.end()
            }
            Value::Environment(_env) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "environment")?;
                
                // Convert im::HashMap to a standard map for serialization
                let mut data_map = serde_json::Map::new();
                for (k, v) in _env.iter() {
                    let serialized = serde_json::to_value(v)
                        .map_err(|_| serde::ser::Error::custom("Failed to serialize value"))?;
                    data_map.insert(k.clone(), serialized);
                }
                
                map.serialize_entry("data", &data_map)?;
                map.end()
            }
            Value::Type(pt) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "pattern_type")?;
                map.serialize_entry("value", &pt.to_string())?;
                map.end()
            }
            Value::Frozen(_expr) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "frozen")?;
                map.serialize_entry("value", "<frozen: not serializable>")?;
                map.end()
            }
            Value::Pattern(_pat) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "pattern")?;
                map.serialize_entry("value", "<pattern: not serializable>")?;
                map.end()
            }
            Value::Native(idx) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "native")?;
                map.serialize_entry("index", idx)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for NumericValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Type,
            Value,
        }

        struct NumericValueVisitor;

        impl<'de> Visitor<'de> for NumericValueVisitor {
            type Value = NumericValue;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a numeric value object")
            }

            fn visit_map<V>(self, mut map: V) -> Result<NumericValue, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut value_type: Option<String> = None;
                let mut value: Option<String> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Type => {
                            if value_type.is_some() {
                                return Err(de::Error::duplicate_field("type"));
                            }
                            value_type = Some(map.next_value()?);
                        }
                        Field::Value => {
                            if value.is_some() {
                                return Err(de::Error::duplicate_field("value"));
                            }
                            value = Some(map.next_value()?);
                        }
                    }
                }

                let value_type = value_type.ok_or_else(|| de::Error::missing_field("type"))?;
                let value_str = value.ok_or_else(|| de::Error::missing_field("value"))?;

                match value_type.as_str() {
                    "float" => {
                        let f: f64 = value_str
                            .parse()
                            .map_err(|_| de::Error::custom("invalid float"))?;
                        Ok(NumericValue::Float(NotNan::new(f).map_err(|_| de::Error::custom("NaN not allowed"))?))
                    }
                    "int" => {
                        let i: BigInt = value_str
                            .parse()
                            .map_err(|_| de::Error::custom("invalid int"))?;
                        Ok(NumericValue::Int(i))
                    }
                    "uint" => {
                        let u: BigUint = value_str
                            .parse()
                            .map_err(|_| de::Error::custom("invalid uint"))?;
                        Ok(NumericValue::Uint(u))
                    }
                    _ => Err(de::Error::custom("unknown numeric type")),
                }
            }
        }

        deserializer.deserialize_map(NumericValueVisitor)
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            #[serde(rename = "type")]
            Type,
            Value,
            Data,
            #[serde(rename = "num_type")]
            NumType,
            Car,
            Cdr,
            Name,
            Index,
        }

        struct ValueVisitor;

        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a value object")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut value_type: Option<String> = None;
                let mut value: Option<serde_json::Value> = None;
                let mut data: Option<serde_json::Value> = None;
                let mut num_type: Option<String> = None;
                let mut car: Option<Value> = None;
                let mut cdr: Option<Value> = None;
                let mut name: Option<String> = None;
                let mut index: Option<usize> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Type => {
                            if value_type.is_some() {
                                return Err(de::Error::duplicate_field("type"));
                            }
                            value_type = Some(map.next_value()?);
                        }
                        Field::Value => {
                            if value.is_some() {
                                return Err(de::Error::duplicate_field("value"));
                            }
                            value = Some(map.next_value()?);
                        }
                        Field::Data => {
                            if data.is_some() {
                                return Err(de::Error::duplicate_field("data"));
                            }
                            data = Some(map.next_value()?);
                        }
                        Field::NumType => {
                            if num_type.is_some() {
                                return Err(de::Error::duplicate_field("num_type"));
                            }
                            num_type = Some(map.next_value()?);
                        }
                        Field::Car => {
                            if car.is_some() {
                                return Err(de::Error::duplicate_field("car"));
                            }
                            car = Some(map.next_value()?);
                        }
                        Field::Cdr => {
                            if cdr.is_some() {
                                return Err(de::Error::duplicate_field("cdr"));
                            }
                            cdr = Some(map.next_value()?);
                        }
                        Field::Name => {
                            if name.is_some() {
                                return Err(de::Error::duplicate_field("name"));
                            }
                            name = Some(map.next_value()?);
                        }
                        Field::Index => {
                            if index.is_some() {
                                return Err(de::Error::duplicate_field("index"));
                            }
                            index = Some(map.next_value()?);
                        }
                    }
                }

                let value_type = value_type.ok_or_else(|| de::Error::missing_field("type"))?;

                match value_type.as_str() {
                    "number" => {
                        let f: f64 = value
                            .ok_or_else(|| de::Error::missing_field("value"))?
                            .as_f64()
                            .ok_or_else(|| de::Error::custom("expected float"))?;
                        Ok(Value::Num(NotNan::new(f).map_err(|_| de::Error::custom("NaN not allowed"))?))
                    }
                    "strict_number" => {
                        let num_type_str = num_type.ok_or_else(|| de::Error::missing_field("num_type"))?;
                        let numeric_json = value.ok_or_else(|| de::Error::missing_field("value"))?;
                        
                        let numeric_value: NumericValue = serde_json::from_value(numeric_json)
                            .map_err(|e| de::Error::custom(e.to_string()))?;
                        
                        let num_type = match num_type_str.as_str() {
                            "i32" => NumericType::I32,
                            "i64" => NumericType::I64,
                            "u32" => NumericType::U32,
                            "u64" => NumericType::U64,
                            "f32" => NumericType::F32,
                            "f64" => NumericType::F64,
                            _ => return Err(de::Error::custom("unknown numeric type")),
                        };
                        Ok(Value::StrictNum(num_type, numeric_value))
                    }
                    "string" => {
                        let s = value
                            .ok_or_else(|| de::Error::missing_field("value"))?
                            .as_str()
                            .ok_or_else(|| de::Error::custom("expected string"))?
                            .to_string();
                        Ok(Value::Str(s))
                    }
                    "bool" => {
                        let b = value
                            .ok_or_else(|| de::Error::missing_field("value"))?
                            .as_bool()
                            .ok_or_else(|| de::Error::custom("expected bool"))?;
                        Ok(Value::Bool(b))
                    }
                    "nil" => Ok(Value::Nil),
                    "pair" => {
                        let car_val = car.ok_or_else(|| de::Error::missing_field("car"))?;
                        let cdr_val = cdr.ok_or_else(|| de::Error::missing_field("cdr"))?;
                        Ok(Value::Pair(Box::new(car_val), Box::new(cdr_val)))
                    }
                    "builtin" => {
                        let builtin_name = name.ok_or_else(|| de::Error::missing_field("name"))?;
                        Ok(Value::Builtin(builtin_name))
                    }
                    "environment" => {
                        let env_data = data.ok_or_else(|| de::Error::missing_field("data"))?;
                        let env_map = env_data.as_object()
                            .ok_or_else(|| de::Error::custom("expected object for environment"))?;
                        
                        let mut env = im::HashMap::new();
                        for (k, v) in env_map.iter() {
                            let value: Value = serde_json::from_value(v.clone())
                                .map_err(|e| de::Error::custom(e.to_string()))?;
                            env.insert(k.clone(), value);
                        }
                        Ok(Value::Environment(env))
                    }
                    "pattern_type" => {
                        let type_str = value
                            .ok_or_else(|| de::Error::missing_field("value"))?
                            .as_str()
                            .ok_or_else(|| de::Error::custom("expected string"))?
                            .to_string();
                        
                        let pt = match type_str.as_str() {
                            "any" => PatternType::Any,
                            "nil" => PatternType::Nil,
                            "pattern" => PatternType::Pattern,
                            "number" => PatternType::Number,
                            "string" => PatternType::String,
                            "lambda" | "fn" => PatternType::Lambda,
                            "bool" => PatternType::Bool,
                            "environment" | "env" => PatternType::Environment,
                            "frozen" => PatternType::Frozen,
                            _ => return Err(de::Error::custom("unknown pattern type")),
                        };
                        Ok(Value::Type(pt))
                    }
                    "native" => {
                        let idx = index.ok_or_else(|| de::Error::missing_field("index"))?;
                        Ok(Value::Native(idx))
                    }
                    _ => Err(de::Error::custom(format!("unknown value type: {}", value_type))),
                }
            }
        }

        deserializer.deserialize_map(ValueVisitor)
    }
}
