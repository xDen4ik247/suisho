//! Math input → syntax tree, with Desmos-like rules: variables are single letters (x, y, a_1, θ),
//! multi-letter names are only known ones (sin, ln, det, pi, theta…), so `xy` is x·y and `sinx` is sin(x);
//! `2x`, `x(x+1)`, `(a)(b)` multiply; `sin 2x` is sin(2x); `sin^2 x` is (sin x)²; `|x|` is abs.
//! `.` is always the decimal point and `,` always separates. Pasted LaTeX (\frac, \sqrt, \cdot, \left( …) works.

use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq)]
pub enum Ast {
    Num(String),
    Var(String),
    Neg(Box<Ast>),
    Add(Box<Ast>, Box<Ast>),
    Sub(Box<Ast>, Box<Ast>),
    Mul(Box<Ast>, Box<Ast>, bool), // implicit?
    Div(Box<Ast>, Box<Ast>),
    Pow(Box<Ast>, Box<Ast>),
    Fact(Box<Ast>),
    Deg(Box<Ast>),
    Call(String, Vec<Ast>),
    Abs(Box<Ast>),
    List(Vec<Ast>),
    Matrix(Vec<Vec<Ast>>),
    Tuple(Vec<Ast>),
    Rel(Box<Ast>, Vec<(Rel, Ast)>),
    Piecewise(Vec<(Ast, Ast)>, Option<Box<Ast>>),
    Restrict(Box<Ast>, Vec<Ast>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rel {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Rel {
    pub fn sym(self) -> &'static str {
        match self {
            Rel::Eq => "=",
            Rel::Ne => "≠",
            Rel::Lt => "<",
            Rel::Le => "≤",
            Rel::Gt => ">",
            Rel::Ge => "≥",
        }
    }
}

/// Built-in functions (applied with or without parentheses).
pub const FUNCS: &[&str] = &[
    "arcsinh", "arccosh", "arctanh", "arcsin", "arccos", "arctan", "arctg", "arccot", "arcctg", "arcsec", "arccsc",
    "sinh", "cosh", "tanh", "coth", "sin", "cos", "tan", "tg", "cot", "ctg", "sec", "csc", "cosec", "ln", "lg", "log",
    "exp", "sqrt", "cbrt", "abs", "floor", "ceil", "round", "sign", "sgn", "det", "tr", "trace", "rank", "inv", "rref",
    "transpose", "min", "max", "gcd", "lcm", "mod", "nCr", "nPr", "binom", "fact", "factorial", "mean", "median",
    "total", "sum", "length", "real", "imag", "conj", "arg", "re", "im", "frac", "eye", "ident", "root", "cross", "dot",
    "norm", "deg", "rad", "expand", "factor", "solve", "eigenvalues", "eig", "diff", "derivative", "integrate", "int", "lim", "limit", "tangent", "analyze", "investigate",
];
/// Named constants and Greek letters (single variables).
pub const NAMES: &[&str] = &[
    "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa", "lambda", "mu", "nu", "xi",
    "pi", "rho", "sigma", "tau", "upsilon", "phi", "chi", "psi", "omega", "infinity", "inf",
];

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(String),
    Ident(String),
    Func(String),
    Op(char),
    Rel(Rel),
    Open(char),
    Close(char),
    Comma,
    Semi,
    Colon,
    Bar,
    Bang,
    Degree,
    Sqrt(u32),
}

fn greek(c: char) -> Option<&'static str> {
    Some(match c {
        'α' => "alpha",
        'β' => "beta",
        'γ' => "gamma",
        'δ' => "delta",
        'ε' => "epsilon",
        'θ' => "theta",
        'λ' => "lambda",
        'μ' => "mu",
        'π' => "pi",
        'ρ' => "rho",
        'σ' => "sigma",
        'τ' => "tau",
        'φ' | 'ϕ' => "phi",
        'ψ' => "psi",
        'ω' => "omega",
        'ξ' => "xi",
        'η' => "eta",
        'ζ' => "zeta",
        'χ' => "chi",
        _ => return None,
    })
}

