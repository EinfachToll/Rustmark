
use std::str::CharRange;
use regex::Regex;

static RX_ABSOLUTE_URI: Regex = regex!(r"<(?i:coap|doi|javascript|aaa|aaas|about|acap|cap|cid|crid|data|dav|dict|dns|file|ftp|geo|go|gopher|h323|http|https|iax|icap|im|imap|info|ipp|iris|iris\.beep|iris\.xpc|iris\.xpcs|iris\.lwz|ldap|mailto|mid|msrp|msrps|mtqp|mupdate|news|nfs|ni|nih|nntp|opaquelocktoken|pop|pres|rtsp|service|session|shttp|sieve|sip|sips|sms|snmp|soap\.beep|soap\.beeps|tag|tel|telnet|tftp|thismessage|tn3270|tip|tv|urn|vemmi|ws|wss|xcon|xcon-userid|xmlrpc\.beep|xmlrpc\.beeps|xmpp|z39\.50r|z39\.50s|adiumxtra|afp|afs|aim|apt|attachment|aw|beshare|bitcoin|bolo|callto|chrome|chrome-extension|com-eventbrite-attendee|content|cvs|dlna-playsingle|dlna-playcontainer|dtn|dvb|ed2k|facetime|feed|finger|fish|gg|git|gizmoproject|gtalk|hcp|icon|ipn|irc|irc6|ircs|itms|jar|jms|keyparc|lastfm|ldaps|magnet|maps|market|message|mms|ms-help|msnim|mumble|mvn|notes|oid|palm|paparazzi|platform|proxy|psyc|query|res|resource|rmi|rsync|rtmp|secondlife|sftp|sgn|skype|smb|soldat|spotify|ssh|steam|svn|teamspeak|things|udp|unreal|ut2004|ventrilo|view-source|webcal|wtai|wyciwyg|xfire|xri|ymsgr):[^<> ]+>");

static RX_EMAIL_ADDRESS: Regex = regex!(r"<[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*>");

#[derive(Show)]
enum Inline {
    URIAutolink(String),
    EmailAutolink(String),
    HTMLTag(String),
    CodeSpan(String),
    Link(Box<Inline>, String, String),
    Image(Box<Inline>, String, String),
    Emph(Box<Inline>),
    Strong(Box<Inline>),
    Text(String),
}

pub type InlineText = Vec<Inline>;


fn find_first_uri_autolink(text: &str) -> Option<(Inline, uint, uint)> {
    return match RX_ABSOLUTE_URI.find(text) {
        Some((from, to)) => Some((Inline::URIAutolink(text.slice(from+1, to-1).to_string()), from, to)),
        None => None,
    }
}


fn find_first_email(text: &str) -> Option<(Inline, uint, uint)> {
    return match RX_EMAIL_ADDRESS.find(text) {
        Some((from, to)) => Some((Inline::EmailAutolink(text.slice(from+1, to-1).to_string()), from, to)),
        None => None,
    }
}


fn find_first_code_span(text: &str) -> Option<(Inline, uint, uint)> {
    let mut pos = 0;
    let mut seen_bts: Vec<(uint, uint)> = Vec::new();
    while pos < text.len() {
        let CharRange {ch, next} = text.char_range_at(pos);
        if ch == '`' {
            let bts_start = pos;
            let mut bts_length = 1u;

            // collect backtick string
            pos = next;
            while pos < text.len() {
                let CharRange {ch, next} = text.char_range_at(pos);
                pos = next;
                if ch == '`' {
                    bts_length += 1;
                } else {
                    break;
                }
            }

            // look for backtick string of the same length in seen_bts, the text between is the
            // code string
            for &(seen_bts_start, seen_bts_length) in seen_bts.iter() {
                if seen_bts_length == bts_length {
                    return Some((Inline::CodeSpan(
                                text.slice(seen_bts_start+bts_length, bts_start).to_string()),
                                seen_bts_start,
                                bts_start+bts_length));
                }
            }

            // not found, so we append the position info of the backtick string to seen_bts
            seen_bts.push((bts_start, bts_length))
        } else {
            pos = next;
        }
    }
    return None;
}

fn find_first_html(text: &str) -> Option<(Inline, uint, uint)> {
    return None;
}

fn parse_autolinks_code_and_html(text: &str) -> InlineText {
    let mut result = Vec::new();
    let mut pos_in_text = 0u;
    loop {
        let subslice = text.slice_from(pos_in_text);

        // look for the first autolink, code or html block in the text
        let possible_absolut_uri = find_first_uri_autolink(subslice);
        let possible_email = find_first_email(subslice);
        let possible_code_span = find_first_code_span(subslice);
        let possible_html = find_first_html(subslice);

        let mut first_element = box possible_absolut_uri;
        if possible_email.is_some() && (first_element.is_none() || possible_email.as_ref().unwrap().1 < first_element.as_ref().unwrap().1) {
            first_element = box possible_email;
        }
        if possible_code_span.is_some() && (first_element.is_none() || possible_code_span.as_ref().unwrap().1 < first_element.as_ref().unwrap().1) {
            first_element = box possible_code_span;
        }
        if possible_html.is_some() && (first_element.is_none() || possible_html.as_ref().unwrap().1 < first_element.as_ref().unwrap().1) {
            first_element = box possible_html;
        }

        // if there is a autolink, code or html element, split the text and process the second half
        // TODO: there is room for optimization: don't throw away found but unused elements
        if first_element.is_none() {
            break;
        } else {
            let (inline, start, end) = first_element.unwrap();
            result.push(Inline::Text(subslice.slice_to(start).to_string()));
            result.push(inline);
            pos_in_text += end;
        }
    }

    // don't forget the rest of the string
    result.push(Inline::Text(text.slice_from(pos_in_text).to_string()));
    return result;
}

