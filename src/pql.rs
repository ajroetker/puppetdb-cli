extern crate rustc_serialize;
extern crate regex;
extern crate docopt;

#[macro_use]
extern crate puppetdb;

use docopt::Docopt;

const USAGE: &'static str = "
pql.

Usage:
  pql (--version | --help)
  pql <partial>

Options:
  -h --help           Show this screen.
  -v --version        Show version.
";

const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

#[derive(Debug, RustcDecodable)]
struct Args {
    flag_version: bool,
    arg_partial: String,
}

use std::io::{Read, Write};
use std::fs::File;
use std::path::Path;

use rustc_serialize::json;

use regex::Regex;

fn get_entity(rest: String) -> (String, String) {
    let s = Regex::new(r"(((.*)(in ))|^[:space:]*)([a-z]+)?[:space:]*(((\[.*\])?[:space:]*\{[:space:]*([^}]*)|\[([a-z]+,[:space:]*)*[a-z]*))?$").unwrap();
    let caps = s.captures(&rest).unwrap();
    let entity = caps.at(5).unwrap_or("");
    let other = caps.at(6).unwrap_or("");
    return (entity.to_string(), other.to_string());
}

fn get_last_clause(rest: String) -> String {
    let s = Regex::new(r"^(.*?[:space:]*(and|or)[:space:]*)*(.*)$").unwrap();
    return s.captures(&rest).unwrap().at(3).unwrap().to_string();
}

fn parse_clause(rest: String) -> (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>){
    let s = Regex::new(r"([a-z]+)?([:space:]+)?(=|~|<|>|>=|<=|~>)?[:space:]*([:^space:]+)?([:space:]+(a|an|o)?)?").unwrap();
    let f = s.captures(&rest).unwrap();
    return (f.at(1).and_then(|x| Some(x.to_string())),
            f.at(2).and_then(|x| Some(x.to_string())),
            f.at(3).and_then(|x| Some(x.to_string())),
            f.at(4).and_then(|x| Some(x.to_string())),
            f.at(5).and_then(|x| Some(x.to_string())));
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());
    if args.flag_version {
        println!("pql v{}", VERSION.unwrap_or("unknown"));
        return;
    }

    let path = "./pql.conf".to_string();
    if !Path::new(&path).exists() {
        pretty_panic!("No config file detected at {:?}", path)
    }
    let mut f = File::open(&path)
        .unwrap_or_else(|e| pretty_panic!("Error opening config {:?}: {}", path, e));
    let mut s = String::new();
    if let Err(e) = f.read_to_string(&mut s) {
        pretty_panic!("Error reading from config {:?}: {}", path, e)
    }
    let json = json::Json::from_str(&s)
        .unwrap_or_else(|e| pretty_panic!("Error parsing config {:?}: {}", path, e));
    //println!("{}", json.find_path(&[&args.arg_partial, "extract"]).unwrap().as_array().unwrap().into_iter().map(|x| x.as_string().unwrap().to_string()).collect::<Vec<String>>().join("\n"));
    let mut rest = args.arg_partial.clone();
    if rest != "" && rest.chars().nth(0).unwrap() == '"' {
        rest.remove(0);
    };
    //let rest = "nod";
    //let rest = "nodes { certname in reports[certname]{certname = 'foo' and certname = 'foo'} and certname in resources[certname] {certname = 'bar' and";
    //let rest = "nodes { certname in reports[certname]{certname = 'foo' and certname = 'foo'} and certname in ";
    //let rest = "nodes { certname in reports[certname]{certname = 'foo' and certname = 'foo'} and certname in nodes[certname] {cert";
    //let rest = "nodes { certname in reports[certname]{certname = 'foo' and certname = 'foo'} and certname in reports[certname, pro";
    let (entity, other) = get_entity(rest.to_string());
    //println!("entitity: {:?}, other: {:?}", entity, other);
    //other empty means entity needs to be completed
    if other.len() == 0 {
        let res = json.as_object().unwrap().keys().cloned().filter(|x| x.contains(&entity)).collect::<Vec<String>>();
        if res.len() == 1 && res[0] == entity {
            println!("{}", [entity.clone() + "[", entity.clone() + "{" ].join("\n"));
        } else {
            println!("{}", res.join("\n"));
        }
        return;
    }
    let s = Regex::new(r"(\[.*\])?[:space:]*\{[:space:]*([^}]*)").unwrap();
    if s.is_match(&other) {
        let caps = s.captures(&other).unwrap();
        let o = caps.at(2).unwrap_or("");
        let f = get_last_clause(o.to_string());
        let (a, b, c, d, e) = parse_clause(f.to_string());
        if a.is_none() {
            println!("{}", json.find_path(&[&entity, "query"]).unwrap().as_array().unwrap().into_iter().map(|x| x.as_string().unwrap().to_string()).collect::<Vec<String>>().join("\n"));
        } else if b.is_none() && c.is_none() {
            let p = a.unwrap();
            let ps = json.find_path(&[&entity, "query"]).unwrap().as_array().unwrap().into_iter().map(|x| x.as_string().unwrap().to_string()).filter(|x| x.contains(&p)).collect::<Vec<String>>();
            if ps.len() == 1 && ps[0] == p {
                println!("{} {}", p, ["in", "=", ">", ">=", "<", "<=", "~", "~>"].join(&format!("\n{} ", p)));
            } else {
                println!("{}", ps.join("\n"));
            };
        } else if c.is_none() {
            println!("{}", ["in", "=", ">", ">=", "<", "<=", "~", "~>"].join("\n"));
        } else if d.is_none() {
            // do nothing
        } else if e.is_some() {
            let ob = e.unwrap().clone();
            let tm = ob.trim();
            if tm == "a" || tm == "an" {
                println!("and");
            } else if tm == "o" {
                println!("or");
            } else {
                println!("{}", ["and", "or"].join("\n"));
            };
        }
        return;
    }
    let t = Regex::new(r"\[([a-z]+,[:space:]*)*([a-z]*)$").unwrap();
    if t.is_match(&other) {
        let caps = t.captures(&other).unwrap();
        let o = caps.at(2).unwrap_or("");
        println!("{}", json.find_path(&[&entity, "extract"]).unwrap().as_array().unwrap().into_iter().map(|x| x.as_string().unwrap().to_string()).filter(|x| x.contains(&o)).collect::<Vec<String>>().join("\n"));
        return;
    }

