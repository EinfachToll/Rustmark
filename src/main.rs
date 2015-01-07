#![feature(phase)]
#[phase(plugin)]
extern crate regex_macros;
extern crate regex;

use std::io::fs::File;

mod parser;


fn main() {
    let program_args: Vec<String> = std::os::args();
    if program_args.len() < 2 {
        println!("Wie wärsn mit ner Datei, hä?");
        return;
    }
    let md_filename:&String = &program_args[1];
    let mut md_file = File::open(&Path::new(md_filename)).ok().expect("Sicher, dass das ne Datei ist?");
    let md_string:String = md_file.read_to_string().ok().expect("Kann nix lesen");

    let result = parser::parse_markdown(md_string);

    println!("{}", result);
}