struct Emphasis {
    ch: char,
    pos: uint,
    length: uint,
}

fn parse_emphasis_and_strong(s: &str) -> InlineText {
    let mut result: InlineText = Vec::new();
    let mut positions: Vec<(uint, uint, uint)> = Vec::new();
    let mut stack: Vec<Emphasis> = Vec::new();
    let mut pos = 0;

    // go through all characters from left to right and look for * or _
    while pos < s.len() {
        let CharRange {ch, next} = s.char_range_at(pos);

        if ch == '*' || ch == '_' {

            // collect the whole string of *** or ___
            let symbol = ch;
            let symbol_start = pos;
            let mut length = 1u;
            let mut space_before: bool;
            let mut space_after: bool;
            let mut ascii_alphanum_before: bool;
            let mut ascii_alphanum_after: bool;

            if pos == 0 {
                space_before = true;
                ascii_alphanum_before = false;
            } else {
                let CharRange {ch, next} = s.char_range_at_reverse(pos);
                space_before = ch == ' ';
                ascii_alphanum_before = ch.is_alphanumeric(); //XXX consider only ASCII
            }

            let mut symbol_pos = next;
            while symbol_pos < s.len() {
                let CharRange {ch, next} = s.char_range_at(symbol_pos);
                if ch == symbol {
                    length += 1;
                    symbol_pos = next;
                } else {
                    break;
                }
            }
            let after_symbols = symbol_pos;

            if after_symbols >= s.len() {
                space_after = true;
                ascii_alphanum_after = false;
            } else {
                space_after = s.char_at(after_symbols) == ' ';
                ascii_alphanum_after = s.char_at(after_symbols).is_alphanumeric(); //XXX
            }

            pos = after_symbols;


            let potential_begin = Emphasis{ch:symbol, pos:symbol_start, length: length};

            // put sequences of * or _ that definitively start an emph/strong part on the stack
            if !space_after && (symbol == '*' && space_before) ||
                        (symbol == '_' && !ascii_alphanum_before) {
                stack.push(potential_begin);
                continue;
            }

            // a sequence of * inside text (e.g. bla**bla) ends a emph/strong part if there is
            // a corresponding start on the stack. Otherwise, it starts an e/s part
            if !space_after && symbol == '*' && !space_before {
                match stack.iter().rev().position(|e| e.ch == potential_begin.ch) {
                    Some(pos_in_reverse_stack) => {
                        let starting_element_pos = stack.len() - 1 - pos_in_reverse_stack;
                        positions.push(make_emph_strong_tag(&mut stack, starting_element_pos, potential_begin));
                    },
                    None => {
                        stack.push(potential_begin);
                    }
                }
                continue;
            }

            // sequences of * or _ that end an e/s part
            if !space_before && (symbol == '*' || (symbol == '_' && !ascii_alphanum_after)) {
                if let Some(pos_in_reverse_stack) = stack.iter().rev().position(|e| e.ch == potential_begin.ch) {
                    let starting_element_pos = stack.len() - 1 - pos_in_reverse_stack;
                    positions.push(make_emph_strong_tag(&mut stack, starting_element_pos, potential_begin));
                }
            }

        } else {
            pos = next;
        }
    }
    

    let mut p = 0;
    for &(emph_start, emph_end, emph_length) in positions.iter() {
        if emph_start > 0 {
            result.push(Inline::Text(s.slice(p, emph_start).to_string()));
        }
        result.push(make_emph_strong_inline(s.slice(emph_start + emph_length, emph_end).to_string(), emph_length));
        p = emph_end + emph_length;
    }
    if p < s.len()-1 {
        result.push(Inline::Text(s.slice_from(p).to_string()));
    }
    return result;
}

fn make_emph_strong_inline(text: String, number_of_emphs: uint) -> Inline {
    let mut n = number_of_emphs;
    let mut result: Inline = Inline::Text(text);
    if n % 2 == 1 {
        result = Inline::Emph(box result);
        n -= 1;
    }
    for _ in range(0, n/2) {
        result = Inline::Strong(box result);
    }
    return result;
}


fn make_emph_strong_tag(stack: &mut Vec<Emphasis>, pos_in_stack: uint, end_emph: Emphasis) -> (uint, uint, uint) {
    let start_emph: Emphasis = stack.remove(pos_in_stack);
    stack.truncate(pos_in_stack);
    let mut start_pos: uint;
    let mut end_pos: uint;
    let mut length: uint;
    if start_emph.length < end_emph.length {
        length = start_emph.length;
        let new_start_emph = Emphasis{ch:end_emph.ch, pos: end_emph.pos + length, length: end_emph.length - length};
        stack.push(new_start_emph);
        start_pos = start_emph.pos;
        end_pos = end_emph.pos;
    } else if start_emph.length > end_emph.length {
        length = end_emph.length;
        let new_start_emph = Emphasis{ch: start_emph.ch, length: start_emph.length - length, pos: start_emph.pos };
        stack.push(new_start_emph);
        start_pos = start_emph.pos + start_emph.length - length;
        end_pos = end_emph.pos;
    } else {
        length = start_emph.length;
        start_pos = start_emph.pos;
        end_pos = end_emph.pos;
    }
    return (start_pos, end_pos, length);
}


pub fn parse_inline(s: String) -> InlineText {
    //return parse_emphasis_and_strong(s.as_slice());
    return parse_autolinks_code_and_html(s.as_slice());
    //return vec![Inline::Text(s)];
}
