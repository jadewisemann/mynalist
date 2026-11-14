//! The look's values: the base (`design/base.toml`), the chosen modes (theme, size, density, fonts; the device
//! from the platform) and the user's appearance, a TOML document in the same schema holding only what they change
//! (`design/schema.md`). All of it comes from the generated `crate::design`; `style.rs` reads the result.
//!
//! The same rules as the web's `apps/web/src/design/appearance.ts`: a value is checked against its entry (kind,
//! range, choices), a color or font may hold `{table.key}` references, resolved after all layers are applied, and an
//! invalid document is rejected whole, with the line of its first error, leaving the look as it was.

use std::collections::HashMap;
use std::rc::Rc;

use nsh_core::prelude::*;

use crate::design::{self, Kind, Value, BASE, GROUPS, SCHEMA};

/// A resolved value.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    /// 0xRRGGBBAA
    Color(u32),
    Number(f32),
    Text(String),
}

/// One mode per group (`design/modes.toml`); the device is not here: both are resolved, the platform picks.
#[derive(Clone, Debug, PartialEq)]
pub struct Modes {
    pub theme: String,
    pub size: String,
    pub density: String,
    pub font: String,
    pub eafont: String,
}

impl Default for Modes {
    fn default() -> Self {
        let d = |g: &str| GROUPS.iter().find(|x| x.name == g).map_or("", |x| x.default).to_owned();
        Modes { theme: d("theme"), size: d("size"), density: d("density"), font: d("font"), eafont: d("eafont") }
    }
}

impl Modes {
    /// Modes from `group=mode` words (`"theme=dark size=large"`); groups not named keep their default.
    pub fn parse(spec: &str) -> Result<Modes, String> {
        let mut m = Modes::default();
        for word in spec.split_whitespace() {
            let (group, mode) = word.split_once('=').ok_or_else(|| format!("not group=mode: {word}"))?;
            let known = GROUPS.iter().find(|g| g.name == group).ok_or_else(|| format!("no mode group {group}"))?;
            if mode != known.default && !known.modes.iter().any(|(n, _)| *n == mode) {
                return Err(format!("no mode {mode} in {group}"));
            }
            let slot = match group {
                "theme" => &mut m.theme,
                "size" => &mut m.size,
                "density" => &mut m.density,
                "font" => &mut m.font,
                "eafont" => &mut m.eafont,
                _ => return Err(format!("{group} is not chosen by the user")),
            };
            *slot = mode.to_owned();
        }
        Ok(m)
    }
}

/// A user's appearance, checked: what it sets, in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct User(Vec<(&'static str, Val)>);

/// An invalid appearance: what is wrong, and the 1-based line it is on (0 when the document as a whole is).
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

