#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Line {
    RawLine(String),
    ColonLine(String, String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Token {
    RawText(String),
    Target {
        target: String,
        ip: Option<String>,
        command: String,
    },
}
