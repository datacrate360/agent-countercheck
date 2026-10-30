//! A small JSON editor that keeps a settings file's key order and numbers as written, so adding
//! Countercheck's hook to someone's `.claude/settings.json` changes only what it must
//! (`serde_json`'s map sorts keys, and the hook-settings check relies on that, so it can't be
//! switched to insertion order). Input is a user's file: size and depth are capped, errors are
//! plain messages, nothing panics.

/// Settings files larger than this are refused (same cap as the hook-settings check).
pub const MAX_BYTES: usize = countercheck_core::hook_settings::MAX_BYTES;
const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Null,
    Bool(bool),
    /// The number exactly as written.
    Num(String),
    Str(String),
    Arr(Vec<Node>),
    Obj(Vec<(String, Node)>),
    /// A list or object read from the file, with its text: written back exactly as it was unless
    /// something inside changes (then it is opened into the node).
    Kept(String, Box<Node>),
}

impl Node {
    /// Replace a kept list or object by its node, so it can change (and is written out anew).
    fn open(&mut self) {
        if let Node::Kept(_, inner) = self {
            let inner = std::mem::replace(inner.as_mut(), Node::Null);
            *self = inner;
        }
    }

    /// True when any string inside contains `needle`.
    pub fn mentions(&self, needle: &str) -> bool {
        match self {
            Node::Kept(_, inner) => inner.mentions(needle),
            Node::Str(s) => s.contains(needle),
            Node::Arr(items) => items.iter().any(|n| n.mentions(needle)),
            Node::Obj(fields) => fields.iter().any(|(_, n)| n.mentions(needle)),
            _ => false,
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Node> {
        self.open();
        match self {
            Node::Obj(fields) => fields.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The value under `key`, added at the end as `make()` when missing.
    pub fn entry(&mut self, key: &str, make: impl FnOnce() -> Node) -> Result<&mut Node, String> {
        self.open();
        let Node::Obj(fields) = self else {
            return Err(format!(
                "\"{key}\" can't be added: its parent is not an object"
            ));
        };
        let i = match fields.iter().position(|(k, _)| k == key) {
            Some(i) => i,
            None => {
                fields.push((key.to_string(), make()));
                fields.len() - 1
            }
        };
        match fields.get_mut(i) {
            Some((_, v)) => Ok(v),
            None => Err("internal: entry vanished".to_string()),
        }
    }

    /// The list's items, opened for change; `None` when this isn't a list.
    pub fn items_mut(&mut self) -> Option<&mut Vec<Node>> {
        self.open();
        match self {
            Node::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn remove(&mut self, key: &str) {
        self.open();
        if let Node::Obj(fields) = self {
            fields.retain(|(k, _)| k != key);
        }
    }

    pub fn is_object(&self) -> bool {
        match self {
            Node::Kept(_, inner) => inner.is_object(),
            Node::Obj(_) => true,
            _ => false,
        }
    }

    pub fn is_empty_container(&self) -> bool {
        match self {
            Node::Kept(_, inner) => inner.is_empty_container(),
            Node::Arr(a) => a.is_empty(),
            Node::Obj(o) => o.is_empty(),
            _ => false,
        }
    }
}

/// A `serde_json` value (Countercheck's own entries) as a node.
pub fn from_value(v: &serde_json::Value) -> Node {
    match v {
        serde_json::Value::Null => Node::Null,
        serde_json::Value::Bool(b) => Node::Bool(*b),
        serde_json::Value::Number(n) => Node::Num(n.to_string()),
        serde_json::Value::String(s) => Node::Str(s.clone()),
        serde_json::Value::Array(a) => Node::Arr(a.iter().map(from_value).collect()),
        serde_json::Value::Object(o) => {
            Node::Obj(o.iter().map(|(k, v)| (k.clone(), from_value(v))).collect())
        }
    }
}

pub fn parse(text: &str) -> Result<Node, String> {
    if text.len() > MAX_BYTES {
        return Err(format!("larger than {} KiB", MAX_BYTES / 1024));
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != p.s.len() {
        return Err(p.err("unexpected text after the JSON value"));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn err(&self, what: &str) -> String {
        let line = self.s[..self.i.min(self.s.len())]
            .iter()
            .filter(|b| **b == b'\n')
            .count()
            + 1;
        format!("not valid JSON (line {line}): {what}")
    }
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }
    fn lit(&mut self, word: &str, node: Node) -> Result<Node, String> {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(node)
        } else {
            Err(self.err("unknown word"))
        }
    }
    fn value(&mut self, depth: usize) -> Result<Node, String> {
        if depth > MAX_DEPTH {
            return Err(self.err("nested too deeply"));
        }
        let start = self.i;
        let kept = |p: &Self, n: Node| {
            let raw = String::from_utf8_lossy(&p.s[start..p.i]).into_owned();
            Node::Kept(raw, Box::new(n))
        };
        match self.peek() {
            Some(b'{') => self.object(depth).map(|n| kept(self, n)),
            Some(b'[') => self.array(depth).map(|n| kept(self, n)),
            Some(b'"') => self.string().map(Node::Str),
            Some(b't') => self.lit("true", Node::Bool(true)),
            Some(b'f') => self.lit("false", Node::Bool(false)),
            Some(b'n') => self.lit("null", Node::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(b'/') => Err(self.err("comments are not allowed in this file")),
            _ => Err(self.err("expected a value")),
        }
    }
    fn number(&mut self) -> Result<Node, String> {
        let start = self.i;
        while matches!(
            self.peek(),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.i += 1;
        }
        let raw =
            std::str::from_utf8(&self.s[start..self.i]).map_err(|_| self.err("bad number"))?;
        if raw.parse::<f64>().is_err() {
            return Err(self.err("bad number"));
        }
        Ok(Node::Num(raw.to_string()))
    }
    fn string(&mut self) -> Result<String, String> {
        // From the opening quote to the closing one, then let serde_json decode the escapes.
        let start = self.i;
        self.i += 1;
        loop {
            match self.peek() {
                None => return Err(self.err("unterminated string")),
                Some(b'\\') => self.i += 2,
                Some(b'"') => {
                    self.i += 1;
                    break;
                }
                Some(_) => self.i += 1,
            }
        }
        let raw = self
            .s
            .get(start..self.i)
            .ok_or_else(|| self.err("unterminated string"))?;
        serde_json::from_slice::<String>(raw).map_err(|_| self.err("bad string"))
    }
    fn array(&mut self, depth: usize) -> Result<Node, String> {
        self.i += 1;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Node::Arr(items));
        }
        loop {
            self.ws();
            items.push(self.value(depth + 1)?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Node::Arr(items));
                }
                _ => return Err(self.err("expected , or ]")),
            }
        }
    }
    fn object(&mut self, depth: usize) -> Result<Node, String> {
        self.i += 1;
        let mut fields: Vec<(String, Node)> = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Node::Obj(fields));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("expected a key in quotes"));
            }
            let k = self.string()?;
            self.ws();
            if self.peek() != Some(b':') {
                return Err(self.err("expected :"));
            }
            self.i += 1;
            self.ws();
            let v = self.value(depth + 1)?;
            if fields.iter().any(|(old, _)| *old == k) {
                return Err(self.err(&format!("the key \"{k}\" appears twice")));
            }
            fields.push((k, v));
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Node::Obj(fields));
                }
                _ => return Err(self.err("expected , or }")),
            }
        }
    }
}