/// The resolved look of one device.
#[derive(Clone, Debug, PartialEq)]
pub struct Look(HashMap<&'static str, Val>);

impl Look {
    fn get(&self, id: &str) -> &Val {
        self.0.get(id).unwrap_or_else(|| panic!("no design entry {id}"))
    }
    /// A color entry, as 0xRRGGBBAA.
    pub fn color(&self, id: &str) -> u32 {
        match self.get(id) {
            Val::Color(c) => *c,
            v => panic!("{id} is not a color: {v:?}"),
        }
    }
    /// A length, ratio or weight entry.
    pub fn number(&self, id: &str) -> f32 {
        match self.get(id) {
            Val::Number(n) => *n,
            v => panic!("{id} is not a number: {v:?}"),
        }
    }
    /// A font or choice entry.
    pub fn text(&self, id: &str) -> &str {
        match self.get(id) {
            Val::Text(t) => t,
            v => panic!("{id} is not text: {v:?}"),
        }
    }
}

/// The looks of both devices for the current modes and user appearance.
#[derive(Clone, Debug, PartialEq)]
pub struct Looks {
    pub modes: Modes,
    pub user: User,
    pub desktop: Look,
    pub phone: Look,
}

impl Looks {
    pub fn new(modes: Modes, user: User) -> Looks {
        let desktop = resolve(&modes, "desktop", &user);
        let phone = resolve(&modes, "phone", &user);
        Looks { modes, user, desktop, phone }
    }
}

impl Default for Looks {
    fn default() -> Self {
        Looks::new(Modes::default(), User::default())
    }
}

fn val(v: &Value) -> Val {
    match v {
        Value::Color(c) => Val::Color(*c),
        Value::Number(n) => Val::Number(*n),
        Value::Text(t) => Val::Text((*t).to_owned()),
    }
}

fn layer(map: &mut HashMap<&'static str, Val>, group: &str, mode: &str) {
    let Some(g) = GROUPS.iter().find(|g| g.name == group) else { return };
    if let Some((_, values)) = g.modes.iter().find(|(name, _)| *name == mode) {
        for (id, v) in values.iter() {
            map.insert(id, val(v));
        }
    }
}

/// Base, then the modes, then the device, then the user; then `{table.key}` references.
fn resolve(modes: &Modes, device: &str, user: &User) -> Look {
    let mut map: HashMap<&'static str, Val> = BASE.iter().map(|(id, v)| (*id, val(v))).collect();
    layer(&mut map, "theme", &modes.theme);
    layer(&mut map, "size", &modes.size);
    layer(&mut map, "density", &modes.density);
    layer(&mut map, "font", &modes.font);
    layer(&mut map, "eafont", &modes.eafont);
    layer(&mut map, "device", device);
    for (id, v) in &user.0 {
        map.insert(id, v.clone());
    }
    // references point at entries that may hold references themselves: a few passes settle them (the files are
    // checked to have no cycles: see `check_refs`)
    for _ in 0..4 {
        let snapshot = map.clone();
        for v in map.values_mut() {
            if let Val::Text(t) = v {
                if !t.contains('{') {
                    continue;
                }
                let whole = t.strip_prefix('{').and_then(|r| r.strip_suffix('}')).filter(|r| !r.contains('{'));
                if let Some(target @ Val::Color(_)) = whole.and_then(|r| snapshot.get(r)) {
                    *v = target.clone();
                    continue;
                }
                *v = Val::Text(substitute(t, &snapshot));
            }
        }
    }
    Look(map)
}

/// `{table.key}` inside a text replaced by that entry's text.
fn substitute(t: &str, values: &HashMap<&'static str, Val>) -> String {
    let mut out = String::new();
    let mut rest = t;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}') else { break };
        let id = &rest[start + 1..start + end];
        match values.get(id) {
            Some(Val::Text(x)) => out.push_str(x),
            _ => out.push_str(&rest[start..=start + end]),
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}

/// A color from its TOML text: "#rgb", "#rrggbb", "#rrggbbaa", "rgba(r, g, b, a)", "rgb(r, g, b)", "transparent".
pub fn parse_color(s: &str) -> Option<u32> {
    let s = s.trim();
    if s == "transparent" {
        return Some(0);
    }
    if let Some(h) = s.strip_prefix('#') {
        if !h.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let full = match h.len() {
            3 => h.chars().flat_map(|c| [c, c]).collect::<String>() + "ff",
            6 => format!("{h}ff"),
            8 => h.to_owned(),
            _ => return None,
        };
        return u32::from_str_radix(&full, 16).ok();
    }
    let inner = s.strip_prefix("rgba(").or_else(|| s.strip_prefix("rgb("))?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if !(3..=4).contains(&parts.len()) {
        return None;
    }
    let mut c = 0u32;
    for p in &parts[..3] {
        let n: u32 = p.parse().ok().filter(|n| *n <= 255)?;
        c = (c << 8) | n;
    }
    let a = match parts.get(3) {
        Some(a) => a.parse::<f32>().ok().filter(|a| (0.0..=1.0).contains(a))?,
        None => 1.0,
    };
    Some((c << 8) | (a * 255.0).round() as u32)
}

fn entry(id: &str) -> Option<&'static design::Entry> {
    SCHEMA.iter().find(|e| e.id == id)
}

