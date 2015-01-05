
use std::collections::HashMap;
use regex::Regex;

mod inline;
mod preprocess;


static RX_HORIZONTAL_RULE: Regex = regex!(r"^ {0,3}(?:(?:\* *){3,}|(?:- *){3,}|(?:_ *){3,})$");
static RX_HEADER_ATX:Regex = regex!(r"^ {0,3}(#{1,6})(?: +(.*?))??(?: +#* *)?$");
static RX_HEADER_SETEXT_TEXT:Regex = regex!(r"^ {0,3}(\S.*?) *$");
static RX_HEADER_SETEXT_LINE:Regex = regex!(r"^ {0,3}(-+|=+) *$");
static RX_BLANK_LINE:Regex = regex!(r"^ *$");
static RX_INDENTED_CODE_LINE_NO_BLANK_LINE:Regex = regex!(r"^ {4}(.*)$");
static RX_INDENTED_CODE_LINE_BLANK_LINE:Regex = regex!(r"^ {0,4}( *)$");
static RX_BLOCKQUOTE_LINE:Regex = regex!(r"^ {0,3}> ?(.*)$");
static RX_LIST_ITEM:Regex = regex!(r"^( {0,3}([-*+]|\d+[.)]) )( *)(.*)$");
static RX_CODE_FENCE_START: Regex = regex!(r"^( {0,3})(`{3,}|~{3,}) *([^`]*?) *$");
static RX_CODE_FENCE_END: Regex = regex!(r"^ {0,3}(`{3,}|~{3,}) *$");
static RX_LINK_REFERENCE_DEFINITION_START: Regex = regex!(r"^ {0,3}\[((?:[^\]]|\\\])+)\]:(.*)$");
static RX_HTML_BLOCK: Regex = regex!(r"^ {0,3}<(?:/?(?i:article|header|aside|hgroup|blockquote|hr|iframe|body|li|map|button|object|canvas|ol|caption|output|col|p|colgroup|pre|dd|progress|div|section|dl|table|td|dt|tbody|embed|textarea|fieldset|tfoot|figcaption|th|figure|thead|footer|tr|form|ul|h1|h2|h3|h4|h5|h6|video|script|style)|!|\?)");

#[deriving(Show)]
enum Block<'r> {
    Rule,
    Header(uint, inline::InlineText),
    Paragraph(inline::InlineText),
    Code(Option<inline::InlineText>, String),
    BlockQuote(Box<Vec<Block<'r>>>),
    HTMLBlock(inline::InlineText),
    List(bool, Vec<ListItem<'r>>),
}

#[deriving(Show)]
struct ListItem<'r> {
    blocks: Box<Vec<Block<'r>>>,
    listtype: ListType,
}

#[deriving(Show)]
enum ListType {
    Ordered(uint, char),    // start number and '.' or ')'
    Unordered(char)         // '-' or '+' or '*'
}

impl PartialEq for ListType {
    fn eq(&self, other: &ListType) -> bool {
        match self {
            &ListType::Ordered(_, self_char) => {
                match other {
                    &ListType::Ordered(_, other_char) => self_char == other_char,
                    _ => false
                }
            },
            &ListType::Unordered(self_char) => {
                match other {
                    &ListType::Unordered(other_char) => self_char == other_char,
                    _ => false
                }
            }
        }
    }
}

#[deriving(Show)]
enum ContainerType {
    BQ,
    LI(uint) // List item with a width
}

#[deriving(Show)]
struct ParseState<'r> {
    s: &'r mut[String],
    pos: uint,
    in_paragraph: bool,
    container_stack: Vec<ContainerType>,
    //link_reference_defs: Vec<HashMap<String, (String,String)>>,
}


impl<'r> ParseState<'r> {
    fn new(s: &mut[String]) -> ParseState {
        ParseState { s: s, pos: 0, in_paragraph: false, container_stack: Vec::new() }
    }

