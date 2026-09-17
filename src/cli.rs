use std::env;

pub fn has_flag(name: &str) -> bool {
    env::args().skip(1).any(|argument| argument == name)
}

pub fn flag(name: &str) -> Option<String> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let inline_prefix = format!("{name}=");
    arguments
        .iter()
        .enumerate()
        .find_map(|(index, argument)| {
            if argument == name {
                arguments.get(index + 1).cloned()
            } else {
                argument.strip_prefix(&inline_prefix).map(str::to_owned)
            }
        })
}
