//! Bounded AMF0 command codec. Server-controlled strings are never used as diagnostics.
use std::io;

#[derive(Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Bool(bool),
    String(String),
    Object(Vec<(String, Value)>),
    Array(Vec<Value>),
    Null,
}

impl Value {
    pub fn text(&self) -> Option<&str> {
        if let Self::String(s) = self { Some(s) } else { None }
    }
    pub fn number(&self) -> Option<f64> {
        if let Self::Number(n) = self { Some(*n) } else { None }
    }
    pub fn field(&self, name: &str) -> Option<&Value> {
        if let Self::Object(fields) = self { fields.iter().find(|(k, _)| k == name).map(|(_, v)| v) } else { None }
    }
}

pub fn string(s: &str) -> Value {
    Value::String(s.into())
}
pub fn object(fields: &[(&str, Value)]) -> Value {
    Value::Object(fields.iter().map(|(k, v)| ((*k).into(), v.clone())).collect())
}
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid or unsupported AMF0 command")
}

pub fn encode(values: &[Value]) -> io::Result<Vec<u8>> {
    fn value(out: &mut Vec<u8>, v: &Value, depth: usize) -> io::Result<()> {
        if depth > 16 {
            return Err(invalid());
        }
        match v {
            Value::Number(n) => {
                out.push(0);
                out.extend(n.to_be_bytes());
            }
            Value::Bool(b) => out.extend([1, u8::from(*b)]),
            Value::Null => out.push(5),
            Value::String(s) => {
                if let Ok(n) = u16::try_from(s.len()) {
                    out.push(2);
                    out.extend(n.to_be_bytes());
                } else {
                    out.push(12);
                    out.extend(u32::try_from(s.len()).map_err(|_| invalid())?.to_be_bytes());
                }
                out.extend(s.as_bytes());
            }
            Value::Object(fields) => {
                out.push(3);
                for (name, v) in fields {
                    out.extend(u16::try_from(name.len()).map_err(|_| invalid())?.to_be_bytes());
                    out.extend(name.as_bytes());
                    value(out, v, depth + 1)?;
                }
                out.extend([0, 0, 9]);
            }
            Value::Array(values) => {
                out.push(10);
                out.extend(u32::try_from(values.len()).map_err(|_| invalid())?.to_be_bytes());
                for v in values {
                    value(out, v, depth + 1)?;
                }
            }
        }
        if out.len() > 1 << 20 {
            return Err(invalid());
        }
        Ok(())
    }
    let mut out = Vec::new();
    for v in values {
        value(&mut out, v, 0)?;
    }
    Ok(out)
}

pub fn decode(mut input: &[u8]) -> io::Result<Vec<Value>> {
    fn take<'a>(input: &mut &'a [u8], n: usize) -> io::Result<&'a [u8]> {
        if input.len() < n {
            return Err(invalid());
        }
        let (head, tail) = input.split_at(n);
        *input = tail;
        Ok(head)
    }
    fn number(input: &mut &[u8], n: usize) -> io::Result<usize> {
        Ok(take(input, n)?.iter().fold(0, |v, b| (v << 8) | *b as usize))
    }
    fn text(input: &mut &[u8], n: usize) -> io::Result<String> {
        String::from_utf8(take(input, n)?.to_vec()).map_err(|_| invalid())
    }
    fn value(input: &mut &[u8], depth: usize, budget: &mut usize) -> io::Result<Value> {
        if depth > 16 || *budget == 0 {
            return Err(invalid());
        }
        *budget -= 1;
        Ok(match take(input, 1)?[0] {
            0 => Value::Number(f64::from_be_bytes(take(input, 8)?.try_into().map_err(|_| invalid())?)),
            1 => Value::Bool(take(input, 1)?[0] != 0),
            2 => {
                let n = number(input, 2)?;
                Value::String(text(input, n)?)
            }
            5 | 6 => Value::Null,
            marker @ (3 | 8) => {
                if marker == 8 {
                    take(input, 4)?;
                }
                let mut fields = Vec::new();
                loop {
                    if input.starts_with(&[0, 0, 9]) {
                        take(input, 3)?;
                        break;
                    }
                    let n = number(input, 2)?;
                    let name = text(input, n)?;
                    fields.push((name, value(input, depth + 1, budget)?));
                }
                Value::Object(fields)
            }
            10 => {
                let n = number(input, 4)?;
                if n > *budget {
                    return Err(invalid());
                }
                let mut values = Vec::new();
                for _ in 0..n {
                    values.push(value(input, depth + 1, budget)?);
                }
                Value::Array(values)
            }
            12 => {
                let n = number(input, 4)?;
                Value::String(text(input, n)?)
            }
            _ => return Err(invalid()),
        })
    }
    if input.len() > 1 << 20 {
        return Err(invalid());
    }
    let mut values = Vec::new();
    let mut budget = 4096;
    while !input.is_empty() {
        values.push(value(&mut input, 0, &mut budget)?);
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_roundtrip_and_known_bytes() {
        let values = vec![
            string("connect"),
            Value::Number(1.),
            object(&[("app", string("live")), ("fpad", Value::Bool(false))]),
            Value::Null,
        ];
        let bytes = encode(&values).expect("encode");
        assert_eq!(&bytes[..10], b"\x02\0\x07connect");
        assert!(decode(&bytes).expect("decode") == values);
        for end in 1..bytes.len() {
            // Some prefixes end on complete values and are valid AMF sequences.
            if let Ok(v) = decode(&bytes[..end]) {
                assert!(!v.is_empty());
            }
        }
    }
    #[test]
    fn rejects_hostile_lengths_depth_and_utf8() {
        for bytes in [&[12, 255, 255, 255, 255][..], &[10, 255, 255, 255, 255], &[2, 0, 1, 255], &[3, 0, 0]] {
            assert!(decode(bytes).is_err());
        }
        let mut nested = [10, 0, 0, 0, 1].repeat(18);
        nested.push(5);
        assert!(decode(&nested).is_err());
    }
}
