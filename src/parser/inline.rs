
use std::str::CharRange;
use regex::Regex;

//XXX dont include control characters
static RX_ABSOLUTE_URI: Regex = regex!(r"<((?i:coap|doi|javascript|aaa|aaas|about|acap|cap|cid|crid|data|dav|dict|dns|file|ftp|geo|go|gopher|h323|http|https|iax|icap|im|imap|info|ipp|iris|iris\.beep|iris\.xpc|iris\.xpcs|iris\.lwz|ldap|mailto|mid|msrp|msrps|mtqp|mupdate|news|nfs|ni|nih|nntp|opaquelocktoken|pop|pres|rtsp|service|session|shttp|sieve|sip|sips|sms|snmp|soap\.beep|soap\.beeps|tag|tel|telnet|tftp|thismessage|tn3270|tip|tv|urn|vemmi|ws|wss|xcon|xcon-userid|xmlrpc\.beep|xmlrpc\.beeps|xmpp|z39\.50r|z39\.50s|adiumxtra|afp|afs|aim|apt|attachment|aw|beshare|bitcoin|bolo|callto|chrome|chrome-extension|com-eventbrite-attendee|content|cvs|dlna-playsingle|dlna-playcontainer|dtn|dvb|ed2k|facetime|feed|finger|fish|gg|git|gizmoproject|gtalk|hcp|icon|ipn|irc|irc6|ircs|itms|jar|jms|keyparc|lastfm|ldaps|magnet|maps|market|message|mms|ms-help|msnim|mumble|mvn|notes|oid|palm|paparazzi|platform|proxy|psyc|query|res|resource|rmi|rsync|rtmp|secondlife|sftp|sgn|skype|smb|soldat|spotify|ssh|steam|svn|teamspeak|things|udp|unreal|ut2004|ventrilo|view-source|webcal|wtai|wyciwyg|xfire|xri|ymsgr):[^<> ]+)>");

static RX_EMAIL_ADDRESS: Regex = regex!(r"<([a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*)>");

#[deriving(Show)]
enum Inline {
    Autolink(String),
    HTMLTag(String),
    CodeSpan(String),
    Link(Box<Inline>, String, String),
    Image(Box<Inline>, String, String),
    Emph(Box<Inline>),
    Strong(Box<Inline>),
    Text(String),
}

pub type InlineText = Vec<Inline>;


fn parse_autolinks(s: &str) -> InlineText {
    for cap in RX_ABSOLUTE_URI.captures_iter(s) {
        println!("->{}<-", cap.at(1));
    }
    for cap in RX_EMAIL_ADDRESS.captures_iter(s) {
        println!("+>{}<-", cap.at(1));
    }
    return Vec::new();
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

            // collect the whole chunk of *** or ___
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
    match number_of_emphs {
        1 => Inline::Emph(box Inline::Text(text)),
        2 => Inline::Strong(box Inline::Text(text)),
        _ => Inline::Strong(box make_emph_strong_inline(text, number_of_emphs-2))
    }
}


fn make_emph_strong_tag(stack: &mut Vec<Emphasis>, pos_in_stack: uint, end_emph: Emphasis) -> (uint, uint, uint) {
    let start_emph: Emphasis = stack[pos_in_stack];
    stack.truncate(pos_in_stack);
    let mut start_pos: uint;
    let mut end_pos: uint;
    let mut length: uint;
    if start_emph.length < end_emph.length {
        length = start_emph.length;
        let mut new_start_emph = end_emph;
        new_start_emph.pos = end_emph.pos + length;
        new_start_emph.length = end_emph.length - length;
        stack.push(new_start_emph);
        start_pos = start_emph.pos;
        end_pos = end_emph.pos;
    } else if start_emph.length > end_emph.length {
        length = end_emph.length;
        let mut new_start_emph = start_emph;
        new_start_emph.length = start_emph.length - length;
        stack.push(new_start_emph);
        start_pos = start_emph.pos + new_start_emph.length;
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
    return parse_autolinks(s.as_slice());
}