    fn strip_container_prefixes<'r>(&'r self, line: &'r str) -> Option<&str> {
        let mut result = line;
        for container in self.container_stack.iter() {
            match *container {
                ContainerType::BQ => {
                    match RX_BLOCKQUOTE_LINE.captures(result) {
                        None => {
                            return if self.in_paragraph {
                                Some(result)
                            } else {
                                return None;
                            }
                        },
                        Some(cap) => {
                            result = cap.at(1);
                        }
                    }
                },
                ContainerType::LI(indent) => {
                    if result.chars().take(indent).all(|x| x==' ') {
                        if indent > result.len() {
                            result = result.slice_from(result.len());
                        } else {
                            result = result.slice_from(indent);
                        }
                    } else if self.in_paragraph {
                        return Some(result);
                    } else if RX_BLANK_LINE.is_match(result) {
                        return Some(result);
                    } else {
                        return None;
                    }
                }
            }
        }
        return Some(result);
    }

    fn current_line(&self) -> Option<String> {
        //XXX die Funktion wird unnötig oft aufgerufen
        if self.pos >= self.s.len() {
            return None;
        }
        return match self.strip_container_prefixes(self.s[self.pos].as_slice()) {
            None => None,
            Some(stripped_line) => Some(stripped_line.to_string())
        }
    }

    fn next_line(&self) -> Option<String> {
        if self.pos >= self.s.len()-1 {
            return None;
        }
        return match self.strip_container_prefixes(self.s[self.pos+1].as_slice()) {
            None => None,
            Some(stripped_line) => Some(stripped_line.to_string())
        }
    }

    fn onwards(&mut self) {
        self.pos += 1;
    }

    fn parse_blocks(&mut self) -> (Vec<Block<'r>>, bool, bool) {
        let mut blocks = Vec::new();
        let mut no_blank_line = true;
        let mut more_than_2_empty_lines = false;
        while self.current_line().is_some() {

            let skipped_lines = self.skip_empty_lines();
            if skipped_lines >= 2 && !self.container_stack.is_empty() {
                if let &ContainerType::LI(_) = self.container_stack.last().unwrap() {
                    more_than_2_empty_lines = true;
                    break;
                }
            }
            if skipped_lines >= 1 {
                no_blank_line = false;
            }

            self.parse_link_reference_definition();

            if let Some(block) = self.parse_horizontal_rule() {
                blocks.push(block);
                continue;
            }

            
            if let Some(block) = self.parse_atx_header() {
                blocks.push(block);
                continue;
            }
            
            if let Some(block) = self.parse_indented_code_block() {
                blocks.push(block);
                continue;
            }

            if let Some(block) = self.parse_html_block() {
                blocks.push(block);
                continue;
            }

            if let Some(block) = self.parse_fenced_code_block() {
                blocks.push(block);
                continue;
            }

            if let Some(block) = self.parse_blockquote() {
                blocks.push(block);
                continue;
            }

            if let Some((block, end_all_lists)) = self.parse_list() {
                blocks.push(block);
                if end_all_lists {
                    more_than_2_empty_lines = true;
                    if !self.container_stack.is_empty() {
                        if let &ContainerType::LI(_) = self.container_stack.last().unwrap() {
                            break;
                        }
                    }
                    continue;
                } else {
                    continue;
                }
            }

            if let Some(block) = self.parse_setext_header() {
                blocks.push(block);
                continue;
            }

            if let Some(block) = self.parse_paragraph() {
                blocks.push(block);
                continue;
            }

        }

        if !self.container_stack.is_empty() {
            self.container_stack.pop();
        }

        return (blocks, no_blank_line, more_than_2_empty_lines);
    }


    fn see_empty_line(&self) -> bool {
        return match self.current_line() {
            None => false,
            Some(line) => RX_BLANK_LINE.is_match(line.as_slice())
        }
    }

    /// also returns the number of skipped empty lines
    fn skip_empty_lines(&mut self) -> uint {
        let mut blank_lines_count = 0u;
        loop {
            match self.current_line() {
                None => { break; },
                Some(line) => {
                    if RX_BLANK_LINE.is_match(line.as_slice()) {
                        self.onwards();
                        blank_lines_count += 1;
                    } else {
                        break;
                    }
                }
            }
        }
        return blank_lines_count;
    }

    fn see_horizontal_rule(&self) -> bool {
        return match self.current_line() {
            None => false,
            Some(line) => RX_HORIZONTAL_RULE.is_match(line.as_slice())
        }
    }


    fn parse_horizontal_rule(&mut self) -> Option<Block<'r>> {
        let line = self.current_line();
        if line.is_some() && RX_HORIZONTAL_RULE.is_match(line.unwrap().as_slice()) {
            self.onwards();
            return Some(Block::Rule);
        } else {
            return None;
        }
    }


    fn see_atx_header(&self) -> bool {
        return match self.current_line() {
            None => false,
            Some(line) => RX_HEADER_ATX.is_match(line.as_slice())
        }
    }

    fn parse_atx_header(&mut self) -> Option<Block<'r>> {
        let line = self.current_line();
        if line.is_none() {
            return None;
        }
        match RX_HEADER_ATX.captures(line.unwrap().as_slice()) {
            None => {
                return None;
            },
            Some(cap) => {
                let level = cap.at(1).len();
                let matched_str = cap.at(2);
                self.onwards();
                return Some(Block::Header(level, inline::parse_inline(matched_str.to_string())));
            }
        }
    }


    fn parse_setext_header(&mut self) -> Option<Block<'r>> {
        let current_line = self.current_line();
        let next_line = self.next_line();
        if current_line.is_none() || next_line.is_none() {
            return None;
        }
        match RX_HEADER_SETEXT_TEXT.captures(current_line.unwrap().as_slice()) {
            None => {
                return None;
            },
            Some(cap_text) => {
                match RX_HEADER_SETEXT_LINE.captures(next_line.unwrap().as_slice()) {
                    None => {
                        return None;
                    },
                    Some(cap_line) => {
                        let level = if cap_line.at(1).char_at(0) == '=' { 1 } else { 2 };
                        let matched_str = cap_text.at(1);
                        self.onwards();
                        self.onwards();
                        return Some(Block::Header(level, inline::parse_inline(matched_str.to_string())));
                    }
                }
            }
        }
    }


    fn parse_indented_code_block(&mut self) -> Option<Block<'r>> {
        let mut is_indented_code_block = false;
        let mut code_string: Vec<String> = Vec::new(); //XXX wenn ich hier &str nehme, passieren komische Sachen
        loop {
            match self.current_line() {
                None => { break; },
                Some(line) => {
                    match RX_INDENTED_CODE_LINE_NO_BLANK_LINE.captures(line.as_slice()) {
                        Some(cap) => {
                            is_indented_code_block = true;
                            code_string.push(cap.at(1).to_string());
                            self.onwards();
                        },
                        None => {
                            match RX_INDENTED_CODE_LINE_BLANK_LINE.captures(line.as_slice()) {
                                Some(cap) => {
                                    is_indented_code_block = true;
                                    code_string.push(cap.at(1).to_string());
                                    self.onwards();
                                },
                                None => { break; }
                            }
                        }
                    }
                }
            }
        }

        // delete trailing blank lines
        while !code_string.is_empty() &&
                    RX_BLANK_LINE.is_match(code_string[code_string.len()-1].as_slice()) {
            code_string.pop();
        }

        let code_string_s: String = code_string.iter().fold("".to_string(), |x, y| x+y+'\n'.to_string());

        return if is_indented_code_block {
            Some(Block::Code(None, code_string_s))
        } else {
            None
        }
    }

    fn see_fenced_code_block(&self) -> bool {
        return match self.current_line() {
            None => false,
            Some(line) => RX_CODE_FENCE_START.is_match(line.as_slice())
        }
    }

    fn parse_fenced_code_block(&mut self) -> Option<Block<'r>> {
        let mut code_string: Vec<String> = Vec::new();
        let line = self.current_line();
        if line.is_none() {
            return None;
        }
        match RX_CODE_FENCE_START.captures(line.unwrap().as_slice()) {
            None => {
                return None;
            },
            Some(cap) => {
                let starting_fence_indent = cap.at(1).len();
                let starting_fence_char = cap.at(2).char_at(0);
                let starting_fence_len = cap.at(2).len();
                let info_string = if cap.at(3).len() > 0 {
                    Some(inline::parse_inline(cap.at(3).to_string()))
                } else {
                    None
                };

                loop {
                    self.onwards();

                    let line = self.current_line();

                    if line.is_none() {
                        break;
                    }

                    let line_string = line.unwrap();
                    let mut line_slice = line_string.as_slice();

                    if let Some(cap) = RX_CODE_FENCE_END.captures(line_slice) {
                        if cap.at(1).char_at(0) == starting_fence_char && cap.at(1).len() >= starting_fence_len {
                            break;
                        }
                    }

                    for _ in range(0, starting_fence_indent) {
                        if line_slice.char_at(0) == ' ' {
                            line_slice = line_slice.slice_from(1);
                        }
                    }

                    code_string.push(line_slice.to_string());
                }

                self.onwards();
                let code_string_s: String = code_string.iter().fold("".to_string(), |x, y| x+y+'\n'.to_string());
                return Some(Block::Code(info_string, code_string_s));
            }
        }
    }

    fn see_blockquote(&self) -> bool {
        return match self.current_line() {
            None => false,
            Some(line) => RX_BLOCKQUOTE_LINE.is_match(line.as_slice())
        }
    }

    fn parse_blockquote(&mut self) -> Option<Block<'r>> {
        if self.see_blockquote() {
            self.container_stack.push(ContainerType::BQ);
            let blocks = box self.parse_document();
            return Some(Block::BlockQuote(blocks));
        } else {
            return None;
        }
    }

    fn see_list_item(&self) -> Option<ListType> {
        return match self.current_line() {
            None => None,
            Some(line) => {
                match RX_LIST_ITEM.captures(line.as_slice()) {
                    None => None,
                    Some(cap) => Some(get_list_type(cap.at(2)))
                }
            }
        }
    }

    fn parse_list_item(&mut self) -> Option<(ListItem<'r>, bool, bool)> {
        let line = self.current_line();
        if line.is_none() {
            return None;
        }
        match RX_LIST_ITEM.captures(line.unwrap().as_slice()) {
            None => {
                return None;
            },
            Some(cap) => {
                let marker = cap.at(2);
                let list_type = get_list_type(marker);
                let number_of_additional_spaces = cap.at(3).len();
                let marker_pos = self.s[self.pos].find_str(marker).unwrap();
                let mut line_rest: String;
                let mut width;
                if number_of_additional_spaces <= 3 {
                    width = cap.at(1).len() + number_of_additional_spaces;
                    line_rest = " ".repeat(marker_pos + width) + cap.at(4).to_string();
                } else { // list item starting with indented code
                    width = cap.at(1).len();
                    line_rest = " ".repeat(marker_pos + width + number_of_additional_spaces) + cap.at(4).to_string();
                }
                self.s[self.pos] = line_rest;
                self.container_stack.push(ContainerType::LI(width));
                let (blocks, is_tight, end_list) = self.parse_blocks();
                return Some((ListItem { blocks: box blocks, listtype: list_type }, is_tight, end_list));
            }
        }
    }

    fn parse_list(&mut self) -> Option<(Block<'r>, bool)> {
        let first_list_item = self.parse_list_item();
        if first_list_item.is_none() {
            return None;
        }
        let (li, ti, el) = first_list_item.unwrap();
        let first_list_type = li.listtype;
        let mut list_items = vec![li];
        let mut tight = ti;
        let mut end_list = el;
        loop {

            if end_list {
                break;
            }

            let skipped_lines = self.skip_empty_lines();
            if skipped_lines >= 2 {
                break;
            }

            match self.see_list_item() {
                None => break,
                Some(seen_list_type) => {
                    if seen_list_type != first_list_type {
                        break;
                    }
                }
            }

            if self.see_horizontal_rule() {
                break;
            }

            if skipped_lines >= 1 {
                tight = false;
            }

            let list_item = self.parse_list_item();
            let (li, ti, el) = list_item.unwrap();
            if li.listtype != first_list_type {
                break;
            }
            list_items.push(li);
            if !ti {
                tight = false;
            }
            end_list = el;
        }
        return Some((Block::List(tight, list_items), end_list));
    }

    fn see_html_block(&self) -> bool {
        return match self.current_line() {
            None => false,
            Some(line) => RX_HTML_BLOCK.is_match(line.as_slice())
        }
    }

    fn parse_html_block(&mut self) -> Option<Block<'r>> {
        if self.see_html_block() {
            let mut block_string = self.current_line().unwrap() + '\n'.to_string();
            loop {
                self.onwards();

                if self.see_empty_line() {
                    break;
                }

                match self.current_line() {
                    None => break,
                    Some(line) => {
                        block_string.push_str(line.as_slice());
                        block_string.push('\n');
                    }
                }
            }
            return Some(Block::HTMLBlock(inline::parse_inline(block_string)));
        } else {
            return None;
        }
    }

    fn parse_paragraph(&mut self) -> Option<Block<'r>> {
        let mut is_paragraph = false;
        let mut paragraph_string: String = "".to_string();
        //XXX die erste Zeile ist immer ne Paragrafenzeile, da muss man nix prüfen
        loop {
            let current_line = self.current_line();

            if current_line.is_none() {
                break;
            }

            if self.see_empty_line() {
                break;
            }

            if self.see_atx_header() {
                break;
            }

            if self.see_horizontal_rule() {
                break;
            }

            if self.see_html_block() {
                break;
            }

            if self.see_blockquote() {
                break;
            }

            if self.see_list_item().is_some() {
                break;
            }

            if self.see_fenced_code_block() {
                break;
            }

            is_paragraph = true;
            paragraph_string.push_str(current_line.unwrap().as_slice());
            paragraph_string.push('\n');
            self.in_paragraph = true;
            self.onwards();
        }

        self.in_paragraph = false;

        return if is_paragraph {
            Some(Block::Paragraph(inline::parse_inline(paragraph_string)))
        } else {
            None
        }
    }

    fn parse_link_reference_definition(&mut self) {
        if let Some(line) = self.current_line() {
            if let Some(cap) = RX_LINK_REFERENCE_DEFINITION_START.captures(line.as_slice()) {

            }
        }
    }

    fn parse_document(&mut self) -> Vec<Block<'r>> {
        return self.parse_blocks().val0();
    }

}



