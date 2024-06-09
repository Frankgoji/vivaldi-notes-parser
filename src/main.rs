/// A utility that can parse the JSON file for Vivaldi notes and return the
/// contents of the desired note based on provided metadata. This will traverse
/// the note hierarchy and return the first note that matches.
use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, BufRead};
use serde_json::{self, json, Value};

fn usage() {
    println!("Usage of vivaldi_notes_parser:");
    println!("vivaldi_notes_parser [-h/--help] [options] [file]");
    println!();
    println!("\t--help/-h\t\tShow this usage message");
    println!("\t--key/-k key\t\tSelect the note with this key, e.g.: -k id");
    println!("\t--value/-v value\tSelect the note with this chosen key and this value, e.g.: -k id -v 456");
    println!("\t--contains/-c contents\tSelect the note with this chosen key and contains the given contents, e.g.: -k contents -c \"Some content\"");
    println!();
    println!("\tIf no options are selected, the parser will print a summary by traversing the notes tree with these fields: {{id, subject, content[:20], children}}");
    println!();
    println!("Examples:");
    println!("\tvivaldi_notes_parser -k id -v 456 Notes");
    println!("\tcat 2022.01.07_21.00.01_Notes.bak | vivaldi_notes_parser -k subject -v \"Todo Queue\"");
}

enum Args {
    Help,
    Key {
        key: Option<String>,
        val: Option<String>,
        contains: Option<String>,
        input: Input,
    },
}

enum Input {
    File(String),
    Stdin,
}

/// Parse the arguments. Retrieve file input as first argument after key, if it
/// is provided.
fn parse_args<I>(args: I) -> Args
    where I: Iterator<Item = String>
{
    let mut key: Option<String> = None;
    let mut val: Option<String> = None;
    let mut contains: Option<String> = None;
    let mut input: Input = Input::Stdin;

    let args: Vec<String> = args.collect();
    let mut args_iter = args.iter().enumerate();
    let mut arg_item = args_iter.next();
    while let Some((i, ref arg)) = arg_item {
        match (i, arg.as_str()) {
            (0, _) => {
                arg_item = args_iter.next();
                continue;
            },
            (_, "-h") | (_, "--help") => {
                return Args::Help;
            },
            (_, "-k") | (_, "--key") => {
                if let Some((_, next_word)) = args_iter.next() {
                    key = Some(String::from(next_word));
                } else {
                    return Args::Help;
                }
            },
            (_, "-v") | (_, "--value") => {
                if let Some((_, next_word)) = args_iter.next() {
                    val = Some(String::from(next_word));
                } else {
                    return Args::Help;
                }
            },
            (_, "-c") | (_, "--contains") => {
                if let Some((_, next_word)) = args_iter.next() {
                    contains = Some(String::from(next_word));
                } else {
                    return Args::Help;
                }
            },
            (n, _) if n == args.len() - 1 => {
                input = Input::File(arg.to_string());
            },
            _ => (),
        }
        arg_item = args_iter.next();
    }

    if let (Some(_v), Some(_c)) = (&val, &contains) {
        return Args::Help;
    }
    // handle case where key is empty but not others
    match (&key, val.is_some() || contains.is_some()) {
        (None, true) => Args::Help,
        _ => Args::Key { key, val, contains, input },
    }
}

/// Traverse the notes json representation and retrieve the contents of the
/// first note object that has a field "key" with the value "val".
fn traverse_json(
    key: &String,
    val: &Option<String>,
    contains: &Option<String>,
    json: &Value
) -> Option<String> {
    let children = match &json["children"] {
        Value::Array(children) if !children.is_empty() => &json["children"],
        _ => &Value::Null,
    };
    match (children, &json[key], &json["content"], val, contains) {
        (Value::Null, Value::String(k), Value::String(content), Some(v), None) if k == v => {
            Some(String::from(content))
        },
        (Value::Null, Value::String(k), Value::String(content), None, Some(c)) if k.contains(c) => {
            Some(String::from(content))
        },
        (Value::Array(children), _, _, _, _) => {
            for child in children {
                let res = traverse_json(key, val, contains, &child);
                if let Some(_) = res {
                    return res;
                }
            }
            return None;
        },
        _ => None,
    }
}

/// Create a summary traversal of the notes json, printing these fields:
/// {id, subject, content[:20], children}
fn summary_traversal(json: &Value) -> Option<String> {
    serde_json::to_string_pretty(&summary_traversal_helper(json)).ok()
}
fn summary_traversal_helper(json: &Value) -> Value {
    let mut res: Value = json!({});

    if let Value::String(id) = &json["id"] {
        res["id"] = Value::String(id.to_string());
    }
    if let Value::String(subject) = &json["subject"] {
        res["subject"] = Value::String(subject[..std::cmp::min(30, subject.len())].to_string());
    }
    if let Value::String(content) = &json["content"] {
        res["content"] = Value::String(content[..std::cmp::min(30, content.len())].to_string());
    }

    match &json["children"] {
        Value::Array(children) if !children.is_empty() => {
            let mut parsed_children: Vec<Value> = Vec::new();
            for child in children {
                parsed_children.push(summary_traversal_helper(child));
            }
            res["children"] = Value::Array(parsed_children);
        },
        _ => {},
    };
    res
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = parse_args(env::args());
    if let Args::Help = args {
        usage();
        return Ok(());
    }

    let Args::Key {key, val, input, contains} = args else {
        panic!("Failed to retrieve arguments");
    };

    let notes_json = if let Input::File(file) = input {
        fs::read_to_string(file)?
    } else {
        io::stdin().lock().lines()
            .map(|r| r.unwrap_or(String::new()))
            .collect::<String>()
    };
    let notes_json: Value = serde_json::from_str(&notes_json)?;

    let content = match key {
        Some(key) => traverse_json(&key, &val, &contains, &notes_json),
        _ => summary_traversal(&notes_json),
    };
    if let Some(content) = content {
        println!("{content}");
    }

    Ok(())
}

