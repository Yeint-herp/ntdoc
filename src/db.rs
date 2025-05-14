use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StructField {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EnumField {
    pub name: String,
    pub init: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub enum Category {
    #[serde(alias = "NT", alias = "Nt", alias = "NT Native API")]
    Nt,
    #[serde(alias = "Win32", alias = "Win32 API")]
    Win32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum Entry {
    Function {
        name: String,
        return_type: String,
        parameters: Vec<String>,
        description: String,
    },
    Typedef {
        name: String,
        typedef: Vec<String>,
    },
    Define {
        name: String,
        value: String,
    },
    Struct {
        name: String,
        fields: Vec<StructField>,
    },
    Union {
        name: String,
        fields: Vec<StructField>,
    },
    Enum {
        name: String,
        fields: Vec<EnumField>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CategorizedEntry {
    category: Category,
    #[serde(flatten)]
    entry: Entry,
}

impl CategorizedEntry {
    pub fn name(&self) -> &str {
        match &self.entry {
            Entry::Function { name, .. }
            | Entry::Typedef { name, .. }
            | Entry::Define { name, .. }
            | Entry::Struct { name, .. }
            | Entry::Union { name, .. }
            | Entry::Enum { name, .. } => name,
        }
    }

    pub fn raw_definition(&self, all: &[CategorizedEntry]) -> String {
        match &self.entry {
            Entry::Function {
                name,
                return_type,
                parameters,
                ..
            } => {
                let params = parameters.join(", ");
                format!("{} {}({});", return_type, name, params)
            }
            Entry::Define { name, value } => format!("#define {} {}", name, value),
            Entry::Typedef { name, typedef } => {
                let tokens = typedef.join(" ");
                format!("typedef {} {};", tokens, name)
            }
            Entry::Struct { name, fields } => {
                let alias = all.iter().find_map(|e| {
                    if let Entry::Typedef {
                        name: td_name,
                        typedef: tokens,
                    } = &e.entry
                    {
                        if tokens.iter().any(|t| t == name || t.ends_with(name)) {
                            Some(td_name.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });
                let real = alias
                    .clone()
                    .unwrap_or_else(|| name.trim_start_matches('_').to_string());
                let ptr = format!("P{}", real);

                let mut s = format!("typedef struct _{} {{\n", name);
                for f in fields {
                    s.push_str(&format!("    {} {};\n", f.type_, f.name));
                }
                s.push_str(&format!("}} {}, *{};", real, ptr));
                s
            }
            Entry::Union { name, fields } => {
                let mut s = format!("union {} {{\n", name);
                for f in fields {
                    s.push_str(&format!("    {} {};\n", f.type_, f.name));
                }
                s.push_str("};");
                s
            }
            Entry::Enum { fields, .. } => {
                let mut s = String::from("enum {\n");
                for f in fields {
                    if let Some(v) = f.init {
                        s.push_str(&format!("    {} = {},\n", f.name, v));
                    } else {
                        s.push_str(&format!("    {},\n", f.name));
                    }
                }
                s.push_str("};");
                s
            }
        }
    }

    pub fn pretty_definition(&self, all: &[CategorizedEntry]) -> String {
        let mut out = String::new();
        out.push_str(&format!("Category: {:?}\n\n", self.category));
        match &self.entry {
            Entry::Function {
                name,
                return_type,
                parameters,
                description,
            } => {
                out.push_str(&format!("Function `{}`\n", name));
                out.push_str(&format!("Signature: {} {}({});\n\n", return_type, name, parameters.join(", ")));
                out.push_str(&format!("Description:\n{}\n", description));
            }
            Entry::Define { name, value } => {
                out.push_str(&format!("Define `{}`\n\n", name));
                out.push_str(&format!("#define {} {}\n", name, value));
            }
            Entry::Typedef { name, typedef } => {
                out.push_str(&format!("Typedef `{}`\n\n", name));
                out.push_str(&format!("typedef {} {};\n", typedef.join(" "), name));
            }
            Entry::Struct { name, .. } => {
                out.push_str(&format!("Struct `{}`\n\n", name));
                out.push_str(&self.raw_definition(all));
                out.push('\n');
            }
            Entry::Union { name, .. } => {
                out.push_str(&format!("Union `{}`\n\n", name));
                out.push_str(&self.raw_definition(all));
                out.push('\n');
            }
            Entry::Enum { .. } => {
                out.push_str("Enum\n\n");
                out.push_str(&self.raw_definition(all));
                out.push('\n');
            }
        }
        out
    }
}

pub fn database_parse() -> Vec<CategorizedEntry> {
    let s = include_str!("source.json");
    serde_json::from_str(s).unwrap()
}

impl fmt::Display for CategorizedEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.pretty_definition(&[]))
    }
}