fn get_list_type(marker: &str) -> ListType {
    if marker.len() == 1 {
        return ListType::Unordered(marker.char_at(0));
    } else {
        let length = marker.len();
        return ListType::Ordered(
            from_str(marker.slice_to(length - 1)).unwrap(),
            marker.char_at(length-1)
            );
    }
}

pub fn parse_markdown(md_string: String) -> String {
    let mut md_lines: Vec<String> = preprocess::preprocess_text(md_string.as_slice());

    let st = &mut ParseState::new(md_lines.as_mut_slice());

    let parse_result = st.parse_document();
    return format!("{}", parse_result);
}








#[allow(dead_code)]
fn druck(s: String) {
    let s = s.replace(" ", "·").replace("\n", "\\n").replace("\u0009", "→");
    println!("->{}<-", s);
}



#[test]
fn test_atx_headers() {
    let text = preprocess_text("# eine Überschrift");
    let s = ParseState::new(text.as_slice());
    let block = parse_atx_header(s).unwrap().val0();
    match block {
        Block::Header(1, "eine Überschrift") => assert!(true),
        _ => assert!(false)
    }
}

#[cfg(test)]
fn bla(teststring: &str, result: Block) -> bool {
    let text = preprocess_text(teststring);
    let s = ParseState::new(text.as_slice());
    let block = parse_atx_header(s).unwrap().val0();
    match block {
        Block::Header(1, "eine Überschrift") => assert!(true),
        _ => assert!(false)
    }
}