/// The `{table.key}` references in a text.
fn refs(t: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = t;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}') else { break };
        out.push(&rest[start + 1..start + end]);
        rest = &rest[start + end + 1..];
    }
    out
}

/// Checks one value against its entry, as the generator and the web do.
fn check(e: &design::Entry, v: &toml::Value) -> Result<Val, String> {
    let range = |n: f32| -> Result<Val, String> {
        if let Some(min) = e.min.filter(|m| n < *m) {
            return Err(format!("less than {min}"));
        }
        if let Some(max) = e.max.filter(|m| n > *m) {
            return Err(format!("more than {max}"));
        }
        Ok(Val::Number(n))
    };
    let number = v.as_float().or_else(|| v.as_integer().map(|i| i as f64)).map(|n| n as f32);
    if let Some(t) = v.as_str() {
        for r in refs(t) {
            if entry(r).is_none() {
                return Err(format!("unknown entry {{{r}}}"));
            }
        }
    }
    match e.kind {
        Kind::Color => {
            let t = v.as_str().ok_or("a color is a string")?;
            let rs = refs(t);
            if !rs.is_empty() {
                let one = rs.len() == 1 && t == format!("{{{}}}", rs[0]);
                return if one && entry(rs[0]).is_some_and(|x| x.kind == Kind::Color) {
                    Ok(Val::Text(t.to_owned()))
                } else {
                    Err("a color reference is one color entry".into())
                };
            }
            parse_color(t).map(Val::Color).ok_or_else(|| format!("not a color: {t}"))
        }
        Kind::Length => {
            if let Some(pt) = v.as_str().and_then(|t| t.strip_suffix("pt")).and_then(|n| n.parse::<f32>().ok()) {
                return Ok(Val::Number(pt * 4.0 / 3.0));
            }
            range(number.ok_or("a length is a number (pixels)")?)
        }
        Kind::Ratio => range(number.ok_or("a ratio is a number")?),
        Kind::Weight => match v.as_integer() {
            Some(w) if (100..=900).contains(&w) => Ok(Val::Number(w as f32)),
            _ => Err("a weight is a whole number from 100 to 900".into()),
        },
        Kind::Font => match v.as_str() {
            Some(t) if !t.trim().is_empty() => Ok(Val::Text(t.to_owned())),
            _ => Err("a font is a CSS font-family list".into()),
        },
        Kind::Choice => match v.as_str() {
            Some(t) if e.choices.contains(&t) => Ok(Val::Text(t.to_owned())),
            _ => Err(format!("one of {}", e.choices.join(", "))),
        },
    }
}

/// The 1-based line of `key` in `[table]` (or of the table's header, or 0).
fn line_of(text: &str, table: &str, key: Option<&str>) -> usize {
    let mut in_table = false;
    let mut header = 0;
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if let Some(name) = t.strip_prefix('[').and_then(|r| r.split(']').next()) {
            in_table = name.trim() == table;
            if in_table {
                header = i + 1;
            }
            continue;
        }
        if let (true, Some(key)) = (in_table, key) {
            let k = t.split('=').next().unwrap_or("").trim().trim_matches('"');
            if k == key {
                return i + 1;
            }
        }
    }
    header
}

/// Parses and checks a user's appearance TOML.
pub fn parse(text: &str) -> Result<User, Error> {
    let doc: toml::Table = text.parse().map_err(|e: toml::de::Error| Error {
        line: e.span().map_or(0, |s| text[..s.start.min(text.len())].matches('\n').count() + 1),
        message: e.message().to_owned(),
    })?;
    let mut out = Vec::new();
    for (table, entries) in &doc {
        let Some(entries) = entries.as_table() else {
            return Err(Error { line: line_of(text, table, None), message: format!("{table} is not a table") });
        };
        for (key, v) in entries {
            let id = format!("{table}.{key}");
            let line = line_of(text, table, Some(key));
            let e = entry(&id).ok_or_else(|| Error { line, message: format!("unknown entry {id}") })?;
            let v = check(e, v).map_err(|m| Error { line, message: format!("{id}: {m}") })?;
            out.push((e.id, v));
        }
    }
    Ok(User(out))
}

