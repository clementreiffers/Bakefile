use clap::Parser;
use colored::*;
use duct::cmd;
use std::fs::File;
use std::io::{self, BufRead};
#[derive(Debug)]
struct Bakefile {
    variables: Vec<(String, String)>,
    rules: Vec<Rule>,
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
    if let Some((key, value)) = line.split_once('=') {
        variables.push((key.trim().to_string(), value.trim().to_string()));
    }
}

fn store_recipe(current_rule: &mut Option<Rule>, line: &str) {
    if let Some(rule) = current_rule.as_mut() {
        rule.recipe.push(line.trim().to_string());
    }
}

fn get_rule<'a>(bakefile: &'a Bakefile, target: &'a str) -> Option<&'a Rule> {
    bakefile.rules.iter().find(|rule| rule.target == target)
}

fn read_bakefile(filename: &str) -> io::Result<Bakefile> {
    let mut variables = Vec::new();
    let mut rules = Vec::new();
    let mut current_rule: Option<Rule> = None;

    let file = File::open(filename)?;
    let reader = io::BufReader::new(file);

    for line in reader.lines() {
        let line = line?;

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with(" ") || line.starts_with("\t") {
            store_recipe(&mut current_rule, &line)
        } else if let Some((target, dependencies)) = line.split_once(':') {
            if let Some(rule) = current_rule {
                rules.push(rule);
            }
            current_rule = Some(Rule {
                target: target.trim().to_string(),
                dependencies: dependencies
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect(),
                recipe: Vec::new(),
            });
        } else {
            store_variable(&mut variables, &line);
        }
    }

    if let Some(rule) = current_rule {
        rules.push(rule);
    }

    Ok(Bakefile { variables, rules })
}

fn execute_command(command: &str) {
    println!("{}", command.bold().green());
    // Split the command line into parts
    let mut parts = command.split_whitespace();
    let command = parts.next().expect("No command found");
    let args: Vec<&str> = parts.collect();
    // Use duct to execute the command
    match cmd(command, args).read() {
        Ok(output) => {
            println!("{}", output);
        }
        Err(e) => {
            eprintln!("Error executing command: {}", e);
        }
    }
}

fn execute_recipe(recipe: &Vec<String>) {
    for command in recipe {
        if command.is_empty() {
            continue;
        }
        execute_command(command);
    }
}

fn execute_rule(bakefile: &Bakefile, target_rule: &str) {
    let rule = get_rule(&bakefile, target_rule).unwrap();

    for dependency in &rule.dependencies {
        if let Some(dependency_rule) = get_rule(bakefile, &dependency) {
            execute_rule(bakefile, dependency_rule.target.as_str());
        }
    }
    execute_recipe(&rule.recipe);
}

fn main() {
    let args: Args = Args::parse();
    let bakefile = read_bakefile("Bakefile").unwrap();

    execute_rule(&bakefile, &args.rule);
}
