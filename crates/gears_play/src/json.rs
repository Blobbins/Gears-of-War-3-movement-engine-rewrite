//! A small JSON reader (enough for glTF and our own JSON files). No dependencies.

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

impl Json {
    pub fn parse(s: &str) -> Result<Json, String> {
        let mut p = Parser {
            b: s.as_bytes(),
            i: 0,
        };
        let v = p.value()?;
        p.ws();
        if p.i != p.b.len() {
            return Err(format!("trailing data at {}", p.i));
        }
        Ok(v)
    }
    pub fn get(&self, k: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.get(k),
            _ => None,
        }
    }
    pub fn idx(&self, i: usize) -> Option<&Json> {
        match self {
            Json::Arr(a) => a.get(i),
            _ => None,
        }
    }
    pub fn arr(&self) -> &[Json] {
        match self {
            Json::Arr(a) => a,
            _ => &[],
        }
    }
    pub fn obj(&self) -> Option<&BTreeMap<String, Json>> {
        match self {
            Json::Obj(m) => Some(m),
            _ => None,
        }
    }
    pub fn num(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }
    pub fn f32(&self) -> f32 {
        self.num().unwrap_or(0.0) as f32
    }
    pub fn usize(&self) -> Option<usize> {
        self.num().map(|n| n as usize)
    }
    pub fn str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        let c = *self.b.get(self.i).ok_or("unexpected end")?;
        match c {
            b'{' => {
                self.i += 1;
                let mut m = BTreeMap::new();
                self.ws();
                if self.b.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Json::Obj(m));
                }
                loop {
                    self.ws();
                    let k = match self.value()? {
                        Json::Str(s) => s,
                        _ => return Err(format!("object key expected at {}", self.i)),
                    };
                    self.ws();
                    if self.b.get(self.i) != Some(&b':') {
                        return Err(format!(": expected at {}", self.i));
                    }
                    self.i += 1;
                    let v = self.value()?;
                    m.insert(k, v);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Json::Obj(m));
                        }
                        _ => return Err(format!(", or }} expected at {}", self.i)),
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut a = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Json::Arr(a));
                }
                loop {
                    a.push(self.value()?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Json::Arr(a));
                        }
                        _ => return Err(format!(", or ] expected at {}", self.i)),
                    }
                }
            }
            b'"' => {
                self.i += 1;
                let mut s = String::new();
                loop {
                    let c = *self.b.get(self.i).ok_or("unterminated string")?;
                    self.i += 1;
                    match c {
                        b'"' => return Ok(Json::Str(s)),
                        b'\\' => {
                            let e = *self.b.get(self.i).ok_or("bad escape")?;
                            self.i += 1;
                            match e {
                                b'n' => s.push('\n'),
                                b't' => s.push('\t'),
                                b'r' => s.push('\r'),
                                b'b' => s.push('\u{8}'),
                                b'f' => s.push('\u{c}'),
                                b'u' => {
                                    let h = std::str::from_utf8(&self.b[self.i..self.i + 4])
                                        .map_err(|e| e.to_string())?;
                                    self.i += 4;
                                    let cp =
                                        u32::from_str_radix(h, 16).map_err(|e| e.to_string())?;
                                    s.push(char::from_u32(cp).unwrap_or('?'));
                                }
                                o => s.push(o as char),
                            }
                        }
                        _ => {
                            // copy a UTF-8 run
                            let start = self.i - 1;
                            let mut end = self.i;
                            while end < self.b.len() && self.b[end] != b'"' && self.b[end] != b'\\'
                            {
                                end += 1;
                            }
                            s.push_str(
                                std::str::from_utf8(&self.b[start..end])
                                    .map_err(|e| e.to_string())?,
                            );
                            self.i = end;
                        }
                    }
                }
            }
            b't' if self.b[self.i..].starts_with(b"true") => {
                self.i += 4;
                Ok(Json::Bool(true))
            }
            b'f' if self.b[self.i..].starts_with(b"false") => {
                self.i += 5;
                Ok(Json::Bool(false))
            }
            b'n' if self.b[self.i..].starts_with(b"null") => {
                self.i += 4;
                Ok(Json::Null)
            }
            _ => {
                let start = self.i;
                while self.i < self.b.len()
                    && matches!(
                        self.b[self.i],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.i += 1;
                }
                let t = std::str::from_utf8(&self.b[start..self.i]).map_err(|e| e.to_string())?;
                t.parse::<f64>()
                    .map(Json::Num)
                    .map_err(|_| format!("bad value at {start}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_nested() {
        let j = Json::parse(r#"{"a":[1,2.5,-3e2],"b":{"c":"x\"y"},"d":true,"e":null}"#).unwrap();
        assert_eq!(j.get("a").unwrap().idx(2).unwrap().num(), Some(-300.0));
        assert_eq!(j.get("b").unwrap().get("c").unwrap().str(), Some("x\"y"));
    }
}