/// The looks this app shows, a reactive global: whatever reads it is drawn again when it changes.
#[derive(Clone, Default)]
struct Current(Rc<Looks>);

/// The current looks (tracked).
pub fn current() -> Rc<Looks> {
    global::<Current>().get().0
}

/// Applies the modes and a user's appearance TOML at once; an invalid TOML leaves the look as it was.
pub fn set(modes: Modes, user_toml: &str) -> Result<(), Error> {
    let user = parse(user_toml)?;
    global::<Current>().set(Current(Rc::new(Looks::new(modes, user))));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_resolves_every_entry() {
        let looks = Looks::default();
        for e in SCHEMA {
            let v = looks.desktop.get(e.id);
            if let Val::Text(t) = v {
                assert!(!t.contains('{'), "{} unresolved: {t}", e.id);
            }
        }
        assert_eq!(looks.desktop.color("color.topbar_border"), looks.desktop.color("color.line"));
        assert_eq!(looks.desktop.number("space.indent"), 38.0);
        assert_eq!(looks.phone.number("space.indent"), 29.0);
    }

    #[test]
    fn modes_and_user_layer_in_order() {
        let modes = Modes { theme: "dark".into(), size: "large".into(), ..Modes::default() };
        let user = parse("[color]\ntag = \"#008080\"\nline = \"rgba(0, 0, 0, .5)\"\n\n[type]\nsize = 17\n").unwrap();
        let looks = Looks::new(modes, user);
        assert_eq!(looks.desktop.color("color.bg"), 0x1b1c1eff);
        assert_eq!(looks.desktop.color("color.tag"), 0x008080ff);
        // a reference follows the entry it names, user layer included
        assert_eq!(looks.desktop.color("color.topbar_border"), 0x00000080);
        assert_eq!(looks.desktop.number("type.size"), 17.0);
        assert_eq!(looks.phone.number("type.size"), 17.0);
    }

    #[test]
    fn fonts_substitute_references() {
        let modes = Modes { eafont: "pretendard".into(), font: "serif".into(), ..Modes::default() };
        let looks = Looks::new(modes, User::default());
        assert_eq!(looks.desktop.text("font.editor"), "Georgia, 'Times New Roman', serif, Pretendard, sans-serif");
    }

    #[test]
    fn errors_have_their_line() {
        let err = |t: &str| parse(t).unwrap_err();
        assert_eq!(err("[color]\nbg = \"#fff\"\ntag = \"teal\"\n").line, 3);
        assert_eq!(err("[type]\n\nsize = 100\n").line, 3);
        assert_eq!(err("[type]\nsize = 100\n").message, "type.size: more than 40");
        assert_eq!(err("[color]\nnope = \"#fff\"\n").message, "unknown entry color.nope");
        assert_eq!(err("[color]\nbg = \n").line, 2);
        assert_eq!(err("[space]\npad = \"x\"\n").message, "space.pad: a length is a number (pixels)");
        assert_eq!(err("[color]\nbg = \"{space.pad}\"\n").message, "color.bg: a color reference is one color entry");
        assert_eq!(err("[type]\nheading_weight = 650.5\n").line, 2);
        assert_eq!(err("[color]\nscheme = \"blue\"\n").message, "color.scheme: one of light, dark");
    }

    #[test]
    fn colors_parse() {
        assert_eq!(parse_color("#abc"), Some(0xaabbccff));
        assert_eq!(parse_color("#008080"), Some(0x008080ff));
        assert_eq!(parse_color("#00808080"), Some(0x00808080));
        assert_eq!(parse_color("rgba(255, 221, 0, .38)"), Some(0xffdd0061));
        assert_eq!(parse_color("transparent"), Some(0));
        assert_eq!(parse_color("teal"), None);
        assert_eq!(parse_color("#12345"), None);
    }
}