//    let f = Regex::new(r"^([a-z]+)[:space:]*\[[:space:]*$").unwrap();
//    let s = Regex::new(r"^([a-z]+)[:space:]*(\[.*\])?[:space:]*\{[:space:]*$").unwrap();
//    if rest == "" {
//        println!("{}", json.as_object().unwrap().keys().cloned().collect::<Vec<String>>().join("\n"));
//    } else if f.is_match(&rest) {
//        let entity = f.captures(&rest).unwrap().at(1).unwrap();
//        println!("{}", json.find_path(&[&entity, "extract"]).unwrap().as_array().unwrap().into_iter().map(|x| x.as_string().unwrap().to_string()).collect::<Vec<String>>().join("\n"));
//    } else if s.is_match(&rest) {
//        let entity = s.captures(&rest).unwrap().at(1).unwrap();
//        println!("{}", json.find_path(&[&entity, "query"]).unwrap().as_array().unwrap().into_iter().map(|x| x.as_string().unwrap().to_string()).collect::<Vec<String>>().join("\n"));
//    } else if Regex::new(r"^([a-z]+)[:space:]+$").unwrap().is_match(&rest) {
//        println!("{}\n{}", "[", "{");
//    } else if Regex::new(r"^([a-z]+)[:space:]*$").unwrap().is_match(&rest) {
//        let res = json.as_object().unwrap().keys().cloned().filter(|x| x.contains(&rest)).collect::<Vec<String>>();
//        if res.len() == 1 && res[0] == rest {
//            println!("{0}[\n{0}{1}", rest, '{');
//        } else {
//            println!("{}", res.join("\n"));
//        }
//    } else {}

//    if args.arg_partial == "\"nodes[" {
//        println!("{}", "\
//            certname\n\
//            producer_timestamp\n\
//            producer");
//    } else if args.arg_partial == "\"nodes{" {
//        println!("{}", "\
//            certname\n\
//            producer");
//    } else if args.arg_partial == "\"nodes{ certname " {
//        println!("{}", "\
//            ==\n\
//            ~=");
//    } else if args.arg_partial == "\"nodes" {
//        println!("{}", "\
//            [\n\
//            {");
//    } else {
//        println!("{}", "\
//            nodes\n\
//            facts\n\
//            reports");
//    }

}