/// Two-space indented JSON, keys in their order, ending with a newline.
pub fn pretty(n: &Node) -> String {
    let mut out = String::new();
    write(n, 0, &mut out);
    out.push('\n');
    out
}

fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

fn write(n: &Node, indent: usize, out: &mut String) {
    let pad = |k: usize| "  ".repeat(k);
    match n {
        Node::Kept(raw, _) => out.push_str(raw),
        Node::Null => out.push_str("null"),
        Node::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Node::Num(s) => out.push_str(s),
        Node::Str(s) => out.push_str(&quote(s)),
        Node::Arr(items) if items.is_empty() => out.push_str("[]"),
        Node::Obj(fields) if fields.is_empty() => out.push_str("{}"),
        Node::Arr(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                out.push_str(&pad(indent + 1));
                write(item, indent + 1, out);
                out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
            }
            out.push_str(&pad(indent));
            out.push(']');
        }
        Node::Obj(fields) => {
            out.push_str("{\n");
            for (i, (k, v)) in fields.iter().enumerate() {
                out.push_str(&pad(indent + 1));
                out.push_str(&quote(k));
                out.push_str(": ");
                write(v, indent + 1, out);
                out.push_str(if i + 1 < fields.len() { ",\n" } else { "\n" });
            }
            out.push_str(&pad(indent));
            out.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_key_order_numbers_and_escapes() {
        let text = "{\"z\": 1.50, \"a\": {\"b\": [true, null, \"\\u00e9\\n\"]}, \"m\": -2e3}";
        let mut n = parse(text).unwrap();
        // Untouched, the file comes back as it was.
        assert_eq!(pretty(&n), format!("{text}\n"));
        n.open();
        if let Node::Obj(fields) = &mut n {
            for (_, v) in fields.iter_mut() {
                v.open();
                if let Some(b) = v.get_mut("b") {
                    b.open();
                }
            }
        }
        assert_eq!(
            pretty(&n),
            "{\n  \"z\": 1.50,\n  \"a\": {\n    \"b\": [\n      true,\n      null,\n      \"é\\n\"\n    ]\n  },\n  \"m\": -2e3\n}\n"
        );
        assert_eq!(pretty(&parse(&pretty(&n)).unwrap()), pretty(&n));
    }

    #[test]
    fn refuses_what_isnt_plain_json() {
        for bad in [
            "{\"a\": 1,}",
            "{\"a\": 1} x",
            "// c\n{}",
            "{\"a\": 1, \"a\": 2}",
            "{\"a\": \"x}",
            "[1 2]",
            "{a: 1}",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        let deep = "[".repeat(100) + &"]".repeat(100);
        assert!(parse(&deep).unwrap_err().contains("nested too deeply"));
        assert!(parse("\u{feff}{}").unwrap().is_empty_container());
    }

    #[test]
    fn entries_are_added_at_the_end_and_found_by_text() {
        let mut n = parse("{\"model\": \"x\"}").unwrap();
        n.entry("hooks", || Node::Obj(vec![]))
            .unwrap()
            .entry("Stop", || Node::Arr(vec![]))
            .unwrap();
        assert_eq!(
            pretty(&n),
            "{\n  \"model\": \"x\",\n  \"hooks\": {\n    \"Stop\": []\n  }\n}\n"
        );
        let mut s = Node::Str("x".into());
        assert!(s.entry("k", || Node::Null).is_err());
        assert!(
            parse("{\"c\": \"countercheck hook stop\"}")
                .unwrap()
                .mentions("hook stop")
        );
    }
}