/// Rewrites the common LaTeX commands into plain input.
fn delatex(s: &str) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let mut t = s.to_string();
    for (a, b) in [
        ("\\left", ""),
        ("\\right", ""),
        ("\\cdot", "*"),
        ("\\times", "*"),
        ("\\div", "/"),
        ("\\le", "≤"),
        ("\\ge", "≥"),
        ("\\ne", "≠"),
        ("\\pi", "π"),
        ("\\theta", "θ"),
        ("\\infty", "∞"),
        ("\\,", ""),
        ("\\ ", ""),
        ("\\operatorname", ""),
    ] {
        t = t.replace(a, b);
    }
    // \frac{a}{b} → ((a)/(b)), \sqrt{a} → sqrt(a), \sqrt[n]{a} → root(a,n)
    loop {
        if let Some(i) = t.find("\\frac") {
            let rest = &t[i + 5..];
            if let Some((a, r1)) = brace_group(rest) {
                if let Some((b, r2)) = brace_group(r1) {
                    t = format!("{}(({})/({})){}", &t[..i], a, b, r2);
                    continue;
                }
            }
            t.replace_range(i..i + 5, "");
            continue;
        }
        if let Some(i) = t.find("\\sqrt") {
            let rest = &t[i + 5..];
            if let Some(stripped) = rest.strip_prefix('[') {
                if let Some(j) = stripped.find(']') {
                    let n = stripped[..j].to_string();
                    if let Some((a, r)) = brace_group(&stripped[j + 1..]) {
                        t = format!("{}root({},{}){}", &t[..i], a, n, r);
                        continue;
                    }
                }
            }
            if let Some((a, r)) = brace_group(rest) {
                t = format!("{}sqrt({}){}", &t[..i], a, r);
                continue;
            }
            t.replace_range(i..i + 5, "√");
            continue;
        }
        break;
    }
    t.replace('\\', "").replace('{', "(").replace('}', ")")
}

