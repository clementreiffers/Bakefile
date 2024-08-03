use clap::Parser;
use colored::*;
use duct::cmd;
use std::fmt::format;
use std::fs::File;
use std::io::{self, BufRead};
use std::process::exit;
#[derive(Debug)]
struct Bakefile {
    variables: Vec<(String, String)>,
    rules: Vec<Rule>,
    includes: Vec<String>,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    rule: String,
}

#[derive(Debug)]
struct Rule {
    target: String,
    dependencies: Vec<String>,
    recipe: Vec<String>,
}

fn store_variable(variables: &mut Vec<(String, String)>, line: &str) {
    line.split_once('=')
        .map(|(key, value)| variables.push((key.trim().to_string(), value.trim().to_string())));
}

fn store_recipe(rules: &mut Vec<Rule>, line: &str, target: &str) {
    rules
        .iter_mut()
        .find(|rule| rule.target == target)
        .map(|rule| rule.recipe.push(line.trim().to_string()));
}

fn get_rule<'a>(bakefile: &'a Bakefile, target: &'a str) -> Option<&'a Rule> {
    bakefile.rules.iter().find(|rule| rule.target == target)
}

fn populate_bakefile(
    filename: &str,
    variables: &mut Vec<(String, String)>,
    rules: &mut Vec<Rule>,
    includes: &mut Vec<String>,
) {
    let mut current_target: String = String::new();
    let mut is_including: bool = false;

    io::BufReader::new(File::open(filename).expect("File; not found"))
        .lines()
        .filter_map(Result::ok) // Ignore les erreurs de lecture
        .filter(|line| !line.is_empty() && !line.starts_with('#')) // Filtre les lignes vides ou commentaires
        .for_each(|line| {
            if line.starts_with(' ') || line.starts_with('\t') {
                if is_including {
                    includes.push(line.trim().to_string());
                } else {
                    store_recipe(rules, &line, &current_target);
                }
            } else if let Some((target, dependencies)) = line.split_once(':') {
                if target == "include" {
                    is_including = true;
                    return;
                }
                let target: String = target.trim().to_string();
                current_target = target.clone();
                is_including = false;
                rules.push(Rule {
                    target,
                    dependencies: dependencies.split_whitespace().map(String::from).collect(),
                    recipe: Vec::new(),
                });
            } else {
                store_variable(variables, &line);
            }
        });
}
fn read_bakefile(filename: &str) -> io::Result<Bakefile> {
    let mut variables = Vec::new();
    let mut rules = Vec::new();
    let mut includes = Vec::new();

    // Initial population of the bakefile
    populate_bakefile(filename, &mut variables, &mut rules, &mut includes);

    // Collect includes that need further processing
    while let Some(include) = includes.pop() {
        populate_bakefile(&include, &mut variables, &mut rules, &mut includes);
    }

    Ok(Bakefile {
        variables,
        rules,
        includes,
    })
}

fn execute_command(command: String) {
    let (command, args): (&str, Vec<&str>) = command
        .split_whitespace()
        .collect::<Vec<&str>>()
        .split_first()
        .map(|(first, rest)| (*first, rest.to_vec()))
        .expect("No command found");

    cmd(command, args)
        .read()
        .map(|output| println!("{}", output))
        .unwrap_or_else(|e| {
            eprintln!("{}", e.to_string().red());
            exit(1);
        });
}
fn execute_recipe(recipe: &[String], variables: &[(String, String)]) {
    recipe
        .iter()
        .filter(|command| !command.is_empty())
        .for_each(|command| {
            println!("{}", command.bold().green());
            execute_command(set_variables(command, variables));
        });
}

fn set_variables(command: &str, variables: &[(String, String)]) -> String {
    variables
        .iter()
        .fold(command.to_string(), |cmd, (key, value)| {
            cmd.replace(&format!("${}", key), value)
                .replace(&format!("$({})", key), value)
                .replace(&format!("${{{}}}", key), value)
        })
}

fn execute_rule(bakefile: &Bakefile, target_rule: &str) {
    if let Some(rule) = get_rule(bakefile, target_rule) {
        rule.dependencies
            .iter()
            .filter_map(|dependency| get_rule(bakefile, dependency))
            .for_each(|dependency_rule| execute_rule(bakefile, &dependency_rule.target));

        println!("{}", rule.target.bold().blue());
        execute_recipe(&rule.recipe, &bakefile.variables);
    }
}

fn main() {
    let args: Args = Args::parse();
    let bakefile = read_bakefile("Bakefile").unwrap();

    println!("{:?}", bakefile);
    execute_rule(&bakefile, &args.rule);
}
