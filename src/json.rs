#![allow(unused_imports)]

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
    Object(Vec<(String, Value)>),
    Array(Vec<Value>),
}

impl Value {
    pub fn to_json(&self) -> String {
        match self {
            Value::String(s) => format!("\"{}\"", s.escape_default()),
            Value::Number(n) => if n.fract() == 0.0 { format!("{}", *n as i64) } else { format!("{}", n) },
            Value::Boolean(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::Object(pairs) => { let items: Vec<String> = pairs.iter().map(|(k, v)| format!("\"{}\":{}", k, v.to_json())).collect(); format!("{{{}}}", items.join(",")) }
            Value::Array(items) => { let vals: Vec<String> = items.iter().map(|v| v.to_json()).collect(); format!("[{}]", vals.join(",")) }
        }
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self { Value::Object(pairs) => { for (k, v) in pairs { if k == key { return Some(v); } } None } _ => None }
    }
    pub fn get_str(&self, key: &str) -> Option<&str> {
        match self.get(key) { Some(Value::String(s)) => Some(s), _ => None }
    }
    pub fn set(&mut self, key: &str, value: &Value) {
        match self { Value::Object(pairs) => { for (k, _) in pairs.iter_mut() { if k == key { return; } } pairs.push((key.to_string(), value.clone())); } _ => {} }
    }
    pub fn remove(&mut self, key: &str) {
        if let Value::Object(pairs) = self { pairs.retain(|(k, _)| k != key); }
    }
}

pub fn parse(s: &str) -> Option<Value> {
    let s = s.trim(); if s.is_empty() { return None; } let mut pos = 0; parse_value(s, &mut pos)
}

fn parse_value(s: &str, pos: &mut usize) -> Option<Value> {
    skip_ws(s, pos); if *pos >= s.len() { return None; }
    match s.as_bytes()[*pos] {
        b'"' => parse_string(s, pos).map(Value::String),
        b'{' => parse_object(s, pos),
        b'[' => parse_array(s, pos),
        b't' | b'f' => parse_bool(s, pos),
        b'n' => parse_null(s, pos),
        b'-' | b'0'..=b'9' => parse_number(s, pos).map(Value::Number),
        _ => None,
    }
}

fn skip_ws(s: &str, pos: &mut usize) { while *pos < s.len() { let c = s.as_bytes()[*pos]; if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' { *pos += 1; } else { break; } } }

fn parse_string(s: &str, pos: &mut usize) -> Option<String> {
    *pos += 1; let mut r = String::new(); let b = s.as_bytes();
    while *pos < b.len() { match b[*pos] {
        b'"' => { *pos += 1; return Some(r); }
        b'\\' => { *pos += 1; if *pos < b.len() { match b[*pos] { b'n' => r.push('\n'), b't' => r.push('\t'), b'r' => r.push('\r'), b'"' => r.push('"'), b'\\' => r.push('\\'), _ => r.push(b[*pos] as char) } *pos += 1; } }
        _ => { r.push(b[*pos] as char); *pos += 1; }
    }}
    Some(r)
}

fn parse_object(s: &str, pos: &mut usize) -> Option<Value> {
    *pos += 1; let mut pairs = Vec::new(); skip_ws(s, pos);
    if *pos < s.len() && s.as_bytes()[*pos] == b'}' { *pos += 1; return Some(Value::Object(pairs)); }
    loop { skip_ws(s, pos); let key = parse_string(s, pos)?; skip_ws(s, pos); if *pos >= s.len() || s.as_bytes()[*pos] != b':' { return None; } *pos += 1; skip_ws(s, pos); let val = parse_value(s, pos)?; pairs.push((key, val)); skip_ws(s, pos); if *pos >= s.len() { return None; } match s.as_bytes()[*pos] { b',' => { *pos += 1; continue; } b'}' => { *pos += 1; return Some(Value::Object(pairs)); } _ => return None } }
}

fn parse_array(s: &str, pos: &mut usize) -> Option<Value> {
    *pos += 1; let mut items = Vec::new(); skip_ws(s, pos);
    if *pos < s.len() && s.as_bytes()[*pos] == b']' { *pos += 1; return Some(Value::Array(items)); }
    loop { skip_ws(s, pos); if let Some(val) = parse_value(s, pos) { items.push(val); } skip_ws(s, pos); if *pos >= s.len() { return None; } match s.as_bytes()[*pos] { b',' => { *pos += 1; continue; } b']' => { *pos += 1; return Some(Value::Array(items)); } _ => return None } }
}

fn parse_number(s: &str, pos: &mut usize) -> Option<f64> {
    let start = *pos; if *pos < s.len() && s.as_bytes()[*pos] == b'-' { *pos += 1; }
    while *pos < s.len() && s.as_bytes()[*pos].is_ascii_digit() { *pos += 1; }
    if *pos < s.len() && s.as_bytes()[*pos] == b'.' { *pos += 1; while *pos < s.len() && s.as_bytes()[*pos].is_ascii_digit() { *pos += 1; } }
    s[start..*pos].parse().ok()
}

fn parse_bool(s: &str, pos: &mut usize) -> Option<Value> {
    if s[*pos..].starts_with("true") { *pos += 4; Some(Value::Boolean(true)) } else if s[*pos..].starts_with("false") { *pos += 5; Some(Value::Boolean(false)) } else { None }
}

fn parse_null(s: &str, pos: &mut usize) -> Option<Value> {
    if s[*pos..].starts_with("null") { *pos += 4; Some(Value::Null) } else { None }
}