fn brace_group(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    let mut it = s.char_indices();
    if it.next()?.1 != '{' {
        return None;
    }
    let mut depth = 1;
    for (i, c) in it {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((s[1..i].to_string(), &s[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let src = delatex(src);
    let cs: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && cs.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let st = i;
            while i < cs.len() && cs[i].is_ascii_digit() {
                i += 1;
            }
            if i < cs.len() && cs[i] == '.' && cs.get(i + 1).is_some_and(|d| d.is_ascii_digit()) {
                i += 1;
                while i < cs.len() && cs[i].is_ascii_digit() {
                    i += 1;
                }
            } else if i < cs.len() && cs[i] == '.' && !cs.get(i + 1).is_some_and(|d| *d == '.') {
                i += 1; // "3." is 3
            }
            out.push(Tok::Num(cs[st..i].iter().collect::<String>().trim_end_matches('.').to_string()));
            continue;
        }
        if c.is_alphabetic() && c.is_ascii() {
            // longest known name at this position, else a single letter (+ subscript)
            let rest: String = cs[i..].iter().take_while(|c| c.is_ascii_alphabetic()).collect();
            let mut best: Option<(&str, bool)> = None;
            for f in FUNCS {
                if rest.starts_with(f) && best.is_none_or(|(b, _)| f.len() > b.len()) {
                    best = Some((f, true));
                }
            }
            for n in NAMES {
                if rest.starts_with(n) && best.is_none_or(|(b, _)| n.len() > b.len()) {
                    best = Some((n, false));
                }
            }
            if let Some((name, is_fn)) = best.filter(|(n, _)| n.len() > 1) {
                i += name.len();
                if name == "log" && i < cs.len() && cs[i] == '_' {
                    // log_2 x, log_(10) x, log_b x
                    i += 1;
                    let st = i;
                    if i < cs.len() && cs[i] == '(' {
                        while i < cs.len() && cs[i] != ')' {
                            i += 1;
                        }
                        let b: String = cs[st + 1..i.min(cs.len())].iter().collect();
                        i += 1;
                        out.push(Tok::Func(format!("log_{b}")));
                    } else {
                        while i < cs.len() && (cs[i].is_ascii_alphanumeric() || cs[i] == '.') {
                            i += 1;
                        }
                        out.push(Tok::Func(format!("log_{}", cs[st..i].iter().collect::<String>())));
                    }
                    continue;
                }
                let name = match name {
                    "inf" | "infinity" => "∞",
                    n => n,
                };
                out.push(if is_fn { Tok::Func(name.to_string()) } else { Tok::Ident(name.to_string()) });
                continue;
            }
            i += 1;
            let mut name = c.to_string();
            if i < cs.len() && cs[i] == '_' {
                i += 1;
                if i < cs.len() && cs[i] == '(' {
                    let st = i + 1;
                    while i < cs.len() && cs[i] != ')' {
                        i += 1;
                    }
                    name.push('_');
                    name.extend(&cs[st..i.min(cs.len())]);
                    i += 1;
                } else {
                    let st = i;
                    while i < cs.len() && cs[i].is_ascii_alphanumeric() {
                        i += 1;
                    }
                    name.push('_');
                    name.extend(&cs[st..i]);
                }
            }
            out.push(Tok::Ident(name));
            continue;
        }
        if let Some(g) = greek(c) {
            out.push(Tok::Ident(g.into()));
            i += 1;
            continue;
        }
        let two: String = cs[i..(i + 2).min(cs.len())].iter().collect();
        let t = match two.as_str() {
            "<=" => Some(Tok::Rel(Rel::Le)),
            ">=" => Some(Tok::Rel(Rel::Ge)),
            "!=" => Some(Tok::Rel(Rel::Ne)),
            "**" => Some(Tok::Op('^')),
            _ => None,
        };
        if let Some(t) = t {
            out.push(t);
            i += 2;
            continue;
        }
        let t = match c {
            '+' => Tok::Op('+'),
            '-' | '−' | '–' => Tok::Op('-'),
            '*' | '·' | '×' | '⋅' => Tok::Op('*'),
            '/' | '÷' => Tok::Op('/'),
            '^' => Tok::Op('^'),
            '²' => {
                out.push(Tok::Op('^'));
                Tok::Num("2".into())
            }
            '³' => {
                out.push(Tok::Op('^'));
                Tok::Num("3".into())
            }
            '=' => Tok::Rel(Rel::Eq),
            '<' => Tok::Rel(Rel::Lt),
            '>' => Tok::Rel(Rel::Gt),
            '≤' | '⩽' => Tok::Rel(Rel::Le),
            '≥' | '⩾' => Tok::Rel(Rel::Ge),
            '≠' => Tok::Rel(Rel::Ne),
            '(' | '[' | '{' => Tok::Open(c),
            ')' | ']' | '}' => Tok::Close(c),
            ',' => Tok::Comma,
            ';' => Tok::Semi,
            ':' => Tok::Colon,
            '|' => Tok::Bar,
            '!' => Tok::Bang,
            '°' => Tok::Degree,
            '√' => Tok::Sqrt(2),
            '∛' => Tok::Sqrt(3),
            '∜' => Tok::Sqrt(4),
            '∞' => Tok::Ident("∞".into()),
            _ => return Err(format!("unexpected character '{c}'")),
        };
        out.push(t);
        i += 1;
    }
    Ok(out)
}

pub struct Parser<'a> {
    toks: Vec<Tok>,
    pos: usize,
    funcs: &'a HashSet<String>,
    bar_depth: usize,
}

pub fn parse(src: &str, user_funcs: &HashSet<String>) -> Result<Option<Ast>, String> {
    let toks = lex(src)?;
    if toks.is_empty() {
        return Ok(None);
    }
    let mut p = Parser { toks, pos: 0, funcs: user_funcs, bar_depth: 0 };
    let e = p.statement()?;
    if p.pos < p.toks.len() {
        return Err(match &p.toks[p.pos] {
            Tok::Close(c) => format!("unmatched '{c}'"),
            t => format!("unexpected {}", tok_desc(t)),
        });
    }
    Ok(Some(e))
}

fn tok_desc(t: &Tok) -> String {
    match t {
        Tok::Num(n) => n.clone(),
        Tok::Ident(n) | Tok::Func(n) => format!("'{n}'"),
        Tok::Op(c) | Tok::Open(c) | Tok::Close(c) => format!("'{c}'"),
        Tok::Rel(r) => format!("'{}'", r.sym()),
        Tok::Comma => "','".into(),
        Tok::Semi => "';'".into(),
        Tok::Colon => "':'".into(),
        Tok::Bar => "'|'".into(),
        Tok::Bang => "'!'".into(),
        Tok::Degree => "'°'".into(),
        Tok::Sqrt(_) => "'√'".into(),
    }
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }
    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect_close(&mut self, open: char) -> Result<(), String> {
        let want = match open {
            '(' => ')',
            '[' => ']',
            _ => '}',
        };
        match self.next() {
            Some(Tok::Close(c)) if c == want => Ok(()),
            Some(Tok::Close(c)) if open != '{' && (c == ')' || c == ']') => Ok(()), // forgive [x) mixups
            Some(t) => Err(format!("expected '{want}', found {}", tok_desc(&t))),
            None => Err(format!("missing '{want}'")),
        }
    }

    /// relation chain, then an optional {restriction}
    fn statement(&mut self) -> Result<Ast, String> {
        let mut e = self.relation()?;
        let mut conds = Vec::new();
        while self.peek() == Some(&Tok::Open('{')) {
            self.pos += 1;
            loop {
                conds.push(self.relation()?);
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
            self.expect_close('{')?;
        }
        if !conds.is_empty() {
            e = Ast::Restrict(Box::new(e), conds);
        }
        Ok(e)
    }

    fn relation(&mut self) -> Result<Ast, String> {
        let first = self.expr()?;
        let mut chain = Vec::new();
        while let Some(Tok::Rel(r)) = self.peek().cloned() {
            self.pos += 1;
            chain.push((r, self.expr()?));
        }
        Ok(if chain.is_empty() { first } else { Ast::Rel(Box::new(first), chain) })
    }

    fn expr(&mut self) -> Result<Ast, String> {
        let mut l = if self.eat(&Tok::Op('-')) {
            Ast::Neg(Box::new(self.term()?))
        } else {
            self.eat(&Tok::Op('+'));
            self.term()?
        };
        loop {
            match self.peek() {
                Some(Tok::Op('+')) => {
                    self.pos += 1;
                    l = Ast::Add(Box::new(l), Box::new(self.term()?));
                }
                Some(Tok::Op('-')) => {
                    self.pos += 1;
                    l = Ast::Sub(Box::new(l), Box::new(self.term()?));
                }
                _ => return Ok(l),
            }
        }
    }

    fn term(&mut self) -> Result<Ast, String> {
        let mut l = self.unary()?;
        loop {
            match self.peek() {
                Some(Tok::Op('*')) => {
                    self.pos += 1;
                    l = Ast::Mul(Box::new(l), Box::new(self.unary()?), false);
                }
                Some(Tok::Op('/')) => {
                    self.pos += 1;
                    l = Ast::Div(Box::new(l), Box::new(self.unary()?));
                }
                Some(t) if self.starts_factor(t) => {
                    l = Ast::Mul(Box::new(l), Box::new(self.power()?), true);
                }
                _ => return Ok(l),
            }
        }
    }

    /// Can this token begin an implicitly multiplied factor? (a bar only opens one when no |…| is open)
    fn starts_factor(&self, t: &Tok) -> bool {
        match t {
            Tok::Num(_) | Tok::Ident(_) | Tok::Func(_) | Tok::Sqrt(_) => true,
            Tok::Open(c) => *c != '{',
            Tok::Bar => self.bar_depth == 0,
            _ => false,
        }
    }

    fn unary(&mut self) -> Result<Ast, String> {
        if self.eat(&Tok::Op('-')) {
            return Ok(Ast::Neg(Box::new(self.unary()?)));
        }
        if self.eat(&Tok::Op('+')) {
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> Result<Ast, String> {
        let base = self.postfix()?;
        if self.eat(&Tok::Op('^')) {
            let exp = if self.eat(&Tok::Op('-')) {
                Ast::Neg(Box::new(self.power()?))
            } else {
                self.power()?
            };
            return Ok(Ast::Pow(Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn postfix(&mut self) -> Result<Ast, String> {
        let mut e = self.primary()?;
        loop {
            match self.peek() {
                Some(Tok::Bang) => {
                    self.pos += 1;
                    e = Ast::Fact(Box::new(e));
                }
                Some(Tok::Degree) => {
                    self.pos += 1;
                    e = Ast::Deg(Box::new(e));
                }
                _ => return Ok(e),
            }
        }
    }

    fn primary(&mut self) -> Result<Ast, String> {
        let t = self.next().ok_or("unexpected end")?;
        match t {
            Tok::Num(n) => Ok(Ast::Num(n)),
            Tok::Ident(n) => {
                // user function call f(x) / f(x,y)
                if self.funcs.contains(&n) && self.peek() == Some(&Tok::Open('(')) {
                    self.pos += 1;
                    let args = self.args(')')?;
                    return Ok(Ast::Call(n, args));
                }
                Ok(Ast::Var(n))
            }
            Tok::Func(f) => self.func(f),
            Tok::Sqrt(k) => {
                let arg = self.postfix()?;
                Ok(if k == 2 { Ast::Call("sqrt".into(), vec![arg]) } else { Ast::Call("root".into(), vec![arg, Ast::Num(k.to_string())]) })
            }
            Tok::Bar => {
                self.bar_depth += 1;
                let e = self.expr();
                self.bar_depth -= 1;
                let e = e?;
                if !self.eat(&Tok::Bar) {
                    return Err("missing closing '|'".into());
                }
                Ok(Ast::Abs(Box::new(e)))
            }
            Tok::Open('(') => {
                let saved = self.bar_depth;
                self.bar_depth = 0;
                let first = self.relation();
                self.bar_depth = saved;
                let first = first?;
                if self.eat(&Tok::Comma) {
                    let mut items = vec![first];
                    loop {
                        items.push(self.expr()?);
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                    self.expect_close('(')?;
                    return Ok(Ast::Tuple(items));
                }
                self.expect_close('(')?;
                Ok(first)
            }
            Tok::Open('[') => self.bracket(),
            Tok::Open('{') => self.piecewise(),
            t => Err(format!("unexpected {}", tok_desc(&t))),
        }
    }

    fn args(&mut self, close: char) -> Result<Vec<Ast>, String> {
        let saved = self.bar_depth;
        self.bar_depth = 0;
        let mut v = Vec::new();
        if self.peek() != Some(&Tok::Close(close)) {
            loop {
                match self.relation() {
                    Ok(e) => v.push(e),
                    Err(e) => {
                        self.bar_depth = saved;
                        return Err(e);
                    }
                }
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
        }
        self.bar_depth = saved;
        self.expect_close(if close == ')' { '(' } else { '[' })?;
        Ok(v)
    }

    /// sin x, sin(x), sin^2 x, sin^-1 x, log_2 x, ln(x)
    fn func(&mut self, f: String) -> Result<Ast, String> {
        let mut f = canon_func(&f).to_string();
        let mut base: Option<Ast> = None;
        let mut power: Option<Ast> = None;
        if let Some(b) = f.strip_prefix("log_") {
            base = Some(parse(b, self.funcs)?.ok_or("log needs a base")?);
            f = "log".into();
        }
        if self.peek() == Some(&Tok::Op('^')) {
            self.pos += 1;
            let neg = self.eat(&Tok::Op('-'));
            let p = self.primary()?;
            if neg && p == Ast::Num("1".into()) {
                f = inverse_trig(&f).ok_or_else(|| format!("{f}^-1 is not a function"))?.to_string();
            } else {
                power = Some(if neg { Ast::Neg(Box::new(p)) } else { p });
            }
        }
        let args = if self.peek() == Some(&Tok::Open('(')) {
            self.pos += 1;
            self.args(')')?
        } else {
            // juxtaposed argument: a product of factors without + - / (sin 2x = sin(2x), sin x^2 = sin(x²))
            let mut a = self.power()?;
            while let Some(t) = self.peek() {
                if matches!(t, Tok::Num(_) | Tok::Ident(_) | Tok::Open('(')) {
                    a = Ast::Mul(Box::new(a), Box::new(self.power()?), true);
                } else {
                    break;
                }
            }
            vec![a]
        };
        if let Some(b) = base.take() {
            let mut v = args;
            v.push(b);
            return Ok(Ast::Call(f, v));
        }
        let call = Ast::Call(f, args);
        Ok(match power {
            Some(p) => Ast::Pow(Box::new(call), Box::new(p)),
            None => call,
        })
    }

    /// [1,2,3] list, [[1,2],[3,4]] or [1,2;3,4] matrix
    fn bracket(&mut self) -> Result<Ast, String> {
        let saved = self.bar_depth;
        self.bar_depth = 0;
        let r = self.bracket_inner();
        self.bar_depth = saved;
        r
    }
    fn bracket_inner(&mut self) -> Result<Ast, String> {
        if self.eat(&Tok::Close(']')) {
            return Ok(Ast::List(vec![]));
        }
        let mut rows: Vec<Vec<Ast>> = vec![vec![]];
        loop {
            let e = self.expr()?;
            rows.last_mut().unwrap().push(e);
            match self.next() {
                Some(Tok::Comma) => {}
                Some(Tok::Semi) => rows.push(vec![]),
                Some(Tok::Close(']')) => break,
                Some(t) => return Err(format!("expected ',' or ']', found {}", tok_desc(&t))),
                None => return Err("missing ']'".into()),
            }
        }
        if rows.len() > 1 {
            let w = rows[0].len();
            if rows.iter().any(|r| r.len() != w) {
                return Err("matrix rows have different lengths".into());
            }
            return Ok(Ast::Matrix(rows));
        }
        let items = rows.pop().unwrap();
        // [[1,2],[3,4]]: a list of equal-length lists is a matrix
        if !items.is_empty() && items.iter().all(|i| matches!(i, Ast::List(_))) {
            let rs: Vec<Vec<Ast>> = items.into_iter().map(|i| if let Ast::List(v) = i { v } else { unreachable!() }).collect();
            let w = rs[0].len();
            if w > 0 && rs.iter().all(|r| r.len() == w) {
                return Ok(Ast::Matrix(rs));
            }
            return Ok(Ast::List(rs.into_iter().map(Ast::List).collect()));
        }
        Ok(Ast::List(items))
    }

    /// {x<0: -x, x>=0: x} or {x<0: -x, x}
    fn piecewise(&mut self) -> Result<Ast, String> {
        let mut cases = Vec::new();
        let mut other = None;
        loop {
            let c = self.relation()?;
            if self.eat(&Tok::Colon) {
                let v = self.expr()?;
                cases.push((c, v));
            } else if matches!(c, Ast::Rel(..)) {
                cases.push((c, Ast::Num("1".into())));
            } else {
                other = Some(Box::new(c));
            }
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect_close('{')?;
        Ok(Ast::Piecewise(cases, other))
    }
}

pub fn canon_func(f: &str) -> &str {
    match f {
        "tg" => "tan",
        "ctg" => "cot",
        "cosec" => "csc",
        "arctg" => "arctan",
        "arcctg" => "arccot",
        "sgn" => "sign",
        "trace" => "tr",
        "factorial" | "fact" => "fact",
        "binom" => "nCr",
        "re" => "real",
        "im" => "imag",
        "ident" => "eye",
        f => f,
    }
}

fn inverse_trig(f: &str) -> Option<&'static str> {
    Some(match f {
        "sin" => "arcsin",
        "cos" => "arccos",
        "tan" => "arctan",
        "cot" => "arccot",
        "sec" => "arcsec",
        "csc" => "arccsc",
        "sinh" => "arcsinh",
        "cosh" => "arccosh",
        "tanh" => "arctanh",
        _ => return None,
    })
}

/// Names a statement defines, read from its tokens before full parsing: "f(x, y) = …" → ("f", ["x","y"]).
pub fn definition_head(src: &str) -> Option<(String, Vec<String>)> {
    let toks = lex(src).ok()?;
    let eq = toks.iter().position(|t| *t == Tok::Rel(Rel::Eq))?;
    let head = &toks[..eq];
    match head {
        [Tok::Ident(n)] => Some((n.clone(), vec![])),
        [Tok::Ident(n), Tok::Open('('), rest @ .., Tok::Close(')')] => {
            let mut params = Vec::new();
            for (i, t) in rest.iter().enumerate() {
                match (i % 2, t) {
                    (0, Tok::Ident(p)) => params.push(p.clone()),
                    (1, Tok::Comma) => {}
                    _ => return None,
                }
            }
            (!params.is_empty()).then(|| (n.clone(), params))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Ast {
        parse(s, &HashSet::new()).unwrap().unwrap()
    }
    fn pf(s: &str, f: &[&str]) -> Ast {
        let set: HashSet<String> = f.iter().map(|s| s.to_string()).collect();
        parse(s, &set).unwrap().unwrap()
    }
    fn v(n: &str) -> Box<Ast> {
        Box::new(Ast::Var(n.into()))
    }
    fn n(s: &str) -> Box<Ast> {
        Box::new(Ast::Num(s.into()))
    }

    #[test]
    fn implicit_products() {
        assert_eq!(p("2x"), Ast::Mul(n("2"), v("x"), true));
        assert_eq!(p("xy"), Ast::Mul(v("x"), v("y"), true));
        assert_eq!(p("-x^2"), Ast::Neg(Box::new(Ast::Pow(v("x"), n("2")))));
        assert_eq!(p("2^3^2"), Ast::Pow(n("2"), Box::new(Ast::Pow(n("3"), n("2")))));
    }

    #[test]
    fn functions() {
        assert_eq!(p("sinx"), Ast::Call("sin".into(), vec![Ast::Var("x".into())]));
        assert_eq!(p("sin 2x"), Ast::Call("sin".into(), vec![Ast::Mul(n("2"), v("x"), true)]));
        assert_eq!(p("sin^2 x"), Ast::Pow(Box::new(Ast::Call("sin".into(), vec![Ast::Var("x".into())])), n("2")));
        assert_eq!(p("sin^-1(x)"), Ast::Call("arcsin".into(), vec![Ast::Var("x".into())]));
        assert_eq!(p("tg x"), Ast::Call("tan".into(), vec![Ast::Var("x".into())]));
        assert_eq!(pf("f(2)", &["f"]), Ast::Call("f".into(), vec![Ast::Num("2".into())]));
        assert_eq!(p("a(2)"), Ast::Mul(v("a"), n("2"), true));
        assert!(matches!(p("|x-1|+|x|"), Ast::Add(..)));
        assert!(matches!(p("2|x|"), Ast::Mul(..)));
    }

    #[test]
    fn structures() {
        assert!(matches!(p("[[1,2],[3,4]]"), Ast::Matrix(ref r) if r.len() == 2));
        assert!(matches!(p("[1,2;3,4]"), Ast::Matrix(ref r) if r.len() == 2));
        assert!(matches!(p("[1,2,3]"), Ast::List(ref r) if r.len() == 3));
        assert!(matches!(p("(1,2)"), Ast::Tuple(_)));
        assert!(matches!(p("y=x^2 {x>0}"), Ast::Restrict(..)));
        assert!(matches!(p("1<x<=3"), Ast::Rel(_, ref c) if c.len() == 2));
        assert!(matches!(p("{x<0: -x, x}"), Ast::Piecewise(ref c, Some(_)) if c.len() == 1));
        assert!(matches!(p("\\frac{1}{2}+\\sqrt{4}"), Ast::Add(..)));
        assert_eq!(definition_head("f(x,y)=x+y"), Some(("f".into(), vec!["x".into(), "y".into()])));
        assert_eq!(definition_head("a=3"), Some(("a".into(), vec![])));
    }
}
