
pub fn preprocess_text(s: &str) -> Vec<String> {
    let mut input: String = s.to_string();
    // delete all U+0000 characters
    input = input.replace("\u0000", "");
    // a string must end with \n in order to make lines_any() work
    if !input.ends_with("\n") {
        input.push('\n');
    }
    let mut result: Vec<String> = Vec::new();
    for line in input.lines_any() {
        result.push(preprocess_line(line));
    }
    return result;
}


/// expands all tabs to spaces
///
/// with tabstop of 4 characters
fn preprocess_line(s: &str) -> String {
    if !s.contains_char('\u0009') {
        return s.to_string();
    }

    let mut result: String = "".to_string();
    let mut col = 0;
    let iter_parts = s.split('\u0009');
    let part_count = iter_parts.count();
    for (part_nr, part) in iter_parts.enumerate() {
        result.push_str(part);
        if part_nr < part_count - 1 {
            let part_length = part.graphemes(true).count();
            col += part_length;
            let additional_spaces = 4 - (col % 4);
            let was: String = " ".repeat(additional_spaces);
            result.push_str(was.as_slice());
            col += additional_spaces;
        }
    }

    return result;
}