/// Unit tests
#[cfg(test)]
mod tests {
    use super::*;

    fn get_string_iter<'a>(v: &'a Vec<&'a str>) -> Box<dyn Iterator<Item = String> + 'a>
    {
        Box::new(v.iter().map(|&i| String::from(i)))
    }

    #[test]
    fn test_index()
    {
        let s = "abcdefg";
        let t = &s[..std::cmp::min(10, s.len())];
        println!("{t}");
    }

    #[test]
    fn test_parse_args()
    {
        let help_vec = vec!["V", "-h"];
        let mut help_args: Box<dyn Iterator<Item = String>> = get_string_iter(&help_vec);
        assert_eq!(help_args.next(), Some(String::from("V")));
        assert_eq!(help_args.next(), Some(String::from("-h")));

        let help_args: Box<dyn Iterator<Item = String>> = get_string_iter(&help_vec);
        let help_args_parsed = parse_args(help_args);
        if let Args::Help = help_args_parsed {
            assert!(true);
        } else {
            assert!(false);
        }

        // No args present should return help
        let help_key_vec = vec!["V"];
        let help_key_args: Box<dyn Iterator<Item = String>> = get_string_iter(&help_key_vec);
        let help_key_args_parsed = parse_args(help_key_args);
        if let Args::Help = help_key_args_parsed {
            assert!(true);
        } else {
            assert!(false);
        }

        // Even with other args present, -h always shows help
        let help_key_vec = vec!["V", "-k", "key", "-h"];
        let help_key_args: Box<dyn Iterator<Item = String>> = get_string_iter(&help_key_vec);
        let help_key_args_parsed = parse_args(help_key_args);
        if let Args::Help = help_key_args_parsed {
            assert!(true);
        } else {
            assert!(false);
        }

        // -k, -v, and -c should return help (only one of -v or -c)
        let val_contains_vec = vec!["V", "-k", "key", "-v", "value", "-c", "contents"];
        let val_contains_args: Box<dyn Iterator<Item = String>> = get_string_iter(&val_contains_vec);
        let val_contains_args_parsed = parse_args(val_contains_args);
        if let Args::Help = val_contains_args_parsed {
            assert!(true);
        } else {
            assert!(false);
        }

        // -v without -k should return help
        let val_only_vec = vec!["V", "-v", "value"];
        let val_only_args: Box<dyn Iterator<Item = String>> = get_string_iter(&val_only_vec);
        let val_only_args_parsed = parse_args(val_only_args);
        if let Args::Help = val_only_args_parsed {
            assert!(true);
        } else {
            assert!(false);
        }

        // -c without -k should return help
        let contains_only_vec = vec!["V", "-c", "contents"];
        let contains_only_args: Box<dyn Iterator<Item = String>> = get_string_iter(&contains_only_vec);
        let contains_only_args_parsed = parse_args(contains_only_args);
        if let Args::Help = contains_only_args_parsed {
            assert!(true);
        } else {
            assert!(false);
        }

        // -k with no key given should return help
        let help_key_vec = vec!["V", "-k"];
        let help_key_args: Box<dyn Iterator<Item = String>> = get_string_iter(&help_key_vec);
        let help_key_args_parsed = parse_args(help_key_args);
        if let Args::Help = help_key_args_parsed {
            assert!(true);
        } else {
            assert!(false);
        }

        // -k, -v, and no file
        let key_vec = vec!["V", "-k", "key", "-v", "value"];
        let key_args: Box<dyn Iterator<Item = String>> = get_string_iter(&key_vec);
        let key_args_parsed = parse_args(key_args);
        if let Args::Key {key, val, contains, input} = key_args_parsed {
            assert_eq!(key, Some(String::from("key")));
            assert_eq!(val, Some(String::from("value")));
            assert_eq!(contains, None);
            if let Input::Stdin = input {
                assert!(true);
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }

        // -k, -v, and file
        let key_vec = vec!["V", "-k", "key", "-v", "value", "test.json"];
        let key_args: Box<dyn Iterator<Item = String>> = get_string_iter(&key_vec);
        let key_args_parsed = parse_args(key_args);
        if let Args::Key {key, val, contains, input} = key_args_parsed {
            assert_eq!(key, Some(String::from("key")));
            assert_eq!(val, Some(String::from("value")));
            assert_eq!(contains, None);
            if let Input::File(file) = input {
                assert_eq!(file, String::from("test.json"));
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }

        // -k, -c, and no file
        let key_vec = vec!["V", "-k", "key", "-c", "contents"];
        let key_args: Box<dyn Iterator<Item = String>> = get_string_iter(&key_vec);
        let key_args_parsed = parse_args(key_args);
        if let Args::Key {key, val, contains, input} = key_args_parsed {
            assert_eq!(key, Some(String::from("key")));
            assert_eq!(val, None);
            assert_eq!(contains, Some(String::from("contents")));
            if let Input::Stdin = input {
                assert!(true);
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }

        // -k, -c, and file
        let key_vec = vec!["V", "-k", "key", "-c", "contents", "test.json"];
        let key_args: Box<dyn Iterator<Item = String>> = get_string_iter(&key_vec);
        let key_args_parsed = parse_args(key_args);
        if let Args::Key {key, val, contains, input} = key_args_parsed {
            assert_eq!(key, Some(String::from("key")));
            assert_eq!(val, None);
            assert_eq!(contains, Some(String::from("contents")));
            if let Input::File(file) = input {
                assert_eq!(file, String::from("test.json"));
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }
}
