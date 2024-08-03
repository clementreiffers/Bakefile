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

fn store_recipe(current_rule: &mut Option<Rule>, line: &str) {
    current_rule
        .as_mut()
        .map(|rule| rule.recipe.push(line.trim().to_string()));
}

fn get_rule<'a>(bakefile: &'a Bakefile, target: &'a str) -> Option<&'a Rule> {
    bakefile.rules.iter().find(|rule| rule.target == target)
}

fn read_bakefile(filename: &str) -> io::Result<Bakefile> {
    let file = File::open(filename)?;
    let reader = io::BufReader::new(file);

    let mut variables = Vec::new();
    let mut rules = Vec::new();
    let mut current_rule: Option<Rule> = None;

    reader
        .lines()
        .filter_map(Result::ok) // Ignore les erreurs de lecture
        .filter(|line| !line.is_empty() && !line.starts_with('#')) // Filtre les lignes vides ou commentaires
        .for_each(|line| {
            if line.starts_with(' ') || line.starts_with('\t') {
                store_recipe(&mut current_rule, &line);
            } else if let Some((target, dependencies)) = line.split_once(':') {
                if let Some(rule) = current_rule.take() {
                    rules.push(rule);
                }
                current_rule = Some(Rule {
                    target: target.trim().to_string(),
                    dependencies: dependencies.split_whitespace().map(String::from).collect(),
                    recipe: Vec::new(),
                });
            } else {
                store_variable(&mut variables, &line);
            }
        });

    if let Some(rule) = current_rule {
        rules.push(rule);
    }

    Ok(Bakefile { variables, rules })
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
            // .replace(&format!("${{}}", key), value)
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
    execute_rule(&bakefile, &args.rule);
}
